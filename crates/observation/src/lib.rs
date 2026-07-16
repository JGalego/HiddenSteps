//! `ObservationSource` implementations, per ADR-0005: one source per
//! platform/signal-type pair, each declaring the minimum privacy level it may run
//! at.
//!
//! Platform coverage in this crate, and why:
//!
//! - **Linux** (`linux` module): implemented and tested against a real X11 display
//!   — this dev environment (WSLg) actually has one (`DISPLAY=:0`), so
//!   [`linux::ActiveWindowSource`] and [`linux::FileOperationSource`] are exercised
//!   against real X11/inotify in their test suites, not mocks.
//! - **macOS** (`macos` module, `#[cfg(target_os = "macos")]`): written against the
//!   real APIs named in `docs/roadmap/02-technology-choices.md` (`NSWorkspace`,
//!   `AXUIElement`, `CGWindowListCopyWindowInfo`), but **not compiled or tested in
//!   this environment** — there is no macOS toolchain/SDK here. `cfg`-gating means
//!   it simply isn't part of a Linux build, which is the correct, honest way to
//!   exclude platform code this environment can't verify, as opposed to leaving a
//!   `todo!()` stub in its place.
//! - **Windows** (`windows` module, `#[cfg(target_os = "windows")]`): same
//!   situation, against Win32 (`GetForegroundWindow`, `SetWinEventHook`) and UI
//!   Automation.
//!
//! Two signal sources named in `docs/design/05-privacy-model.md` §1 are
//! deliberately **not** implemented anywhere in this crate yet, and that gap is
//! disclosed rather than papered over with a stub:
//!
//! - **Browser domain visited** — there is no OS-level API for "which domain is
//!   the foreground browser tab showing"; this signal requires a browser-extension
//!   component (a distinct artifact — WebExtension code, a store listing, its own
//!   update channel) that hasn't been built. What *is* real here is the
//!   consumption side: any `ObservationSource` (including a future extension
//!   bridge) that can produce a `CapturedPayload::BrowserDomainVisited` plugs into
//!   the same `ObservationSource`/pipeline path as everything else.
//! - **Global keyboard shortcut capture** — implementing this on Linux means an
//!   X11 global key grab (`XGrabKey`), which intercepts that key combo
//!   session-wide. Installing one automatically in a shared development sandbox
//!   would risk interfering with whoever/whatever else is using this X session.
//!   The correct real implementation is written in `linux::shortcuts`, but it is
//!   opt-in (never auto-started) and its integration test is `#[ignore]`d for the
//!   same "don't do this by default in a shared environment" reason the real
//!   OS-vault test in `hiddensteps-security` is.

mod source;

pub use source::{ObservationSource, PollError};

#[cfg(target_os = "linux")]
pub mod linux;

#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(target_os = "windows")]
pub mod windows;
