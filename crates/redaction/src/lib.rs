//! The Redaction Engine: the pipeline stage between Classify and Summarize
//! (ADR-0006), implementing the detection categories and drop-on-uncertainty
//! policy from `docs/design/05-privacy-model.md` §4.
//!
//! This crate operates on plain `&str` — it has no knowledge of `EventSummary` or
//! any storage type, so it can be unit-tested (and adversarially red-teamed, per
//! `docs/roadmap/04-security-testing.md`) in complete isolation from the rest of
//! the pipeline.

mod detectors;
mod engine;
mod entropy;

pub use detectors::Category;
pub use engine::{RedactionEngine, RedactionOutcome};
