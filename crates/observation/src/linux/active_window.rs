use hiddensteps_domain::{CapturedPayload, CapturedSignal, PrivacyLevel};
use x11rb::protocol::xproto::{AtomEnum, ConnectionExt};
use x11rb::rust_connection::RustConnection;

use crate::{ObservationSource, PollError};

/// Watches which window has X11 input focus and reports app-focus-change and
/// window-title signals when it changes — the Linux implementation of
/// `docs/design/05-privacy-model.md` §1's Level 1 signals.
///
/// Uses core X11's `GetInputFocus` request rather than the EWMH
/// `_NET_ACTIVE_WINDOW` root-window property, deliberately: `GetInputFocus` is
/// part of the base X11 protocol every X server implements, so this works
/// regardless of which (or whether any) EWMH-compliant window manager is running
/// — relevant since minimal/embedded compositors (including the one this was
/// developed and tested against, WSLg's) don't reliably set `_NET_ACTIVE_WINDOW`.
pub struct ActiveWindowSource {
    conn: RustConnection,
    net_wm_name: u32,
    utf8_string: u32,
    last_window: Option<u32>,
    last_class: Option<String>,
    last_title: Option<String>,
}

impl ActiveWindowSource {
    pub fn connect() -> Result<Self, PollError> {
        let (conn, _screen_num) =
            x11rb::connect(None).map_err(|e| PollError::Backend(e.to_string()))?;
        let net_wm_name = intern_atom(&conn, b"_NET_WM_NAME")?;
        let utf8_string = intern_atom(&conn, b"UTF8_STRING")?;
        Ok(Self {
            conn,
            net_wm_name,
            utf8_string,
            last_window: None,
            last_class: None,
            last_title: None,
        })
    }

    fn focused_window(&self) -> Result<u32, PollError> {
        let focus = self
            .conn
            .get_input_focus()
            .map_err(|e| PollError::Backend(e.to_string()))?
            .reply()
            .map_err(|e| PollError::Backend(e.to_string()))?;
        Ok(focus.focus)
    }

    fn wm_class(&self, window: u32) -> Result<Option<String>, PollError> {
        let reply = self
            .conn
            .get_property(false, window, AtomEnum::WM_CLASS, AtomEnum::STRING, 0, 1024)
            .map_err(|e| PollError::Backend(e.to_string()))?
            .reply()
            .map_err(|e| PollError::Backend(e.to_string()))?;
        // WM_CLASS is two NUL-separated strings: instance name, then class name.
        // The class name (second component) is the stable app identifier.
        let parts: Vec<&[u8]> = reply.value.split(|&b| b == 0).collect();
        Ok(parts
            .get(1)
            .filter(|s| !s.is_empty())
            .map(|s| String::from_utf8_lossy(s).into_owned()))
    }

    fn window_title(&self, window: u32) -> Result<Option<String>, PollError> {
        let reply = self
            .conn
            .get_property(false, window, self.net_wm_name, self.utf8_string, 0, 1024)
            .map_err(|e| PollError::Backend(e.to_string()))?
            .reply()
            .map_err(|e| PollError::Backend(e.to_string()))?;
        if reply.value.is_empty() {
            return Ok(None);
        }
        Ok(Some(String::from_utf8_lossy(&reply.value).into_owned()))
    }
}

fn intern_atom(conn: &RustConnection, name: &[u8]) -> Result<u32, PollError> {
    Ok(conn
        .intern_atom(false, name)
        .map_err(|e| PollError::Backend(e.to_string()))?
        .reply()
        .map_err(|e| PollError::Backend(e.to_string()))?
        .atom)
}

impl ObservationSource for ActiveWindowSource {
    fn id(&self) -> &str {
        "linux.active_window"
    }

    fn min_privacy_level(&self) -> PrivacyLevel {
        PrivacyLevel::ApplicationMetadata
    }

    fn poll(&mut self) -> Result<Vec<CapturedSignal>, PollError> {
        let window = self.focused_window()?;
        // Window id 0 / 1 (PointerRoot, None) means no window currently has
        // focus (e.g. focus is on the root window) — nothing to report.
        if window <= 1 {
            return Ok(Vec::new());
        }

        let mut signals = Vec::new();
        let class = self.wm_class(window)?;
        let title = self.window_title(window)?;

        let window_changed = self.last_window != Some(window);
        if window_changed {
            if let Some(app) = &class {
                signals.push(CapturedSignal::new(
                    self.id(),
                    CapturedPayload::AppFocusChange {
                        app_identifier: app.clone(),
                    },
                ));
            }
        }

        let title_changed = self.last_title.as_deref() != title.as_deref();
        if title_changed {
            if let Some(t) = &title {
                signals.push(CapturedSignal::new(
                    self.id(),
                    CapturedPayload::WindowTitle { title: t.clone() },
                ));
            }
        }

        self.last_window = Some(window);
        self.last_class = class;
        self.last_title = title;

        Ok(signals)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // These connect to and query the REAL X11 display available in this
    // environment (`DISPLAY=:0`, provided by WSLg) — not a mock. If no X server is
    // reachable, `ActiveWindowSource::connect` fails and these tests fail loudly
    // rather than silently no-op, so a CI runner without a display should skip
    // this crate's tests explicitly rather than rely on them passing vacuously.

    #[test]
    fn connects_to_the_real_display_and_reads_a_focused_window() {
        let mut source = ActiveWindowSource::connect().expect("connect to X11 display");
        // Whatever currently has input focus, this must not error — that's the
        // real assertion here (correctness of the X11 request/response handling),
        // not a specific app being focused, which this test can't control.
        let signals = source
            .poll()
            .expect("poll should succeed against a live display");
        // First poll may or may not yield signals depending on whether a window
        // currently has focus at all in this session; either is valid.
        for signal in &signals {
            assert_eq!(signal.source_id, "linux.active_window");
        }
    }

    #[test]
    fn second_poll_with_no_focus_change_yields_no_duplicate_signals() {
        let mut source = ActiveWindowSource::connect().expect("connect to X11 display");
        let _first = source.poll().expect("first poll");
        let second = source.poll().expect("second poll");
        // With nothing changing between two immediate polls, there should be
        // nothing new to report.
        assert!(second.is_empty());
    }

    #[test]
    fn reports_correct_minimum_privacy_level() {
        let source = ActiveWindowSource::connect().expect("connect to X11 display");
        assert_eq!(
            source.min_privacy_level(),
            PrivacyLevel::ApplicationMetadata
        );
    }
}
