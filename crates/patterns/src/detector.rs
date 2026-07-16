use std::collections::HashMap;

use hiddensteps_domain::EventSummary;
use time::OffsetDateTime;

/// A repeated action-sequence shape found by `PatternDetector::detect` — the
/// deterministic Layer 1 output ADR-0010 describes, before any LLM involvement.
#[derive(Debug, Clone, PartialEq)]
pub struct DetectedPattern {
    /// The ordered sequence of action keys that recurred, e.g.
    /// `["jira:app_action_event", "os:clipboard_metadata", "excel:app_action_event"]`.
    pub signature: Vec<String>,
    pub occurrence_count: u32,
    pub first_seen_at: OffsetDateTime,
    pub last_seen_at: OffsetDateTime,
    pub estimated_minutes_per_occurrence: f64,
    /// Every event id across every occurrence — the source data for FR-13's
    /// traceability ("what observations contributed?") once these are linked via
    /// `EventStore::link_pattern_events`.
    pub contributing_event_ids: Vec<i64>,
}

/// Finds repeated, contiguous action-sequence shapes across an event history via
/// sliding-window n-gram matching — no LLM, no embeddings, just counting.
///
/// "Action key" for an event is `{source_id}:{signal_type}` — the sequence
/// *shape* (which sources/signal-types fired, in what order), not the exact
/// content of any one event. This is deliberate: "you keep switching from Jira to
/// a spreadsheet" is a meaningful, privacy-cheap pattern; "you keep typing the
/// exact string X" is not what this layer is trying to find (and would require
/// content comparison this layer never does).
pub struct PatternDetector {
    /// Inclusive range of contiguous-event window sizes to search. A window of 2
    /// finds simple A→B repetitions; larger windows find longer, more specific
    /// sequences. Both ends of the range are searched independently — a real
    /// repeated 2-step handoff and a real repeated 4-step workflow can both
    /// surface from the same event history without one hiding the other, at the
    /// cost of some deliberate redundancy (see the module-level note below).
    pub window_sizes: std::ops::RangeInclusive<usize>,
    pub min_occurrences: u32,
}

impl Default for PatternDetector {
    fn default() -> Self {
        Self {
            window_sizes: 2..=5,
            min_occurrences: 3,
        }
    }
}

impl PatternDetector {
    pub fn new(window_sizes: std::ops::RangeInclusive<usize>, min_occurrences: u32) -> Self {
        Self {
            window_sizes,
            min_occurrences,
        }
    }

    /// `events` must be sorted oldest-first; this is the same order
    /// `EventStore::list_recent_events` returns rows in if the caller reverses
    /// its (newest-first) result before passing it here.
    pub fn detect(&self, events: &[EventSummary]) -> Vec<DetectedPattern> {
        let mut results = Vec::new();
        for window in self.window_sizes.clone() {
            if window == 0 || events.len() < window {
                continue;
            }
            results.extend(self.detect_for_window(events, window));
        }
        results
    }

    fn detect_for_window(&self, events: &[EventSummary], window: usize) -> Vec<DetectedPattern> {
        struct Occurrence<'a> {
            slice: &'a [EventSummary],
        }

        let mut groups: HashMap<Vec<String>, Vec<Occurrence>> = HashMap::new();
        for start in 0..=(events.len() - window) {
            let slice = &events[start..start + window];
            let signature: Vec<String> = slice.iter().map(action_key).collect();
            groups
                .entry(signature)
                .or_default()
                .push(Occurrence { slice });
        }

        groups
            .into_iter()
            .filter(|(_, occurrences)| occurrences.len() as u32 >= self.min_occurrences)
            .map(|(signature, occurrences)| {
                let first_seen_at = occurrences
                    .iter()
                    .map(|o| o.slice.first().unwrap().occurred_at)
                    .min()
                    .expect("at least one occurrence, checked by the filter above");
                let last_seen_at = occurrences
                    .iter()
                    .map(|o| o.slice.last().unwrap().occurred_at)
                    .max()
                    .expect("at least one occurrence, checked by the filter above");
                let total_span_minutes: f64 = occurrences
                    .iter()
                    .map(|o| {
                        let span = o.slice.last().unwrap().occurred_at
                            - o.slice.first().unwrap().occurred_at;
                        span.as_seconds_f64() / 60.0
                    })
                    .sum();
                let estimated_minutes_per_occurrence =
                    total_span_minutes / occurrences.len() as f64;
                let contributing_event_ids: Vec<i64> = occurrences
                    .iter()
                    .flat_map(|o| o.slice.iter().filter_map(|e| e.id))
                    .collect();

                DetectedPattern {
                    signature,
                    occurrence_count: occurrences.len() as u32,
                    first_seen_at,
                    last_seen_at,
                    estimated_minutes_per_occurrence,
                    contributing_event_ids,
                }
            })
            .collect()
    }
}

