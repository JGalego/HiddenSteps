use hiddensteps_domain::{CapturedSignal, EventSummary, PrivacyLevel};
use hiddensteps_redaction::{RedactionEngine, RedactionOutcome};
use time::{Duration, OffsetDateTime};

use crate::classify::{classify, minimum_level_for, ClassifiedSignal, FieldValue};

/// Extracts text from binary Deep-mode content (OCR over a screenshot). There is
/// deliberately no in-tree implementation that does real OCR — that's a platform-
/// specific integration (e.g. Windows.Media.Ocr, macOS Vision framework,
/// tesseract) left to a future milestone/plugin. Without one configured, Deep-mode
/// screenshot capture safely drops instead of silently skipping redaction.
pub trait TextExtractor: Send + Sync {
    fn extract(&self, raw_bytes: &[u8]) -> Option<String>;
}

/// The default extractor: no OCR available, so screenshot-derived signals always
/// drop. This is the correct default, not a placeholder to fill in later — a
/// pipeline should never invent text from bytes it can't actually read.
pub struct NoTextExtraction;

impl TextExtractor for NoTextExtraction {
    fn extract(&self, _raw_bytes: &[u8]) -> Option<String> {
        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropReason {
    /// The Redaction Engine found an ambiguous (not high-confidence) match and,
    /// per the drop-on-uncertainty policy, the whole event is discarded.
    RedactionUncertain,
    /// A Deep-mode signal (e.g. a screenshot) arrived but no `TextExtractor` could
    /// produce text from it.
    OcrUnavailable,
    /// The signal's type requires a higher privacy level than the one supplied —
    /// see the module doc comment; this should not happen if `ObservationSource`
    /// plugins are correctly gated (ADR-0005), but the pipeline enforces it anyway.
    SignalNotAllowedAtCurrentLevel,
}

pub enum PipelineOutcome {
    Summarized(EventSummary),
    Dropped(DropReason),
}

/// Default Deep-mode retention window, per `docs/design/05-privacy-model.md` §2.
pub const DEFAULT_DEEP_MODE_TTL: Duration = Duration::days(90);

pub struct EventPipeline<T: TextExtractor = NoTextExtraction> {
    redactor: RedactionEngine,
    text_extractor: T,
    /// `None` disables Deep-mode expiry entirely ("keep forever"); callers wanting
    /// the spec's default should use `EventPipeline::new()`, which sets this to
    /// `DEFAULT_DEEP_MODE_TTL`.
    deep_mode_ttl: Option<Duration>,
}

impl EventPipeline<NoTextExtraction> {
    pub fn new() -> Self {
        Self {
            redactor: RedactionEngine::new(),
            text_extractor: NoTextExtraction,
            deep_mode_ttl: Some(DEFAULT_DEEP_MODE_TTL),
        }
    }
}

impl Default for EventPipeline<NoTextExtraction> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: TextExtractor> EventPipeline<T> {
    pub fn with_text_extractor(text_extractor: T, deep_mode_ttl: Option<Duration>) -> Self {
        Self {
            redactor: RedactionEngine::new(),
            text_extractor,
            deep_mode_ttl,
        }
    }

    pub fn process(
        &self,
        signal: CapturedSignal,
        privacy_level: PrivacyLevel,
        now: OffsetDateTime,
    ) -> PipelineOutcome {
        let classified = classify(signal.payload);

        // Check the privacy-level gate before doing any extraction/redaction work
        // — a disallowed signal is rejected outright, not processed and then
        // discarded.
        let signal_type = match &classified {
            ClassifiedSignal::Fields { signal_type, .. } => *signal_type,
            ClassifiedSignal::RequiresExtraction { signal_type, .. } => *signal_type,
        };
        if minimum_level_for(signal_type) > privacy_level {
            return PipelineOutcome::Dropped(DropReason::SignalNotAllowedAtCurrentLevel);
        }

        let (signal_type, fields) = match classified {
            ClassifiedSignal::Fields {
                signal_type,
                fields,
            } => (signal_type, fields),
            ClassifiedSignal::RequiresExtraction {
                signal_type,
                raw_bytes,
            } => match self.text_extractor.extract(&raw_bytes) {
                Some(text) => (signal_type, vec![("text", FieldValue::Redactable(text))]),
                None => return PipelineOutcome::Dropped(DropReason::OcrUnavailable),
            },
        };

        let mut summary = serde_json::Map::new();
        for (name, value) in fields {
            match value {
                FieldValue::Passthrough(v) => {
                    summary.insert(name.to_string(), v);
                }
                FieldValue::Redactable(text) => match self.redactor.scan(&text) {
                    RedactionOutcome::Clean(t) | RedactionOutcome::Redacted(t) => {
                        summary.insert(name.to_string(), serde_json::Value::String(t));
                    }
                    RedactionOutcome::Drop => {
                        return PipelineOutcome::Dropped(DropReason::RedactionUncertain);
                    }
                },
            }
        }

        let ttl_expires_at = if privacy_level == PrivacyLevel::MaximumAssistance {
            self.deep_mode_ttl.map(|ttl| now + ttl)
        } else {
            None
        };

        let event = EventSummary::new(
            now,
            signal.source_id,
            signal_type,
            privacy_level,
            serde_json::Value::Object(summary),
            ttl_expires_at,
        );

        PipelineOutcome::Summarized(event)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hiddensteps_domain::CapturedPayload;

    fn signal(source_id: &str, payload: CapturedPayload) -> CapturedSignal {
        CapturedSignal::new(source_id, payload)
    }

    #[test]
    fn clean_app_focus_signal_is_summarized() {
        let pipeline = EventPipeline::new();
        let outcome = pipeline.process(
            signal(
                "linux.app_focus",
                CapturedPayload::AppFocusChange {
                    app_identifier: "org.gnome.Terminal".to_string(),
                },
            ),
            PrivacyLevel::ApplicationMetadata,
            OffsetDateTime::now_utc(),
        );
        match outcome {
            PipelineOutcome::Summarized(event) => {
                assert_eq!(event.summary["app"], "org.gnome.Terminal");
                assert!(!event.is_deep_mode);
            }
            PipelineOutcome::Dropped(reason) => panic!("expected Summarized, got {reason:?}"),
        }
    }

    #[test]
    fn signal_above_current_privacy_level_is_dropped() {
        let pipeline = EventPipeline::new();
        // ClipboardMetadata requires WorkflowMetadata (Level 2); supply Level 1.
        let outcome = pipeline.process(
            signal(
                "linux.clipboard",
                CapturedPayload::ClipboardMetadata {
                    content_type: "text/plain".to_string(),
                    size_bytes: 42,
                },
            ),
            PrivacyLevel::ApplicationMetadata,
            OffsetDateTime::now_utc(),
        );
        assert!(matches!(
            outcome,
            PipelineOutcome::Dropped(DropReason::SignalNotAllowedAtCurrentLevel)
        ));
    }

    #[test]
    fn secret_in_window_title_gets_redacted_not_leaked() {
        let pipeline = EventPipeline::new();
        let outcome = pipeline.process(
            signal(
                "linux.window_title",
                CapturedPayload::WindowTitle {
                    title: "AWS_ACCESS_KEY_ID=AKIAIOSFODNN7EXAMPLE - Terminal".to_string(),
                },
            ),
            PrivacyLevel::ApplicationMetadata,
            OffsetDateTime::now_utc(),
        );
        match outcome {
            PipelineOutcome::Summarized(event) => {
                let title = event.summary["title"].as_str().unwrap();
                assert!(!title.contains("AKIAIOSFODNN7EXAMPLE"));
                assert!(title.contains("[REDACTED:api_key_or_token]"));
            }
            PipelineOutcome::Dropped(reason) => panic!("expected Summarized, got {reason:?}"),
        }
    }

    #[test]
    fn ambiguous_secret_in_file_path_drops_the_event() {
        let pipeline = EventPipeline::new();
        let outcome = pipeline.process(
            signal(
                "linux.file_ops",
                CapturedPayload::FileOperation {
                    path: "/home/user/xK9mQ2vP7zR4wN8tL1bH5cD3.txt".to_string(),
                    operation: "write".to_string(),
                },
            ),
            PrivacyLevel::WorkflowMetadata,
            OffsetDateTime::now_utc(),
        );
        assert!(matches!(
            outcome,
            PipelineOutcome::Dropped(DropReason::RedactionUncertain)
        ));
    }

    #[test]
    fn clipboard_metadata_never_carries_actual_content() {
        let pipeline = EventPipeline::new();
        let outcome = pipeline.process(
            signal(
                "linux.clipboard",
                CapturedPayload::ClipboardMetadata {
                    content_type: "text/plain".to_string(),
                    size_bytes: 128,
                },
            ),
            PrivacyLevel::WorkflowMetadata,
            OffsetDateTime::now_utc(),
        );
        match outcome {
            PipelineOutcome::Summarized(event) => {
                assert_eq!(event.summary["content_type"], "text/plain");
                assert_eq!(event.summary["size_bytes"], 128);
                // No field on this signal type could ever have carried clipboard
                // *content* — there's no such field in the domain payload at all.
                assert_eq!(event.summary.as_object().unwrap().len(), 2);
            }
            PipelineOutcome::Dropped(reason) => panic!("expected Summarized, got {reason:?}"),
        }
    }

    #[test]
    fn screenshot_without_an_ocr_extractor_is_dropped_not_stored_as_bytes() {
        let pipeline = EventPipeline::new(); // NoTextExtraction
        let outcome = pipeline.process(
            signal(
                "macos.screenshot",
                CapturedPayload::Screenshot {
                    raw_bytes: vec![0u8; 128],
                },
            ),
            PrivacyLevel::MaximumAssistance,
            OffsetDateTime::now_utc(),
        );
        assert!(matches!(
            outcome,
            PipelineOutcome::Dropped(DropReason::OcrUnavailable)
        ));
    }

    struct StubOcr;
    impl TextExtractor for StubOcr {
        fn extract(&self, _raw_bytes: &[u8]) -> Option<String> {
            Some("Invoice total: $4,532.10".to_string())
        }
    }

    #[test]
    fn screenshot_with_an_ocr_extractor_is_redacted_and_gets_a_deep_mode_ttl() {
        let pipeline = EventPipeline::with_text_extractor(StubOcr, Some(Duration::days(90)));
        let now = OffsetDateTime::now_utc();
        let outcome = pipeline.process(
            signal(
                "macos.screenshot",
                CapturedPayload::Screenshot {
                    raw_bytes: vec![0u8; 128],
                },
            ),
            PrivacyLevel::MaximumAssistance,
            now,
        );
        match outcome {
            PipelineOutcome::Summarized(event) => {
                assert!(event.is_deep_mode);
                assert!(event.ttl_expires_at.is_some());
                assert_eq!(event.ttl_expires_at.unwrap(), now + Duration::days(90));
            }
            PipelineOutcome::Dropped(reason) => panic!("expected Summarized, got {reason:?}"),
        }
    }

    #[test]
    fn deep_mode_signal_below_level_four_is_dropped_even_with_an_extractor() {
        let pipeline = EventPipeline::with_text_extractor(StubOcr, Some(Duration::days(90)));
        let outcome = pipeline.process(
            signal(
                "macos.screenshot",
                CapturedPayload::Screenshot {
                    raw_bytes: vec![0u8; 128],
                },
            ),
            PrivacyLevel::ContextAware, // Level 3, not Level 4
            OffsetDateTime::now_utc(),
        );
        assert!(matches!(
            outcome,
            PipelineOutcome::Dropped(DropReason::SignalNotAllowedAtCurrentLevel)
        ));
    }
}
