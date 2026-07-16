//! The Recommendation Engine (ADR-0010): Layer 2, LLM-based synthesis on top of
//! `hiddensteps-patterns`' deterministic Layer 1 output. See `prompt.rs`'s doc
//! comment on `SynthesizedFields` for exactly how the "LLM is never the source of
//! frequency/timing numbers" rule is enforced — structurally, not just checked.

mod prompt;
mod synthesizer;
mod validate;

pub use synthesizer::{SynthesisError, Synthesizer};
pub use validate::contradicts_occurrence_count;
