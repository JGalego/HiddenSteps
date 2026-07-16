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
