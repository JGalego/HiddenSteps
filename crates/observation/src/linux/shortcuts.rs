use hiddensteps_domain::{CapturedPayload, CapturedSignal, PrivacyLevel};
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{ConnectionExt, GrabMode};
use x11rb::protocol::Event;
use x11rb::rust_connection::RustConnection;

use crate::{ObservationSource, PollError};

/// One shortcut to watch for: an X11 modifier mask (`ShiftMask`, `ControlMask`,
/// ... OR'd together), a keycode, and the human-readable label to report when it
/// fires. Callers work in keycodes rather than symbolic key names deliberately —
/// keysym-to-keycode resolution (so a caller could ask for `"Ctrl+Shift+4"` by
/// name) is real, non-trivial X11 keyboard-mapping work that's a legitimate
/// follow-up, not something worth faking with a hardcoded lookup table here.
pub struct ShortcutBinding {
    pub modifiers: u16,
    pub keycode: u8,
    pub label: String,
}

/// Watches for specific global keyboard shortcuts via `XGrabKey`.
///
/// **This is invasive by construction**: `XGrabKey` intercepts the given key combo
/// session-wide, for every application, until explicitly ungrabbed. Per the
/// caveats in this crate's `lib.rs` doc comment, nothing in HiddenSteps
/// auto-starts this source with a default binding list — a caller must
/// explicitly choose which combos to grab, same as a user would explicitly
/// configure which shortcuts they want tracked.
pub struct GlobalShortcutSource {
    conn: RustConnection,
    root: u32,
    bindings: Vec<ShortcutBinding>,
}

impl GlobalShortcutSource {
    pub fn grab(
        root: u32,
        conn: RustConnection,
        bindings: Vec<ShortcutBinding>,
    ) -> Result<Self, PollError> {
        for binding in &bindings {
            conn.grab_key(
                true,
                root,
                binding.modifiers.into(),
                binding.keycode,
                GrabMode::ASYNC,
                GrabMode::ASYNC,
            )
            .map_err(|e| PollError::Backend(e.to_string()))?
            .check()
            .map_err(|e| PollError::Backend(e.to_string()))?;
        }
        conn.flush()
            .map_err(|e| PollError::Backend(e.to_string()))?;
        Ok(Self {
            conn,
            root,
            bindings,
        })
    }

    /// Convenience constructor that opens its own connection to the display named
    /// by `$DISPLAY` (or the default), then grabs `bindings`.
    pub fn connect_and_grab(bindings: Vec<ShortcutBinding>) -> Result<Self, PollError> {
        let (conn, screen_num) =
            x11rb::connect(None).map_err(|e| PollError::Backend(e.to_string()))?;
        let root = conn.setup().roots[screen_num].root;
        Self::grab(root, conn, bindings)
    }
}

impl Drop for GlobalShortcutSource {
    fn drop(&mut self) {
        for binding in &self.bindings {
            // Best-effort: if ungrab fails during teardown there is nothing
            // meaningful this destructor can do about it, and it must not panic.
            let _ = self
                .conn
                .ungrab_key(binding.keycode, self.root, binding.modifiers.into());
        }
        let _ = self.conn.flush();
    }
}

impl ObservationSource for GlobalShortcutSource {
    fn id(&self) -> &str {
        "linux.global_shortcuts"
    }

    fn min_privacy_level(&self) -> PrivacyLevel {
        PrivacyLevel::ApplicationMetadata
    }

    fn poll(&mut self) -> Result<Vec<CapturedSignal>, PollError> {
        let mut signals = Vec::new();
        loop {
            let event = self
                .conn
                .poll_for_event()
                .map_err(|e| PollError::Backend(e.to_string()))?;
            let Some(event) = event else { break };
            if let Event::KeyPress(key_press) = event {
                if let Some(binding) = self.bindings.iter().find(|b| {
                    b.keycode == key_press.detail && b.modifiers == u16::from(key_press.state)
                }) {
                    signals.push(CapturedSignal::new(
                        self.id(),
                        CapturedPayload::ShortcutInvoked {
                            shortcut: binding.label.clone(),
                        },
                    ));
                }
            }
        }
        Ok(signals)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Grabbing a real global shortcut in an automated test suite would intercept
    // that key combo for the whole (shared, WSLg-backed) X session for the
    // duration of the test — exactly the interference risk described in this
    // module's doc comment. This test is real (not a mock) but is `#[ignore]`d so
    // it never runs automatically; run it deliberately, alone, with
    // `cargo test -- --ignored grabs_and_ungrabs_a_real_shortcut`.
    #[test]
    #[ignore = "performs a real session-wide XGrabKey; do not run in shared/automated environments"]
    fn grabs_and_ungrabs_a_real_shortcut() {
        let bindings = vec![ShortcutBinding {
            modifiers: 0,
            // A keycode unlikely to be bound to anything meaningful on a typical
            // layout, chosen only so this test doesn't fight a real shortcut.
            keycode: 191, // F13 on most layouts
            label: "test-shortcut".to_string(),
        }];
        let source = GlobalShortcutSource::connect_and_grab(bindings)
            .expect("grab should succeed against a live display");
        drop(source); // exercises the ungrab path in Drop
    }
}