/// The structural identity of an event for pattern-matching purposes — deliberately
/// excludes `summary` content (see this module's doc comment).
pub fn action_key(event: &EventSummary) -> String {
    format!(
        "{}:{}",
        event.source_id,
        signal_type_label(event.signal_type)
    )
}

fn signal_type_label(signal_type: hiddensteps_domain::SignalType) -> &'static str {
    use hiddensteps_domain::SignalType::*;
    match signal_type {
        AppFocusChange => "app_focus_change",
        WindowTitle => "window_title",
        ShortcutUsed => "shortcut_used",
        AppActionEvent => "app_action_event",
        BrowserDomainVisited => "browser_domain_visited",
        ClipboardMetadata => "clipboard_metadata",
        FileOperationMetadata => "file_operation_metadata",
        OcrText => "ocr_text",
        Screenshot => "screenshot",
        AccessibilityTree => "accessibility_tree",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hiddensteps_domain::{PrivacyLevel, SignalType};

    fn event_at(
        minutes_from_epoch: i64,
        source_id: &str,
        signal_type: SignalType,
        id: i64,
    ) -> EventSummary {
        let mut event = EventSummary::new(
            OffsetDateTime::UNIX_EPOCH + time::Duration::minutes(minutes_from_epoch),
            source_id,
            signal_type,
            PrivacyLevel::WorkflowMetadata,
            serde_json::json!({}),
            None,
        );
        event.id = Some(id);
        event
    }

    #[test]
    fn finds_a_simple_two_step_pattern_repeated_the_minimum_number_of_times() {
        // jira -> excel, three times, each pair 2 minutes apart, sessions spread
        // across an otherwise-quiet history — mirrors PROMPT.md's own worked
        // example ("observed this workflow 31 times over the last two weeks"),
        // just at test scale.
        let mut events = Vec::new();
        let mut id = 1;
        for session_start in [0, 100, 200] {
            events.push(event_at(
                session_start,
                "jira",
                SignalType::AppActionEvent,
                id,
            ));
            id += 1;
            events.push(event_at(
                session_start + 2,
                "excel",
                SignalType::AppActionEvent,
                id,
            ));
            id += 1;
        }

        let detector = PatternDetector::new(2..=2, 3);
        let detected = detector.detect(&events);

        assert_eq!(detected.len(), 1);
        let pattern = &detected[0];
        assert_eq!(
            pattern.signature,
            vec!["jira:app_action_event", "excel:app_action_event"]
        );
        assert_eq!(pattern.occurrence_count, 3);
        assert_eq!(pattern.estimated_minutes_per_occurrence, 2.0);
        assert_eq!(pattern.contributing_event_ids.len(), 6);
    }

    #[test]
    fn does_not_report_a_sequence_below_the_minimum_occurrence_threshold() {
        let events = vec![
            event_at(0, "jira", SignalType::AppActionEvent, 1),
            event_at(2, "excel", SignalType::AppActionEvent, 2),
            event_at(100, "jira", SignalType::AppActionEvent, 3),
            event_at(102, "excel", SignalType::AppActionEvent, 4),
        ];
        let detector = PatternDetector::new(2..=2, 3);
        assert!(detector.detect(&events).is_empty());
    }

    #[test]
    fn distinguishes_different_sequences_and_does_not_conflate_them() {
        let mut events = Vec::new();
        let mut id = 1;
        for _ in 0..3 {
            events.push(event_at(id, "jira", SignalType::AppActionEvent, id));
            id += 1;
            events.push(event_at(id, "excel", SignalType::AppActionEvent, id));
            id += 1;
        }
        for _ in 0..3 {
            events.push(event_at(id, "slack", SignalType::AppFocusChange, id));
            id += 1;
            events.push(event_at(id, "chrome", SignalType::AppFocusChange, id));
            id += 1;
        }

        let detector = PatternDetector::new(2..=2, 3);
        let detected = detector.detect(&events);
        assert_eq!(detected.len(), 2);
    }

    #[test]
    fn window_of_zero_or_larger_than_history_is_skipped_without_panicking() {
        let events = vec![event_at(0, "jira", SignalType::AppActionEvent, 1)];
        let detector = PatternDetector::new(0..=10, 1);
        // Must not panic on an empty range component or an out-of-bounds window;
        // whatever legitimately matches within bounds is fine.
        let _ = detector.detect(&events);
    }

    #[test]
    fn action_key_ignores_summary_content() {
        let mut a = event_at(0, "jira", SignalType::AppActionEvent, 1);
        a.summary = serde_json::json!({"ticket": "PROJ-123"});
        let mut b = event_at(1, "jira", SignalType::AppActionEvent, 2);
        b.summary = serde_json::json!({"ticket": "PROJ-999"});
        assert_eq!(action_key(&a), action_key(&b));
    }
}
