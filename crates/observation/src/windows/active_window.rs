use hiddensteps_domain::{CapturedPayload, CapturedSignal, PrivacyLevel};
use windows::core::PWSTR;
use windows::Win32::Foundation::{CloseHandle, HWND, MAX_PATH};
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetWindowTextW, GetWindowThreadProcessId,
};

use crate::{ObservationSource, PollError};

/// See this module's parent doc comment (`windows/mod.rs`) — written against
/// stable Win32 APIs but not compiled or tested; no Windows toolchain was
/// available in the environment this was written in.
pub struct ActiveWindowSource {
    last_hwnd: Option<isize>,
    last_process_path: Option<String>,
    last_title: Option<String>,
}

impl ActiveWindowSource {
    pub fn new() -> Self {
        Self {
            last_hwnd: None,
            last_process_path: None,
            last_title: None,
        }
    }

    fn window_title(hwnd: HWND) -> Option<String> {
        let mut buf = [0u16; 512];
        // SAFETY: `buf` is a valid, appropriately-sized mutable buffer for the
        // duration of this call, per `GetWindowTextW`'s contract.
        let len = unsafe { GetWindowTextW(hwnd, &mut buf) };
        if len <= 0 {
            return None;
        }
        Some(String::from_utf16_lossy(&buf[..len as usize]))
    }

    /// The owning process's executable path — used as the stable app identifier
    /// (analogous to `WM_CLASS` on Linux, `bundleIdentifier` on macOS).
    fn owning_process_path(hwnd: HWND) -> Option<String> {
        let mut pid = 0u32;
        // SAFETY: `hwnd` came from `GetForegroundWindow`; `pid` is a valid
        // out-pointer for the duration of this call.
        unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
        if pid == 0 {
            return None;
        }

        // SAFETY: `pid` is a process id obtained above; the handle is closed
        // below before returning.
        let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }.ok()?;

        let mut buf = [0u16; MAX_PATH as usize];
        let mut size = buf.len() as u32;
        // SAFETY: `handle` is a valid, open process handle; `buf`/`size` describe
        // a valid mutable buffer of that length.
        let result = unsafe {
            QueryFullProcessImageNameW(
                handle,
                PROCESS_NAME_WIN32,
                PWSTR(buf.as_mut_ptr()),
                &mut size,
            )
        };
        // SAFETY: `handle` was returned by a successful `OpenProcess` above and
        // has not been closed yet.
        unsafe {
            let _ = CloseHandle(handle);
        }

        if result.is_err() {
            return None;
        }
        Some(String::from_utf16_lossy(&buf[..size as usize]))
    }
}

impl Default for ActiveWindowSource {
    fn default() -> Self {
        Self::new()
    }
}

impl ObservationSource for ActiveWindowSource {
    fn id(&self) -> &str {
        "windows.active_window"
    }

    fn min_privacy_level(&self) -> PrivacyLevel {
        PrivacyLevel::ApplicationMetadata
    }

    fn poll(&mut self) -> Result<Vec<CapturedSignal>, PollError> {
        // SAFETY: `GetForegroundWindow` takes no arguments and always returns
        // either a valid HWND or NULL (no foreground window); there is nothing
        // unsafe about the call itself beyond it being an FFI boundary.
        let hwnd = unsafe { GetForegroundWindow() };
        if hwnd.0.is_null() {
            return Ok(Vec::new());
        }

        let mut signals = Vec::new();
        let hwnd_value = hwnd.0 as isize;
        let window_changed = self.last_hwnd != Some(hwnd_value);

        if window_changed {
            let process_path = Self::owning_process_path(hwnd);
            if let Some(path) = &process_path {
                signals.push(CapturedSignal::new(
                    self.id(),
                    CapturedPayload::AppFocusChange {
                        app_identifier: path.clone(),
                    },
                ));
            }
            self.last_process_path = process_path;
        }

        let title = Self::window_title(hwnd);
        if title.as_deref() != self.last_title.as_deref() {
            if let Some(t) = &title {
                signals.push(CapturedSignal::new(
                    self.id(),
                    CapturedPayload::WindowTitle { title: t.clone() },
                ));
            }
        }

        self.last_hwnd = Some(hwnd_value);
        self.last_title = title;

        Ok(signals)
    }
}

// No #[cfg(test)] module here: without a Windows machine to run them on, tests
// asserting real GetForegroundWindow/GetWindowTextW behavior would be untestable
// assertions about code nobody has run — worse than no tests, since they'd imply
// a verification that didn't happen. Real tests belong here once this is built
// and run on Windows for the first time.
