use hiddensteps_domain::{CapturedPayload, CapturedSignal, PrivacyLevel};
use windows::Win32::Foundation::HGLOBAL;
use windows::Win32::System::DataExchange::{
    CloseClipboard, EnumClipboardFormats, GetClipboardData, GetClipboardFormatNameW,
    GetClipboardSequenceNumber, OpenClipboard,
};
use windows::Win32::System::Memory::GlobalSize;

use crate::{ObservationSource, PollError};

/// See this module's parent doc comment (`windows/mod.rs`) — written against
/// stable Win32 APIs but not compiled or tested; no Windows toolchain was
/// available in the environment this was written in.
///
/// Reports clipboard *metadata* only — content type and byte size — never
/// clipboard content, per `docs/design/05-privacy-model.md` §1's Level 2 signal
/// definition. `GlobalSize` measures the length of the data handle
/// `GetClipboardData` returns without ever locking or reading through it, so
/// no clipboard content is inspected at any point — the same discipline
/// `linux::ClipboardMetadataSource` applies to its X11 selection transfer.
pub struct ClipboardMetadataSource {
    last_sequence: u32,
}

impl ClipboardMetadataSource {
    pub fn new() -> Self {
        Self { last_sequence: 0 }
    }

    /// The human-readable name of a clipboard format, if the system or the
    /// current owner registered one (`GetClipboardFormatNameW` only resolves
    /// names for registered/custom formats). Standard formats (plain text,
    /// bitmaps, ...) have no such name — those are reported as `format:<id>`
    /// rather than guessing at a hardcoded `CF_*` id-to-name table.
    fn format_name(format: u32) -> String {
        let mut buf = [0u16; 256];
        // SAFETY: `buf` is a valid, appropriately-sized mutable buffer for the
        // duration of this call, per `GetClipboardFormatNameW`'s contract; the
        // clipboard is already open in every caller of this function.
        let len = unsafe { GetClipboardFormatNameW(format, &mut buf) };
        if len > 0 {
            String::from_utf16_lossy(&buf[..len as usize])
        } else {
            format!("format:{format}")
        }
    }
}

impl Default for ClipboardMetadataSource {
    fn default() -> Self {
        Self::new()
    }
}

impl ObservationSource for ClipboardMetadataSource {
    fn id(&self) -> &str {
        "windows.clipboard_metadata"
    }

    fn min_privacy_level(&self) -> PrivacyLevel {
        PrivacyLevel::WorkflowMetadata
    }

    fn poll(&mut self) -> Result<Vec<CapturedSignal>, PollError> {
        // SAFETY: takes no arguments and only reads a counter Windows
        // maintains internally; safe to call regardless of clipboard state.
        let sequence = unsafe { GetClipboardSequenceNumber() };
        if sequence == self.last_sequence {
            return Ok(Vec::new());
        }
        self.last_sequence = sequence;

        // SAFETY: `OpenClipboard(None)` requests clipboard access for the
        // current thread on behalf of no particular window; every path below
        // closes it exactly once via the matching `CloseClipboard` call before
        // returning.
        if unsafe { OpenClipboard(None) }.is_err() {
            return Ok(Vec::new());
        }

        // SAFETY: `0` is the documented "start enumeration" value for
        // `EnumClipboardFormats`; the clipboard is open for the duration of
        // this call per the guard above.
        let format = unsafe { EnumClipboardFormats(0) };
        let payload = if format == 0 {
            None
        } else {
            let content_type = Self::format_name(format);
            // SAFETY: `format` was just returned by `EnumClipboardFormats`
            // above, so it names a format the current clipboard owner
            // actually offers.
            unsafe { GetClipboardData(format) }.ok().map(|data| {
                // SAFETY: `data` is the handle `GetClipboardData` just
                // returned; `GlobalSize` only measures its length and never
                // locks or reads through it.
                let size_bytes = unsafe { GlobalSize(HGLOBAL(data.0)) };
                CapturedPayload::ClipboardMetadata {
                    content_type,
                    size_bytes,
                }
            })
        };

        // SAFETY: matches the `OpenClipboard` call above; must run regardless
        // of whether a format was found.
        let _ = unsafe { CloseClipboard() };

        Ok(payload
            .map(|payload| vec![CapturedSignal::new(self.id(), payload)])
            .unwrap_or_default())
    }
}

// No #[cfg(test)] module here — see this module's parent doc comment for why:
// without a Windows machine to run them on, tests here would assert behavior
// of code nobody has executed, which is worse than no tests at all.
