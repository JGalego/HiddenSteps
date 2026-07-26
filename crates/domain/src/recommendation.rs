use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

/// Mirrors the `recommendations` table in `docs/design/07-database-schema.md`.
/// Every field here corresponds to one of the "every recommendation must
/// include" requirements in PROMPT.md's Recommendation Engine section and FR-10 —
/// there is no way to construct one of these without supplying all of them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Recommendation {
    pub id: Option<i64>,
    pub pattern_id: i64,
    pub created_at: OffsetDateTime,
    pub title: String,
    pub category: RecommendationCategory,
    pub why: String,
    pub confidence: f32,
    pub estimated_time_saved_minutes: f64,
    pub difficulty: Level,
    pub maintenance_burden: Level,
    pub privacy_implications: String,
    pub implementation_effort: String,
    pub alternatives: Vec<Alternative>,
    pub assumptions: Vec<String>,
    pub ignored_information: Vec<String>,
    pub generating_provider: String,
    pub status: RecommendationStatus,
    pub dismissal_reason: Option<String>,
    /// When an OS notification was last sent for this recommendation, if ever.
    /// `None` means "still owed a notification" — the proactive-delivery sweep
    /// (`recommendation_loop`'s notification pass) treats this as the thing to
    /// check, not `status`, since `status` alone can't distinguish "brand new,
    /// never notified" from "notified an hour ago, still sitting unread."
    /// Snoozing resets this back to `None` (see `snoozed_until` below) so the
    /// recommendation is treated as owing a fresh notification once the snooze
    /// window passes, rather than staying permanently "already notified."
    pub notified_at: Option<OffsetDateTime>,
    /// Set by the snooze action (alongside the existing accept/dismiss
    /// actions) to "come back to this later" — a temporary suppression of
    /// notification, not a fourth `RecommendationStatus`. A snoozed
    /// recommendation is still `Suggested`: it hasn't been acted on, it's just
    /// not due for another notification until this timestamp passes. Modeling
    /// snooze this way (rather than a new status variant) keeps every
    /// existing `status = 'suggested'` query — "what's still outstanding?" —
    /// correct without change, since a snoozed recommendation *is* still
    /// outstanding.
    pub snoozed_until: Option<OffsetDateTime>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecommendationCategory {
    Shortcut,
    Template,
    Script,
    BrowserAutomation,
    Rpa,
    WorkflowPlatform,
    AiAgent,
    Hybrid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Level {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecommendationStatus {
    Suggested,
    Implemented,
    Dismissed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Alternative {
    pub approach: String,
    pub tradeoff: String,
}
