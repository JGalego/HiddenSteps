//! A real `TextExtractor` (`crate::pipeline::TextExtractor`) using
//! [`ocrs`](https://crates.io/crates/ocrs) — a pure-Rust OCR engine built on the
//! [`rten`](https://crates.io/crates/rten) ML runtime.
//!
//! Chosen over shelling out to a system Tesseract binary (e.g. via `leptess`)
//! specifically because it's pure Rust: it cross-compiles the identical way on
//! every OS in `.github/workflows/ci.yml`'s `core` matrix
//! (Linux/macOS/Windows), with no `apt`/`brew`/`choco` step and no differing
//! system-library discovery per platform — unlike a Tesseract binding, which
//! needs a system Tesseract install (and its language data files) present and
//! discoverable differently on each of those three.

use std::path::Path;

use ocrs::{ImageSource, OcrEngine, OcrEngineParams};
use rten::Model;

use crate::pipeline::TextExtractor;

/// Errors constructing an [`OcrsTextExtractor`] — a missing/corrupt model file,
/// or the underlying engine failing to initialize from otherwise-loaded
/// models. Distinct from `TextExtractor::extract`'s per-call failure mode
/// (`None`): a broken model file is a setup-time problem worth surfacing once,
/// not something that should look like "this particular screenshot had no
/// text."
#[derive(Debug, thiserror::Error)]
pub enum OcrExtractorError {
    #[error("failed to load the OCR {kind} model from {path}: {source}")]
    ModelLoad {
        kind: &'static str,
        path: String,
        #[source]
        source: rten::LoadError,
    },
    #[error("failed to initialize the OCR engine: {0}")]
    EngineInit(String),
}

/// A real, pure-Rust [`TextExtractor`] for Level 4 ("Maximum assistance" /
/// Deep-mode) screenshot content, per `docs/design/05-privacy-model.md` §1.
///
/// This type deliberately does **not** fetch its own model files. `ocrs`'s own
/// CLI downloads its two `.rten` models (detection + recognition) from a fixed
/// host (`ocrs-models.s3-accelerate.amazonaws.com`) on first use, but doing
/// that inside this crate would give a crate that otherwise makes no network
/// call of any kind (`hiddensteps-pipeline` is pure Classify/Redact/Summarize
/// logic) a hidden runtime network dependency. Instead, [`Self::from_model_files`]
/// takes paths to already-present `.rten` files; provisioning them (checking a
/// local cache, downloading if absent) is the desktop shell's job — see
/// `apps/desktop/src-tauri/src/ocr_models.rs`.
pub struct OcrsTextExtractor {
    engine: OcrEngine,
}

impl OcrsTextExtractor {
    pub fn from_model_files(
        detection_model_path: &Path,
        recognition_model_path: &Path,
    ) -> Result<Self, OcrExtractorError> {
        let detection_model = Model::load_file(detection_model_path).map_err(|source| {
            OcrExtractorError::ModelLoad {
                kind: "detection",
                path: detection_model_path.display().to_string(),
                source,
            }
        })?;
        let recognition_model = Model::load_file(recognition_model_path).map_err(|source| {
            OcrExtractorError::ModelLoad {
                kind: "recognition",
                path: recognition_model_path.display().to_string(),
                source,
            }
        })?;
        let engine = OcrEngine::new(OcrEngineParams {
            detection_model: Some(detection_model),
            recognition_model: Some(recognition_model),
            ..Default::default()
        })
        .map_err(|e| OcrExtractorError::EngineInit(e.to_string()))?;
        Ok(Self { engine })
    }
}

/// Decodes `raw_bytes` — the PNG-encoded screenshot an `ObservationSource`
/// produces (see `hiddensteps_domain::CapturedPayload::Screenshot`'s doc
/// comment on why it must be self-describing) — into the RGB buffer `ocrs`
/// consumes. Split out from `extract` so the "malformed/non-image bytes are
/// handled gracefully, not a panic" behavior is testable on its own, without a
/// real (multi-megabyte, network-fetched) OCR engine behind it.
fn decode_to_rgb(raw_bytes: &[u8]) -> Option<image::RgbImage> {
    image::load_from_memory(raw_bytes)
        .ok()
        .map(|img| img.into_rgb8())
}

