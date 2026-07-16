//! macOS `ObservationSource` implementation, against `CGWindowListCopyWindowInfo`
//! (Core Graphics), per `docs/roadmap/02-technology-choices.md`.
//!
//! **Disclosure, not a caveat to skim past**: this module was written without
//! access to a macOS toolchain, SDK, or Objective-C/Core Foundation runtime —
//! this Linux sandbox has none of those. It has **not been compiled, run, or
//! tested**. `CGWindowListCopyWindowInfo` itself is a long-stable API (available
//! since Mac OS X 10.5, still current), which is why it's the API chosen here
//! over the more powerful but more invasive/permission-heavy Accessibility
//! (`AXUIElement`) APIs for this Level 1/2 use case — but the Core Foundation
//! FFI/type-bridging code around it (`CFArray`/`CFDictionary`/`CFString` handling
//! via the `core-foundation` crate) is exactly the kind of thing that can have
//! subtle signature mismatches a compiler would catch immediately and this review
//! cannot. Treat this as a strong first draft to compile, fix, and verify on a
//! real Mac before it ships.
//!
//! Deliberately out of scope here (a real, disclosed gap, not a silent one):
//! Accessibility-API-based richer context for privacy Level 3 (`AXUIElement`
//! attribute queries), which requires the user to have granted the Accessibility
//! permission and is materially more involved than window-list enumeration. This
//! module covers Level 1/2 (frontmost app + window title) only.

mod active_window;

pub use active_window::ActiveWindowSource;
