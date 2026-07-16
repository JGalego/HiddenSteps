use crate::detectors::{detect_all, Confidence, Detection};

/// The result of running text through the Redaction Engine — the Event Pipeline
/// stage between Classify and Summarize (ADR-0006).
///
/// There is no variant that returns "stored as-is but flagged risky" — per
/// `docs/design/05-privacy-model.md` §4, uncertainty resolves to `Drop`, never to a
/// partially-trusted pass-through.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RedactionOutcome {
    /// Nothing sensitive found; safe to summarize as-is.
    Clean(String),
    /// One or more high-confidence detections were found and replaced with
    /// `[REDACTED:<category>]` markers. Still safe to summarize.
    Redacted(String),
    /// At least one detection was ambiguous (confidence not high enough to trust
    /// the redaction boundary). The whole event is dropped rather than stored
    /// partially redacted — this is the drop-on-uncertainty policy, not a bug.
    Drop,
}

#[derive(Debug, Clone)]
pub struct RedactionEngine {
    _private: (),
}

impl Default for RedactionEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl RedactionEngine {
    pub fn new() -> Self {
        Self { _private: () }
    }

    /// Scans `text` for secrets/PII/PHI/financial identifiers and returns the
    /// pipeline's next move. Detection categories and the drop-on-uncertainty
    /// policy are documented in `docs/design/05-privacy-model.md` §4.
    pub fn scan(&self, text: &str) -> RedactionOutcome {
        let detections = detect_all(text);
        if detections.is_empty() {
            return RedactionOutcome::Clean(text.to_string());
        }
        if detections
            .iter()
            .any(|d| d.confidence == Confidence::Ambiguous)
        {
            return RedactionOutcome::Drop;
        }
        RedactionOutcome::Redacted(apply_redactions(text, &detections))
    }
}

fn apply_redactions(text: &str, detections: &[Detection]) -> String {
    let mut result = String::with_capacity(text.len());
    let mut cursor = 0usize;
    // Detections are sorted by start offset (detect_all's contract); overlapping
    // high-confidence detections shouldn't occur given the detector set, but we
    // defensively skip any detection that starts before our cursor rather than
    // panic or produce corrupted output.
    for detection in detections {
        if detection.start < cursor {
            continue;
        }
        result.push_str(&text[cursor..detection.start]);
        result.push_str("[REDACTED:");
        result.push_str(detection.category.label());
        result.push(']');
        cursor = detection.end;
    }
    result.push_str(&text[cursor..]);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_text_passes_through_unchanged() {
        let engine = RedactionEngine::new();
        let outcome = engine.scan("opened the spreadsheet and reviewed totals");
        assert_eq!(
            outcome,
            RedactionOutcome::Clean("opened the spreadsheet and reviewed totals".to_string())
        );
    }

    #[test]
    fn high_confidence_secret_is_redacted_in_place_not_dropped() {
        let engine = RedactionEngine::new();
        let outcome = engine.scan("export AWS_ACCESS_KEY_ID=AKIAIOSFODNN7EXAMPLE please rotate");
        match outcome {
            RedactionOutcome::Redacted(text) => {
                assert!(text.contains("[REDACTED:api_key_or_token]"));
                assert!(!text.contains("AKIAIOSFODNN7EXAMPLE"));
                assert!(text.contains("please rotate")); // surrounding context preserved
            }
            other => panic!("expected Redacted, got {other:?}"),
        }
    }

    #[test]
    fn ambiguous_high_entropy_token_drops_the_whole_event() {
        let engine = RedactionEngine::new();
        let outcome = engine.scan("saved config value xK9mQ2vP7zR4wN8tL1bH5cD3 to disk");
        assert_eq!(outcome, RedactionOutcome::Drop);
    }

    #[test]
    fn multiple_secrets_are_all_redacted_and_none_leak() {
        let engine = RedactionEngine::new();
        let text = "user jane.doe@example.com used token ghp_1234567890abcdefghijklmnopqrstuvwxyz";
        match engine.scan(text) {
            RedactionOutcome::Redacted(redacted) => {
                assert!(!redacted.contains("jane.doe@example.com"));
                assert!(!redacted.contains("ghp_1234567890abcdefghijklmnopqrstuvwxyz"));
                assert!(redacted.contains("[REDACTED:email]"));
                assert!(redacted.contains("[REDACTED:api_key_or_token]"));
            }
            other => panic!("expected Redacted, got {other:?}"),
        }
    }

    #[test]
    fn window_title_style_short_strings_are_unaffected() {
        let engine = RedactionEngine::new();
        let outcome = engine.scan("index.rs — HiddenSteps — Visual Studio Code");
        assert!(matches!(outcome, RedactionOutcome::Clean(_)));
    }
}
