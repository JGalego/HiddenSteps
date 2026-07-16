//! The Event Pipeline: Classify → Redact → Summarize, per ADR-0006.
//!
//! `EventPipeline::process` is the *only* way a `CapturedSignal` becomes an
//! `EventSummary` — there is no path around Redact, and there is no path that
//! writes a `CapturedSignal` to storage directly (`hiddensteps-event-store` has no
//! API that would accept one).
//!
//! Note on scope: Level 3 (ContextAware) in `docs/design/05-privacy-model.md` §1 is
//! described as "richer context" layered onto Level 2's signal types (fuller
//! in-app action context, browser page title) rather than introducing wholly new
//! signal types of its own. This implementation's `SignalType` enum (in the domain
//! crate) does not yet model that richer-context distinction — `minimum_level_for`
//! currently maps every Level-2-shaped signal to `WorkflowMetadata`, so a Level 3
//! user sees the same signal *types* as Level 2 until a future milestone adds the
//! richer variants. This is a deliberate, disclosed simplification, not a silent
//! gap: Level 3 is not currently distinguishable from Level 2 in stored data.

mod classify;
mod pipeline;

pub use classify::{minimum_level_for, FieldValue};
pub use pipeline::{DropReason, EventPipeline, NoTextExtraction, PipelineOutcome, TextExtractor};
