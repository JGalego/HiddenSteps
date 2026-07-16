//! Windows `ObservationSource` implementation, against Win32
//! (`GetForegroundWindow`, `GetWindowTextW`, `QueryFullProcessImageNameW`) via the
//! `windows` crate, per `docs/roadmap/02-technology-choices.md`.
//!
//! **Disclosure, not a caveat to skim past**: this module was written without
//! access to a Windows toolchain or SDK — this Linux sandbox has neither. It has
//! **not been compiled, run, or tested**, unlike every other module in this
//! crate. The Win32 functions used here (`GetForegroundWindow`,
//! `GetWindowThreadProcessId`, `GetWindowTextW`, `QueryFullProcessImageNameW`) are
//! long-stable, well-documented APIs unchanged across many Windows/SDK versions,
//! which is why this was still worth writing out in full rather than stubbing —
//! but the exact `windows` crate version pinned in `Cargo.toml` matters: that
//! crate's generated signatures (owned `String` vs. `&mut [u16]` buffers, `Result`
//! vs. raw integer returns) have changed across its own major versions. Treat this
//! as a strong first draft to compile, fix, and verify on a real Windows machine
//! before it ships — not as verified, working code.

mod active_window;

pub use active_window::ActiveWindowSource;
