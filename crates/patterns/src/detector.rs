use std::collections::HashMap;

use hiddensteps_domain::EventSummary;
use time::OffsetDateTime;

/// A repeated action-sequence shape found by `PatternDetector::detect` — the
/// deterministic Layer 1 output ADR-0010 describes, before any LLM involvement.
#[derive(Debug, Clone, PartialEq)]
pub struct DetectedPattern {
    /// The ordered sequence of action keys that recurred, e.g.
    /// `["app_focus_change:slack.exe", "clipboard_metadata:text/plain",
    /// "app_focus_change:excel.exe"]`.
    pub signature: Vec<String>,
    pub occurrence_count: u32,
    pub first_seen_at: OffsetDateTime,
    pub last_seen_at: OffsetDateTime,
    pub estimated_minutes_per_occurrence: f64,
    /// Every event id across every occurrence — the source data for FR-13's
    /// traceability ("what observations contributed?") once these are linked via
    /// `EventStore::link_pattern_events`.
    pub contributing_event_ids: Vec<i64>,
    /// Whether this signature includes a signal type `docs/design/05-privacy-
    /// model.md`'s cloud-eligibility rules (mirrored in
    /// `hiddensteps_privacy_engine::gate::cloud_eligibility`) treat as a verbatim
    /// string (currently: a browser domain) rather than shape-only metadata —
    /// callers dispatching this pattern to a cloud `LlmProvider` must gate on
    /// this, same as any other verbatim content class.
    pub contains_verbatim_strings: bool,
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
                let contains_verbatim_strings = signature_contains_verbatim(&signature);

                DetectedPattern {
                    signature,
                    occurrence_count: occurrences.len() as u32,
                    first_seen_at,
                    last_seen_at,
                    estimated_minutes_per_occurrence,
                    contributing_event_ids,
                    contains_verbatim_strings,
                }
            })
            .collect()
    }
}

/// The structural identity of an event for pattern-matching purposes: which
/// signal type fired, and — where one exists — a low-cardinality *subject*
/// distinguishing *which* app/format/operation it was about. Plain
/// `signal_type` alone (this module's original identity) can never tell "you
/// keep switching to Slack" apart from "you keep switching to Excel", since
/// every `AppFocusChange` event looks identical without it; `source_id` can't
/// fill that gap either — it identifies the *capture module* (e.g.
/// `windows.active_window`), which is the same for every app a given platform's
/// single active-window watcher ever reports on, not the app itself.
///
/// Window titles and Level-4 deep-mode content (OCR text, screenshots,
/// accessibility trees) deliberately have no subject here and fall back to
/// `signal_type` alone: titles are too high-cardinality to ever meaningfully
/// repeat verbatim, and deep-mode content is too sensitive to fold into a
/// pattern signature that may end up in an LLM prompt.
pub fn action_key(event: &EventSummary) -> String {
    match subject(event) {
        Some(subject) => format!("{}:{}", signal_type_label(event.signal_type), subject.value),
        None => format!(
            "{}:{}",
            event.source_id,
            signal_type_label(event.signal_type)
        ),
    }
}

struct Subject {
    value: String,
}

/// Pulls the identifying value out of an event's summary fields (set by
/// `hiddensteps_pipeline::classify`), per signal type. Field names here must
/// match `classify`'s `fields` list for that `CapturedPayload` variant exactly.
fn subject(event: &EventSummary) -> Option<Subject> {
    use hiddensteps_domain::SignalType::*;
    let obj = event.summary.as_object()?;
    let str_field = |name: &str| obj.get(name).and_then(|v| v.as_str());

    match event.signal_type {
        AppFocusChange => str_field("app").map(|app| Subject {
            value: app.to_string(),
        }),
        ShortcutUsed => str_field("shortcut").map(|shortcut| Subject {
            value: shortcut.to_string(),
        }),
        BrowserDomainVisited => str_field("domain").map(|domain| Subject {
            value: domain.to_string(),
        }),
        ClipboardMetadata => str_field("content_type").map(|content_type| Subject {
            value: content_type.to_string(),
        }),
        FileOperationMetadata => {
            let operation = str_field("operation")?;
            // Deliberately the extension, never the path itself — a full path
            // is exactly the kind of verbatim, personally-identifying string
            // (project/folder names) this signature should not carry, and is
            // high-cardinality enough to rarely repeat anyway.
            let extension = str_field("path")
                .and_then(|path| std::path::Path::new(path).extension())
                .and_then(|ext| ext.to_str())
                .unwrap_or("no_extension");
            Some(Subject {
                value: format!("{operation}:{extension}"),
            })
        }
        WindowTitle | AppActionEvent | OcrText | Screenshot | AccessibilityTree => None,
    }
}

