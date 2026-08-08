//! Provisions the two `.rten` model files
//! `hiddensteps_pipeline::OcrsTextExtractor` needs (detection + recognition),
//! by downloading them once from the `ocrs` project's own model host and
//! caching them in this app's data directory.
//!
//! This lives here, not in `hiddensteps-pipeline`, deliberately:
//! `OcrsTextExtractor::from_model_files` only ever reads already-present local
//! files — see that type's doc comment for why a supposedly pure-logic crate
//! (Classify/Redact/Summarize, no network I/O anywhere else in it) shouldn't
//! gain a hidden runtime network dependency. Fetching those files is instead
//! this desktop shell's job, the same way it already owns the OS credential
//! vault, the encrypted database file, and every other piece of real-world I/O
//! the core crates deliberately stay agnostic of.
//!
//! Deliberately **not** attempted unconditionally at every launch: `observation_loop::run`
//! only calls this when the user is already at Level 4 with the screenshot+OCR
//! sub-capability turned on (see `commands::DEEP_MODE_SCREENSHOT_OCR_SETTING_KEY`) —
//! a user who never touches Deep-mode should never see this app make an
//! unsolicited network call to a third-party host, even a one-time, cached one.

use std::path::{Path, PathBuf};

const DETECTION_MODEL_URL: &str =
    "https://ocrs-models.s3-accelerate.amazonaws.com/text-detection.rten";
const RECOGNITION_MODEL_URL: &str =
    "https://ocrs-models.s3-accelerate.amazonaws.com/text-recognition.rten";

/// Ensures both model files exist under `cache_dir`, downloading whichever is
/// missing, and returns their paths. A file already present is trusted as-is
/// and never re-downloaded — there is no version/hash check, since `ocrs`
/// itself publishes these two fixed files at fixed URLs with no versioning
/// scheme of its own to check against.
pub async fn ensure_ocr_models(cache_dir: &Path) -> Result<(PathBuf, PathBuf), String> {
    std::fs::create_dir_all(cache_dir)
        .map_err(|e| format!("creating OCR model cache dir {}: {e}", cache_dir.display()))?;
    let detection_path = cache_dir.join("text-detection.rten");
    let recognition_path = cache_dir.join("text-recognition.rten");
    download_if_missing(&detection_path, DETECTION_MODEL_URL).await?;
    download_if_missing(&recognition_path, RECOGNITION_MODEL_URL).await?;
    Ok((detection_path, recognition_path))
}

/// Downloads to a `.part` sibling file first and renames it into place only on
/// full success — so a connection dropped mid-download can never leave a
/// truncated file at `path` that a later run would mistake for a real,
/// complete model (this function's only "is it already there" check is
/// `path.exists()`).
async fn download_if_missing(path: &Path, url: &str) -> Result<(), String> {
    if path.exists() {
        return Ok(());
    }
    let response = reqwest::get(url)
        .await
        .map_err(|e| format!("requesting OCR model from {url}: {e}"))?
        .error_for_status()
        .map_err(|e| format!("OCR model host returned an error for {url}: {e}"))?;
    let bytes = response
        .bytes()
        .await
        .map_err(|e| format!("reading OCR model response body from {url}: {e}"))?;
    let tmp_path = path.with_extension("rten.part");
    std::fs::write(&tmp_path, &bytes).map_err(|e| {
        format!(
            "writing downloaded OCR model to {}: {e}",
            tmp_path.display()
        )
    })?;
    std::fs::rename(&tmp_path, path)
        .map_err(|e| format!("finalizing downloaded OCR model at {}: {e}", path.display()))?;
    Ok(())
}
