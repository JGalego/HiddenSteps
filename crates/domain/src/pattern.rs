use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

/// A detected repeated workflow — the deterministic Layer 1 output of the
/// Recommendation Engine (ADR-0010), mirroring the `patterns` table in
/// `docs/design/07-database-schema.md`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Pattern {
    pub id: Option<i64>,
    pub first_seen_at: OffsetDateTime,
    pub last_seen_at: OffsetDateTime,
    pub occurrence_count: u32,
    pub estimated_minutes_per_occurrence: Option<f64>,
    /// A canonicalized representation of the repeated action sequence (e.g. the
    /// ordered `(source_id, signal_type)` shape that recurred) — opaque to
    /// storage, meaningful to Pattern Detection.
    pub sequence_signature: serde_json::Value,
    pub status: PatternStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PatternStatus {
    Active,
    Stale,
    Dismissed,
}

impl Pattern {
    pub fn total_estimated_minutes(&self) -> Option<f64> {
        self.estimated_minutes_per_occurrence
            .map(|per_occurrence| per_occurrence * self.occurrence_count as f64)
    }
}