/// Whether any position in a signature is a signal type
/// `hiddensteps_privacy_engine::gate::cloud_eligibility` treats as carrying a
/// verbatim string — currently just `browser_domain_visited`, the one subject
/// type above sourced from exactly the kind of value (a domain) that crate's
/// doc comment names. `signal_type_label` is a stable, closed set, so matching
/// on the label prefix here (rather than re-deriving `Subject`s) is exact, not
/// a heuristic.
fn signature_contains_verbatim(signature: &[String]) -> bool {
    signature
        .iter()
        .any(|key| key.starts_with("browser_domain_visited:"))
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

    #[test]
    fn app_focus_change_action_key_distinguishes_which_app_not_just_the_capture_module() {
        // Both events come from the same capture module (same source_id, as a
        // real single active-window watcher would report for every app) --
        // only the summary's "app" field differs, the way it would in
        // production between two real app switches.
        let mut slack = event_at(0, "windows.active_window", SignalType::AppFocusChange, 1);
        slack.summary = serde_json::json!({"app": "slack.exe"});
        let mut excel = event_at(1, "windows.active_window", SignalType::AppFocusChange, 2);
        excel.summary = serde_json::json!({"app": "excel.exe"});

        assert_ne!(action_key(&slack), action_key(&excel));
        assert_eq!(action_key(&slack), "app_focus_change:slack.exe");
    }

    #[test]
    fn app_focus_change_with_no_app_field_falls_back_to_source_id() {
        let event = event_at(0, "windows.active_window", SignalType::AppFocusChange, 1);
        assert_eq!(action_key(&event), "windows.active_window:app_focus_change");
    }

    #[test]
    fn file_operation_action_key_uses_extension_not_the_full_path() {
        let mut event = event_at(
            0,
            "linux.file_operations",
            SignalType::FileOperationMetadata,
            1,
        );
        event.summary = serde_json::json!({"path": "/home/alice/secret-project/report.xlsx", "operation": "create"});
        let key = action_key(&event);
        assert_eq!(key, "file_operation_metadata:create:xlsx");
        assert!(!key.contains("secret-project"));
        assert!(!key.contains("/home/alice"));
    }

    #[test]
    fn browser_domain_pattern_is_flagged_as_containing_verbatim_strings() {
        let events: Vec<EventSummary> = [0, 100, 200]
            .into_iter()
            .enumerate()
            .map(|(index, start)| {
                let mut event = event_at(
                    start,
                    "linux.browser",
                    SignalType::BrowserDomainVisited,
                    index as i64 + 1,
                );
                event.summary = serde_json::json!({"domain": "example.com"});
                event
            })
            .collect();
        let detector = PatternDetector::new(1..=1, 3);
        let detected = detector.detect(&events);
        assert_eq!(detected.len(), 1);
        assert!(detected[0].contains_verbatim_strings);
    }

    #[test]
    fn app_focus_change_pattern_is_not_flagged_as_verbatim() {
        let events: Vec<EventSummary> = [0, 100, 200]
            .into_iter()
            .enumerate()
            .map(|(index, start)| {
                let mut event = event_at(
                    start,
                    "windows.active_window",
                    SignalType::AppFocusChange,
                    index as i64 + 1,
                );
                event.summary = serde_json::json!({"app": "slack.exe"});
                event
            })
            .collect();
        let detector = PatternDetector::new(1..=1, 3);
        let detected = detector.detect(&events);
        assert_eq!(detected.len(), 1);
        assert!(!detected[0].contains_verbatim_strings);
    }
}
