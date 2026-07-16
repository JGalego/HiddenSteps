use core_foundation::array::CFArray;
use core_foundation::base::TCFType;
use core_foundation::dictionary::CFDictionary;
use core_foundation::number::CFNumber;
use core_foundation::string::CFString;

use hiddensteps_domain::{CapturedPayload, CapturedSignal, PrivacyLevel};

use crate::{ObservationSource, PollError};

#[allow(non_upper_case_globals)]
const kCGWindowListOptionOnScreenOnly: u32 = 1 << 0;
#[allow(non_upper_case_globals)]
const kCGWindowListExcludeDesktopElements: u32 = 1 << 4;
#[allow(non_upper_case_globals)]
const kCGNullWindowID: u32 = 0;
/// Normal application windows report layer 0; anything else (menu bar items,
/// desktop icons, the Dock) reports a non-zero layer — this is how the frontmost
/// *application* window is distinguished from other on-screen surfaces.
const NORMAL_WINDOW_LAYER: i64 = 0;

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGWindowListCopyWindowInfo(
        option: u32,
        relative_to_window: u32,
    ) -> core_foundation::array::CFArrayRef;
}

/// See this module's parent doc comment (`macos/mod.rs`) — written against the
/// stable `CGWindowListCopyWindowInfo` API but not compiled or tested; no macOS
/// toolchain was available in the environment this was written in.
pub struct ActiveWindowSource {
    last_owner: Option<String>,
    last_title: Option<String>,
}

impl ActiveWindowSource {
    pub fn new() -> Self {
        Self {
            last_owner: None,
            last_title: None,
        }
    }

    /// Returns `(owner_name, window_title)` for the frontmost normal-layer
    /// on-screen window, if any. `CGWindowListCopyWindowInfo` returns windows
    /// ordered front-to-back, so the first layer-0 entry is the one we want.
    fn frontmost_window() -> Option<(String, Option<String>)> {
        // SAFETY: both arguments are valid per `CGWindowListCopyWindowInfo`'s
        // documented contract (a bitmask of list options, and
        // `kCGNullWindowID`/0 meaning "not relative to any specific window");
        // the returned `CFArrayRef` is owned per the Core Foundation "Copy"
        // naming convention and is wrapped under the create rule immediately
        // below, which takes ownership correctly.
        let array_ref = unsafe {
            CGWindowListCopyWindowInfo(
                kCGWindowListOptionOnScreenOnly | kCGWindowListExcludeDesktopElements,
                kCGNullWindowID,
            )
        };
        if array_ref.is_null() {
            return None;
        }
        // SAFETY: `array_ref` is a valid, owned `CFArrayRef` of `CFDictionaryRef`
        // elements per `CGWindowListCopyWindowInfo`'s documented return type.
        let windows: CFArray<CFDictionary> = unsafe { CFArray::wrap_under_create_rule(array_ref) };

        let owner_key = CFString::from_static_string("kCGWindowOwnerName");
        let name_key = CFString::from_static_string("kCGWindowName");
        let layer_key = CFString::from_static_string("kCGWindowLayer");

        for window in windows.iter() {
            let layer = window
                .find(&layer_key)
                .and_then(|v| v.downcast::<CFNumber>())
                .and_then(|n| n.to_i64())
                .unwrap_or(-1);
            if layer != NORMAL_WINDOW_LAYER {
                continue;
            }
            let owner = window
                .find(&owner_key)
                .and_then(|v| v.downcast::<CFString>())
                .map(|s| s.to_string());
            let Some(owner) = owner else { continue };
            let title = window
                .find(&name_key)
                .and_then(|v| v.downcast::<CFString>())
                .map(|s| s.to_string());
            return Some((owner, title));
        }
        None
    }
}

impl Default for ActiveWindowSource {
    fn default() -> Self {
        Self::new()
    }
}

impl ObservationSource for ActiveWindowSource {
    fn id(&self) -> &str {
        "macos.active_window"
    }

    fn min_privacy_level(&self) -> PrivacyLevel {
        PrivacyLevel::ApplicationMetadata
    }

    fn poll(&mut self) -> Result<Vec<CapturedSignal>, PollError> {
        let Some((owner, title)) = Self::frontmost_window() else {
            return Ok(Vec::new());
        };

        let mut signals = Vec::new();

        if self.last_owner.as_deref() != Some(owner.as_str()) {
            signals.push(CapturedSignal::new(
                self.id(),
                CapturedPayload::AppFocusChange {
                    app_identifier: owner.clone(),
                },
            ));
        }
        if self.last_title.as_deref() != title.as_deref() {
            if let Some(t) = &title {
                signals.push(CapturedSignal::new(
                    self.id(),
                    CapturedPayload::WindowTitle { title: t.clone() },
                ));
            }
        }

        self.last_owner = Some(owner);
        self.last_title = title;

        Ok(signals)
    }
}

// No #[cfg(test)] module here — see this module's parent doc comment for why:
// without a Mac to run them on, tests here would assert behavior of code nobody
// has executed, which is worse than no tests at all.