impl TextExtractor for OcrsTextExtractor {
    fn extract(&self, raw_bytes: &[u8]) -> Option<String> {
        let rgb = decode_to_rgb(raw_bytes)?;
        let (width, height) = rgb.dimensions();
        let source = ImageSource::from_bytes(rgb.as_raw(), (width, height)).ok()?;
        let input = self.engine.prepare_input(source).ok()?;
        let text = self.engine.get_text(&input).ok()?;
        let trimmed = text.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode_png(img: &image::RgbImage) -> Vec<u8> {
        let mut bytes = Vec::new();
        image::DynamicImage::ImageRgb8(img.clone())
            .write_to(
                &mut std::io::Cursor::new(&mut bytes),
                image::ImageFormat::Png,
            )
            .expect("encoding a freshly-created image to PNG should never fail");
        bytes
    }

    #[test]
    fn decode_to_rgb_returns_none_for_non_image_bytes() {
        assert!(decode_to_rgb(b"this is not an image").is_none());
    }

    #[test]
    fn decode_to_rgb_returns_none_for_empty_bytes() {
        assert!(decode_to_rgb(&[]).is_none());
    }

    #[test]
    fn decode_to_rgb_decodes_a_real_png_with_correct_dimensions() {
        let img = image::RgbImage::from_pixel(3, 5, image::Rgb([10, 20, 30]));
        let png_bytes = encode_png(&img);
        let decoded = decode_to_rgb(&png_bytes).expect("a real PNG should decode");
        assert_eq!(decoded.dimensions(), (3, 5));
        assert_eq!(decoded.get_pixel(0, 0), &image::Rgb([10, 20, 30]));
    }

    // The test below constructs a real `OcrEngine` from the real `.rten` model
    // files `ocrs` publishes, and OCRs a real image with known text rendered
    // into it via a common system font. There is no bundled/fixture model or
    // font in this repo (a real detection+recognition model pair is
    // multi-megabyte binary data, and a bundled font/fixture image raises its
    // own licensing/provenance questions), so this test downloads the models
    // on first run from the same fixed host `ocrs`'s own CLI uses
    // (`ocrs-models.s3-accelerate.amazonaws.com`), caches them in the OS temp
    // directory across runs, and renders its test image from whichever common
    // system font it finds already installed (skipping, not failing, if none
    // is found — e.g. a minimal CI image with no fonts at all). That network
    // dependency is exactly why this is `#[ignore]`d rather than part of the
    // default `cargo test` run, matching this repo's existing convention for
    // tests that need a real environment the automated run doesn't have (see
    // `linux::shortcuts`' and `hiddensteps-security`'s `#[ignore]`d
    // real-environment tests, and `hiddensteps-llm-provider`'s real-Ollama
    // tests).
    //
    // Actually run during development, against the real models and a real
    // DejaVu Sans Bold rendering of the word "HIDDENSTEPS": the engine
    // recognized it verbatim.
    #[test]
    #[ignore = "downloads real OCR model files over the network on first run; \
                run manually with `cargo test -p hiddensteps-pipeline -- --ignored recognizes_real_text`"]
    fn recognizes_real_text() {
        let cache_dir = std::env::temp_dir().join("hiddensteps-ocr-test-models");
        std::fs::create_dir_all(&cache_dir).expect("create test model cache dir");
        let detection_path = cache_dir.join("text-detection.rten");
        let recognition_path = cache_dir.join("text-recognition.rten");
        download_if_missing(
            &detection_path,
            "https://ocrs-models.s3-accelerate.amazonaws.com/text-detection.rten",
        );
        download_if_missing(
            &recognition_path,
            "https://ocrs-models.s3-accelerate.amazonaws.com/text-recognition.rten",
        );

        let extractor = OcrsTextExtractor::from_model_files(&detection_path, &recognition_path)
            .expect("loading the real OCR models should succeed");

        let Some(png_bytes) = render_known_text_image("HIDDENSTEPS") else {
            eprintln!(
                "skipping recognizes_real_text: no common system font found to render the test image \
                 (see this test's doc comment) — this environment can't exercise real recognition accuracy"
            );
            return;
        };

        let text = extractor
            .extract(&png_bytes)
            .expect("a clearly-rendered word should recognize as some text");
        assert!(
            text.to_uppercase().contains("HIDDENSTEPS"),
            "expected recognized text to contain HIDDENSTEPS, got: {text:?}"
        );
    }

    /// Renders `text` into a plain white image using the first common system
    /// font this finds already installed, PNG-encodes it, and returns the
    /// bytes — or `None` if no such font is present (this environment's fonts
    /// aren't this test's concern; it isn't run automatically at all, see
    /// its `#[ignore]` reason).
    #[cfg(test)]
    fn render_known_text_image(text: &str) -> Option<Vec<u8>> {
        const CANDIDATE_FONT_PATHS: &[&str] = &[
            // Common on Debian/Ubuntu (the `fonts-dejavu-core` package).
            "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
            // macOS.
            "/System/Library/Fonts/Supplemental/Arial Bold.ttf",
            // Windows.
            "C:\\Windows\\Fonts\\arialbd.ttf",
        ];
        let font_bytes = CANDIDATE_FONT_PATHS
            .iter()
            .find_map(|path| std::fs::read(path).ok())?;
        let font = ab_glyph::FontRef::try_from_slice(&font_bytes).ok()?;

        let mut img = image::RgbImage::from_pixel(500, 120, image::Rgb([255, 255, 255]));
        imageproc::drawing::draw_text_mut(
            &mut img,
            image::Rgb([0, 0, 0]),
            10,
            20,
            60.0,
            &font,
            text,
        );
        let bytes = encode_png(&img);
        Some(bytes)
    }

    #[cfg(test)]
    fn download_if_missing(path: &std::path::Path, url: &str) {
        if path.exists() {
            return;
        }
        let bytes = reqwest::blocking::get(url)
            .expect("network request for test OCR model")
            .bytes()
            .expect("reading test OCR model response body");
        std::fs::write(path, &bytes).expect("writing test OCR model to cache dir");
    }
}
