//! Pure domain types for HiddenSteps: no I/O, no storage, no OS calls.
//!
//! Per ADR-0002 (Clean Architecture as a modular monolith), this crate sits at the
//! innermost layer — every other crate depends on it, it depends on nothing else
//! in the workspace.

mod audit;
mod captured_signal;
mod event;
mod llm_provider_config;
mod pattern;
mod privacy;
mod recommendation;

pub use audit::{AuditActor, AuditEntry};
pub use captured_signal::{CapturedPayload, CapturedSignal};
pub use event::{EventSummary, SignalType};
pub use llm_provider_config::LlmProviderConfig;
pub use pattern::{Pattern, PatternStatus};
pub use privacy::{PrivacyLevel, PrivacyLevelError, PrivacyState};
pub use recommendation::{
    Alternative, Level, Recommendation, RecommendationCategory, RecommendationStatus,
};
