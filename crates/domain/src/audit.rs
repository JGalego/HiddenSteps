use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

/// Per `docs/design/06-security-architecture.md` §4: the audit log is append-only
/// and records action metadata only — never captured content. There is no variant
/// of `AuditEntry` anywhere in this crate that could hold observation content,
/// by the same structural-enforcement discipline as `CapturedSignal`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditActor {
    User,
    System,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuditEntry {
    pub id: Option<i64>,
    pub occurred_at: OffsetDateTime,
    pub actor: AuditActor,
    /// e.g. "privacy_level_changed", "data_exported", "plugin_capability_granted" —
    /// an open string rather than a closed enum here, deliberately: new action types
    /// (new plugin capabilities, new enterprise-policy actions) should be addable
    /// without a domain-crate release, since this field is metadata, not a trust
    /// boundary in itself.
    pub action_type: String,
    /// Structured metadata about the action (which level, which plugin id, ...).
    /// Must never contain observation content — that invariant is enforced by
    /// convention at call sites, not by the type system, since `serde_json::Value`
    /// can hold anything; reviewers should treat a content-shaped `details` value
    /// as a bug, not a style nit.
    pub details: serde_json::Value,
}

impl AuditEntry {
    pub fn new(
        actor: AuditActor,
        action_type: impl Into<String>,
        details: serde_json::Value,
    ) -> Self {
        Self {
            id: None,
            occurred_at: OffsetDateTime::now_utc(),
            actor,
            action_type: action_type.into(),
            details,
        }
    }
}
