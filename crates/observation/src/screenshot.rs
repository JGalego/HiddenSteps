use hiddensteps_domain::{CapturedPayload, CapturedSignal, PrivacyLevel};
use xcap::Monitor;

use crate::{ObservationSource, PollError};

/// Captures a full-screen screenshot and hands it to the Event Pipeline as
/// `CapturedPayload::Screenshot`, per `docs/design/05-privacy-model.md` §1's
/// Level 4 ("Maximum assistance" / Deep-mode). The pipeline's `TextExtractor`
/// stage (an OCR engine — see `hiddensteps_pipeline::OcrsTextExtractor`) turns
/// the captured bytes into redactable text before anything is summarized or
/// persisted; this source itself never touches text, redaction, or storage —
/// it only ever produces the raw, pre-redaction bytes ADR-0006 requires stay
/// ephemeral.
///
/// One implementation shared across Linux/macOS/Windows, via
/// [`xcap`](https://crates.io/crates/xcap) (v0.9, actively maintained,
/// re-exports `image`), rather than three hand-rolled per-OS backends: each
/// platform's native screenshot API (X11 `GetImage`, Core Graphics
/// `CGDisplayCreateImage`, GDI `BitBlt`) has enough pixel-format/byte-order
/// footguns (padding, channel order, DPI scaling) that reproducing it by hand
/// three times is materially riskier than depending on one maintained crate
/// that already handles it for all three. Verified for real in this
/// environment against the same Xvfb X11 display this crate's other Linux
/// sources are tested against (see this module's tests) — the macOS/Windows
/// code paths inside `xcap` itself are not exercised here, same disclosed gap
/// as this crate's other non-Linux code (see `lib.rs`'s doc comment).
///
/// `xcap`'s Linux backend unconditionally depends on `pipewire`/`zbus`/`gbm`/
/// `wayland-client` (screen-recording support this source never uses, since it
/// only ever calls `capture_image`, not the video recorder) — building this
/// crate on Linux therefore needs a handful of extra `-dev` system packages
/// beyond what the rest of this workspace requires (see
/// `../../README.md` and `.github/workflows/ci.yml`'s `core` job).
///
/// PNG-encoding the captured image (rather than storing a raw pixel buffer) is
/// deliberate, not a style choice: `CapturedPayload::Screenshot` carries only
/// `raw_bytes: Vec<u8>`, with no companion width/height field, so whatever
/// this source puts there must be self-describing — a `TextExtractor` decoding
/// it later has no other source of the image's dimensions.
pub struct ScreenshotSource;

impl ScreenshotSource {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ScreenshotSource {
    fn default() -> Self {
        Self::new()
    }
}

impl ObservationSource for ScreenshotSource {
    fn id(&self) -> &str {
        "deep_mode.screenshot"
    }

    fn min_privacy_level(&self) -> PrivacyLevel {
        PrivacyLevel::MaximumAssistance
    }

    fn poll(&mut self) -> Result<Vec<CapturedSignal>, PollError> {
        let monitors = Monitor::all().map_err(|e| PollError::Backend(e.to_string()))?;
        // Prefer whichever monitor `xcap` reports as primary; fall back to the
        // first one enumerated if none is marked primary (observed for real
        // against Xvfb's single virtual display, which reports `is_primary() ==
        // false` — a headless/virtual display legitimately has no primary
        // monitor concept, not a bug in this fallback).
        let monitor = monitors
            .iter()
            .find(|m| m.is_primary().unwrap_or(false))
            .or_else(|| monitors.first());
        let Some(monitor) = monitor else {
            return Ok(Vec::new());
        };

        let image = monitor
            .capture_image()
            .map_err(|e| PollError::Backend(e.to_string()))?;
        let mut png_bytes = Vec::new();
        image
            .write_to(
                &mut std::io::Cursor::new(&mut png_bytes),
                xcap::image::ImageFormat::Png,
            )
            .map_err(|e| PollError::Backend(e.to_string()))?;

        Ok(vec![CapturedSignal::new(
            self.id(),
            CapturedPayload::Screenshot {
                raw_bytes: png_bytes,
            },
        )])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_correct_minimum_privacy_level() {
        let source = ScreenshotSource::new();
        assert_eq!(source.min_privacy_level(), PrivacyLevel::MaximumAssistance);
    }

    /// Exercised against the real Xvfb X11 display this crate's other Linux
    /// sources use — not a mock. Confirms `xcap` actually captures a
    /// non-trivial, self-describing (PNG) image in this environment, which is
    /// the one thing about this source a compiler can't check on its own
    /// (wrong dimension order, wrong pixel format, an encoder silently writing
    /// nothing).
    #[test]
    fn captures_a_real_screenshot_and_encodes_it_as_png() {
        let mut source = ScreenshotSource::new();
        let signals = source
            .poll()
            .expect("poll should succeed against a real display");
        assert_eq!(signals.len(), 1);
        match &signals[0].payload {
            CapturedPayload::Screenshot { raw_bytes } => {
                // A real captured frame PNG-encodes to well more than a few
                // bytes; this also incidentally confirms it's non-empty and
                // that encoding didn't silently no-op.
                assert!(
                    raw_bytes.len() > 100,
                    "expected a real PNG, got {} bytes",
                    raw_bytes.len()
                );
                // PNG magic bytes: a cheap, real check that this is actually
                // PNG-encoded, not raw pixels or some other format.
                assert_eq!(&raw_bytes[..8], b"\x89PNG\r\n\x1a\n");
            }
            other => panic!("expected Screenshot payload, got {other:?}"),
        }
    }

    #[test]
    fn source_id_matches_the_documented_signal_source() {
        let source = ScreenshotSource::new();
        assert_eq!(source.id(), "deep_mode.screenshot");
    }
}
