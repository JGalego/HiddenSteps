use hiddensteps_domain::{CapturedPayload, PrivacyLevel, SignalType};

/// The minimum privacy level at which a given signal type is allowed to exist at
/// all, per `docs/design/05-privacy-model.md` §1. The Event Pipeline enforces this
/// defensively — belt-and-suspenders on top of `ObservationSource` plugins only
/// activating at their declared `min_privacy_level` (ADR-0005) — so a bug or a
/// misbehaving plugin can't smuggle a higher-level signal through at a lower level.
pub fn minimum_level_for(signal_type: SignalType) -> PrivacyLevel {
    match signal_type {
        SignalType::AppFocusChange | SignalType::WindowTitle | SignalType::ShortcutUsed => {
            PrivacyLevel::ApplicationMetadata
        }
        SignalType::AppActionEvent
        | SignalType::BrowserDomainVisited
        | SignalType::ClipboardMetadata
        | SignalType::FileOperationMetadata => PrivacyLevel::WorkflowMetadata,
        // Level 3 (ContextAware) doesn't yet have signal types exclusively its own
        // in this implementation — see the module-level note in lib.rs.
        SignalType::OcrText | SignalType::Screenshot | SignalType::AccessibilityTree => {
            PrivacyLevel::MaximumAssistance
        }
    }
}

/// A single summary field, either free text that must go through Redact, or a
/// value that's already known-safe (an enum-shaped tag, a byte count) and skips
/// scanning because there's nothing in it that could carry a secret.
pub enum FieldValue {
    Redactable(String),
    Passthrough(serde_json::Value),
}

/// What the Classify stage hands to Redact/Summarize: the signal's domain-level
/// type, and its fields — each either redactable text or already-safe metadata.
pub enum ClassifiedSignal {
    Fields {
        signal_type: SignalType,
        fields: Vec<(&'static str, FieldValue)>,
    },
    /// Binary content (a screenshot) that needs an OCR/text-extraction step before
    /// it can be turned into fields or dropped.
    RequiresExtraction {
        signal_type: SignalType,
        raw_bytes: Vec<u8>,
    },
}

pub fn classify(payload: CapturedPayload) -> ClassifiedSignal {
    match payload {
        CapturedPayload::AppFocusChange { app_identifier } => ClassifiedSignal::Fields {
            signal_type: SignalType::AppFocusChange,
            fields: vec![("app", FieldValue::Redactable(app_identifier))],
        },
        CapturedPayload::WindowTitle { title } => ClassifiedSignal::Fields {
            signal_type: SignalType::WindowTitle,
            fields: vec![("title", FieldValue::Redactable(title))],
        },
        CapturedPayload::ShortcutInvoked { shortcut } => ClassifiedSignal::Fields {
            signal_type: SignalType::ShortcutUsed,
            fields: vec![("shortcut", FieldValue::Redactable(shortcut))],
        },
        CapturedPayload::BrowserDomainVisited { domain } => ClassifiedSignal::Fields {
            signal_type: SignalType::BrowserDomainVisited,
            fields: vec![("domain", FieldValue::Redactable(domain))],
        },
        CapturedPayload::ClipboardMetadata {
            content_type,
            size_bytes,
        } => ClassifiedSignal::Fields {
            signal_type: SignalType::ClipboardMetadata,
            fields: vec![
                // The content *type* (e.g. "text/plain") is an enum-shaped label,
                // not user content — safe to pass through. The clipboard's actual
                // content never reaches this pipeline at all (no
                // `CapturedPayload` variant carries it), per
                // `docs/design/05-privacy-model.md` §1.
                (
                    "content_type",
                    FieldValue::Passthrough(serde_json::json!(content_type)),
                ),
                (
                    "size_bytes",
                    FieldValue::Passthrough(serde_json::json!(size_bytes)),
                ),
            ],
        },
        CapturedPayload::FileOperation { path, operation } => ClassifiedSignal::Fields {
            signal_type: SignalType::FileOperationMetadata,
            fields: vec![
                ("path", FieldValue::Redactable(path)),
                (
                    "operation",
                    FieldValue::Passthrough(serde_json::json!(operation)),
                ),
            ],
        },
        CapturedPayload::OcrText { text } => ClassifiedSignal::Fields {
            signal_type: SignalType::OcrText,
            fields: vec![("text", FieldValue::Redactable(text))],
        },
        CapturedPayload::Screenshot { raw_bytes } => ClassifiedSignal::RequiresExtraction {
            signal_type: SignalType::Screenshot,
            raw_bytes,
        },
        CapturedPayload::AccessibilityTree { serialized } => ClassifiedSignal::Fields {
            signal_type: SignalType::AccessibilityTree,
            fields: vec![("tree", FieldValue::Redactable(serialized))],
        },
    }
}
