//! Windows `ObservationSource` implementations, against Win32
//! (`GetForegroundWindow`, `GetWindowTextW`, `QueryFullProcessImageNameW`,
//! clipboard/`RegisterHotKey` APIs) plus the cross-platform `notify` crate for
//! file-operation watching, per `docs/roadmap/02-technology-choices.md`.
//!
//! **Disclosure, not a caveat to skim past**: `active_window`, `clipboard`, and
//! `shortcuts` were written without access to a Windows toolchain or SDK — this
//! Linux sandbox has neither. They have **not been compiled, run, or tested**,
//! unlike every other module in this crate. The Win32 functions used
//! (`GetForegroundWindow`, `GetWindowThreadProcessId`, `GetWindowTextW`,
//! `QueryFullProcessImageNameW`, the clipboard functions, `RegisterHotKey`) are
//! long-stable, well-documented APIs unchanged across many Windows/SDK versions,
//! which is why this was still worth writing out in full rather than stubbing —
//! but the exact `windows` crate version pinned in `Cargo.toml` matters: that
//! crate's generated signatures (owned `String` vs. `&mut [u16]` buffers, `Result`
//! vs. raw integer returns, `Option<HWND>` vs. bare `HWND` for nullable handles)
//! have changed across its own major versions. Treat these three as a strong
//! first draft to compile, fix, and verify on a real Windows machine before they
//! ship — not as verified, working code. `file_ops` carries meaningfully lower
//! risk: it has no hand-written Win32 FFI of its own, since `notify` owns the
//! platform-specific unsafe code underneath `ReadDirectoryChangesW`.

mod active_window;
mod clipboard;
mod file_ops;
mod shortcuts;

pub use active_window::ActiveWindowSource;
pub use clipboard::ClipboardMetadataSource;
pub use file_ops::FileOperationSource;
pub use shortcuts::{GlobalShortcutSource, ShortcutBinding};
