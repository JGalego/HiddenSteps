use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::PrivacyLevel;

/// The signal-type enumeration mirrors `docs/design/05-privacy-model.md` §1 exactly —
/// each variant is tied to the privacy level that first introduces it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignalType {
    AppFocusChange,
    WindowTitle,
    ShortcutUsed,
    AppActionEvent,
    BrowserDomainVisited,
    ClipboardMetadata,
    FileOperationMetadata,
    OcrText,
    Screenshot,
    AccessibilityTree,
}

/// The durable, post-pipeline abstraction of a captured signal — this is what
/// `EventStore` persists. There is deliberately no field here that could hold raw,
/// pre-redaction content (see `CapturedSignal`'s doc comment for why).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EventSummary {
    /// `None` until the store assigns an id on insert.
    pub id: Option<i64>,
    pub occurred_at: OffsetDateTime,
    pub source_id: String,
    pub signal_type: SignalType,
    pub privacy_level_at_capture: PrivacyLevel,
    /// Structured, already-redacted summary content. Shape depends on `signal_type`;
    /// stored as JSON rather than a per-variant struct so new signal-type payload
    /// shapes don't require a schema migration to add a field.
    pub summary: serde_json::Value,
    pub is_deep_mode: bool,
    /// Non-`None` only when `is_deep_mode` is true, per the Deep-mode retention
    /// rule in `docs/design/05-privacy-model.md` §2.
    pub ttl_expires_at: Option<OffsetDateTime>,
}

impl EventSummary {
    /// Constructs a new, not-yet-persisted summary. `is_deep_mode` and
    /// `ttl_expires_at` are derived together so callers can't accidentally set one
    /// without the other.
    pub fn new(
        occurred_at: OffsetDateTime,
        source_id: impl Into<String>,
        signal_type: SignalType,
        privacy_level_at_capture: PrivacyLevel,
        summary: serde_json::Value,
        ttl_expires_at: Option<OffsetDateTime>,
    ) -> Self {
        let is_deep_mode = privacy_level_at_capture == PrivacyLevel::MaximumAssistance;
        Self {
            id: None,
            occurred_at,
            source_id: source_id.into(),
            signal_type,
            privacy_level_at_capture,
            summary,
            is_deep_mode,
            ttl_expires_at: if is_deep_mode { ttl_expires_at } else { None },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deep_mode_flag_follows_privacy_level() {
        let deep = EventSummary::new(
            OffsetDateTime::now_utc(),
            "macos.ocr",
            SignalType::OcrText,
            PrivacyLevel::MaximumAssistance,
            serde_json::json!({"text": "redacted"}),
            Some(OffsetDateTime::now_utc()),
        );
        assert!(deep.is_deep_mode);
        assert!(deep.ttl_expires_at.is_some());

        let shallow = EventSummary::new(
            OffsetDateTime::now_utc(),
            "macos.app_focus",
            SignalType::AppFocusChange,
            PrivacyLevel::ApplicationMetadata,
            serde_json::json!({"app": "Terminal"}),
            None,
        );
        assert!(!shallow.is_deep_mode);
        assert!(shallow.ttl_expires_at.is_none());
    }

    #[test]
    fn non_deep_mode_never_persists_a_ttl_even_if_caller_passes_one() {
        // Guards against a caller mistakenly attaching a TTL to a non-Deep-mode
        // event — per docs/design/05-privacy-model.md §2, only Level 4 rows carry one.
        let summary = EventSummary::new(
            OffsetDateTime::now_utc(),
            "windows.clipboard",
            SignalType::ClipboardMetadata,
            PrivacyLevel::WorkflowMetadata,
            serde_json::json!({"content_type": "text", "size_bytes": 12}),
            Some(OffsetDateTime::now_utc()),
        );
        assert!(summary.ttl_expires_at.is_none());
    }
}
