//! The Event Pipeline: Classify → Redact → Summarize, per ADR-0006.
//!
//! `EventPipeline::process` is the *only* way a `CapturedSignal` becomes an
//! `EventSummary` — there is no path around Redact, and there is no path that
//! writes a `CapturedSignal` to storage directly (`hiddensteps-event-store` has no
//! API that would accept one).
//!
//! Note on scope: Level 3 (ContextAware) in `docs/design/05-privacy-model.md` §1 is
//! described as "richer context" layered onto Level 2's signal types (fuller
//! in-app action context, browser page title, limited file-operation context)
//! rather than introducing wholly new signal types of its own. Browser page
//! title is no longer part of that gap: `SignalType::BrowserPageTitleViewed`
//! (produced by `hiddensteps_observation::BrowserBridgeSource`, the
//! browser-extension bridge) is mapped by `minimum_level_for` to
//! `PrivacyLevel::ContextAware` specifically, so it's the first signal type this
//! implementation distinguishes as exclusively Level 3. The rest of the gap is
//! still open: fuller in-app action context and file-operation extension detail
//! have no dedicated signal types yet, so a Level 3 user sees the same
//! `AppActionEvent`/`FileOperationMetadata` shapes Level 2 does for those two —
//! a deliberate, disclosed simplification, not a silent one.

mod classify;
mod ocr;
mod pipeline;

pub use classify::{minimum_level_for, FieldValue};
pub use ocr::{OcrExtractorError, OcrsTextExtractor};
pub use pipeline::{
    DropReason, EventPipeline, NoTextExtraction, PipelineOutcome, TextExtractor,
    DEFAULT_DEEP_MODE_TTL,
};
