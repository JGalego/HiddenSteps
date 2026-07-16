//! Linux `ObservationSource` implementations. `active_window`, `file_ops`, and
//! `clipboard` are exercised against real backends (a live X11 display via WSLg,
//! and real inotify) in their own test modules. `shortcuts` is real but
//! deliberately not exercised automatically — see its module doc comment.

mod active_window;
mod clipboard;
mod file_ops;
mod shortcuts;

pub use active_window::ActiveWindowSource;
pub use clipboard::ClipboardMetadataSource;
pub use file_ops::FileOperationSource;
pub use shortcuts::{GlobalShortcutSource, ShortcutBinding};
