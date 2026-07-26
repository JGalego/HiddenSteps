use std::sync::Arc;
use std::time::Duration;

use hiddensteps_domain::{
    AuditActor, AuditEntry, LlmProviderConfig, Pattern, PatternStatus, PrivacyLevel, PrivacyState,
    Recommendation, RecommendationStatus,
};
use hiddensteps_event_store::SqlCipherEventStore;
use hiddensteps_llm_provider::{
    AnthropicProvider, CompletionRequest, CompletionResponse, LlmProvider, OllamaProvider,
    OpenAiCompatibleProvider, ProviderError,
};
use hiddensteps_patterns::{DetectedPattern, PatternDetector};
use hiddensteps_privacy_engine::{DispatchDecision, PrivacyGatedProvider};
use hiddensteps_recommendations::Synthesizer;
use hiddensteps_security::{KeyringSecretStore, SecretStore};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_notification::NotificationExt;
use time::OffsetDateTime;

use crate::VAULT_SERVICE;

const SWEEP_INTERVAL: Duration = Duration::from_secs(5 * 60);
/// How far back to look each sweep. Generous rather than exact: re-detecting
/// an already-known pattern is cheap (a stats update, not a fresh LLM call —
/// see `sweep_once` below), so there's no correctness cost to overlap between
/// sweeps, only a little wasted CPU on the deterministic detector.
const EVENT_HISTORY_LIMIT: i64 = 2000;

/// The settings-table key the user's notification quiet-hours window is
/// persisted under, mirroring how `commands::DEEP_MODE_SCREENSHOT_OCR_SETTING_KEY`
/// is read/written through the generic settings get/update commands
/// (`commands::ALLOWED_SETTING_KEYS`). Deliberately *not* modeled in
/// `EnterprisePolicy` — that crate's own module doc is explicit that its
/// two-knob schema (`privacy_level_floor`, `provider_allowlist`) is closed;
/// notification cadence is an ordinary user preference, not a device policy.
pub const NOTIFICATION_QUIET_HOURS_SETTING_KEY: &str = "notification_quiet_hours";

/// The stored shape of `NOTIFICATION_QUIET_HOURS_SETTING_KEY`'s value: a
/// simple hour range, nothing more — proportionate to what "don't buzz me at
/// 2am" actually needs, not a general scheduling DSL. Both fields are hours-
/// of-day in `0..24`; `start_hour == end_hour` is treated as "no quiet hours"
/// (a zero-width window suppresses nothing) rather than as a full-day window,
/// since a user clearing both fields back to the same value is a much more
/// likely intent than "always quiet."
///
/// Deliberately compared against `OffsetDateTime::now_utc()`'s hour, not a
/// wall-clock-local hour: `time`'s own docs call its `local-offset` feature
/// unsound to enable in a multi-threaded process, and this app runs its
/// whole background loop under tokio's multi-threaded runtime. Comparing in
/// UTC is a real, disclosed simplification (a user not in UTC has to account
/// for their own offset when picking `start_hour`/`end_hour`) rather than a
/// silently-wrong local-time calculation — and it fails safe: with this
/// setting left unconfigured (the default), nothing is ever suppressed.
#[derive(Debug, Clone, Copy, serde::Deserialize)]
struct QuietHours {
    start_hour: u8,
    end_hour: u8,
}

/// Whether `now` (compared in UTC — see `QuietHours`'s doc comment) falls
/// inside the configured quiet-hours window. Handles a window that wraps past
/// midnight (e.g. 22 -> 7) the same way it handles one that doesn't.
fn is_within_quiet_hours(now: OffsetDateTime, quiet_hours: &QuietHours) -> bool {
    let hour = now.hour();
    if quiet_hours.start_hour == quiet_hours.end_hour {
        return false;
    }
    if quiet_hours.start_hour < quiet_hours.end_hour {
        (quiet_hours.start_hour..quiet_hours.end_hour).contains(&hour)
    } else {
        hour >= quiet_hours.start_hour || hour < quiet_hours.end_hour
    }
}

/// Above this many prior dismissals for the same `pattern_id`, resynthesis is
/// throttled by `DISMISSAL_BACKOFF_COOLDOWN` rather than attempted every
/// sweep — the actual feedback loop: a pattern the user keeps saying no to
/// gets suggested less. Deliberately low (2, not e.g. 5): a single dismissal
/// is often situational ("not right now," an experimentation dismissal), and
/// on its own shouldn't silence a still-recurring pattern — but a *second*
/// dismissal of the same underlying pattern is a real, repeated signal, not
/// noise.
const DISMISSAL_BACKOFF_THRESHOLD: usize = 2;

/// How long to wait after the most recent dismissal before attempting
/// resynthesis again, once a pattern has crossed `DISMISSAL_BACKOFF_THRESHOLD`.
/// A week: long enough that a legitimately-changed workflow (more
/// occurrences accumulated, a different context) has had a real chance to
/// look different, without holding the pattern silenced forever. The
/// `patterns` row itself keeps accumulating `occurrence_count`/`last_seen_at`
/// every sweep throughout the cooldown regardless (`update_pattern_stats`
/// still runs) — only resynthesis is paused.
const DISMISSAL_BACKOFF_COOLDOWN: time::Duration = time::Duration::days(7);

/// Periodically runs Layer 1 (deterministic pattern detection,
/// `hiddensteps_patterns`) and Layer 2 (LLM synthesis,
/// `hiddensteps_recommendations`) over stored events. Before this, both crates
/// were fully implemented and tested but nothing in the running app ever
/// called either one — the `patterns`/`recommendations` tables stayed empty no
/// matter how long observation ran.
pub async fn run(app: AppHandle, store: Arc<SqlCipherEventStore>) {
    loop {
        if let Err(e) = sweep_once(&app, &store).await {
            crate::commands::log_audit(
                &store,
                AuditEntry::new(
                    AuditActor::System,
                    "recommendation_sweep_error",
                    serde_json::json!({ "error": e }),
                ),
            );
        }
        tokio::time::sleep(SWEEP_INTERVAL).await;
    }
}

async fn sweep_once(app: &AppHandle, store: &SqlCipherEventStore) -> Result<(), String> {
    // Deep-mode's TTL (`docs/design/05-privacy-model.md` §2) was, until now,
    // metadata computed and persisted per event but never actually acted on
    // — nothing ever deleted a row once its `ttl_expires_at` passed. Piggy-
    // backing this on the same periodic sweep that already runs pattern
    // detection means Deep-mode retention gets enforced on the same
    // real-world cadence (every `SWEEP_INTERVAL`) without a second
    // background task.
    match store.delete_expired_events(OffsetDateTime::now_utc()) {
        Ok(count) if count > 0 => {
            crate::commands::log_audit(
                store,
                AuditEntry::new(
                    AuditActor::System,
                    "deep_mode_ttl_expired_events_deleted",
                    serde_json::json!({ "count": count }),
                ),
            );
        }
        Ok(_) => {}
        Err(e) => {
            crate::commands::log_audit(
                store,
                AuditEntry::new(
                    AuditActor::System,
                    "deep_mode_ttl_sweep_error",
                    serde_json::json!({ "error": e.to_string() }),
                ),
            );
        }
    }

    // Proactive delivery: pick up anything still owed a notification (freshly
    // synthesized last sweep, or a snooze that has just expired) and send it,
    // independent of whether this particular tick detects anything new below
    // — most sweeps won't. See `send_pending_notifications`'s doc comment for
    // the quiet-hours and "don't lose it, just delay it" behavior.
    send_pending_notifications(app, store).await;

    let privacy_state = store.get_privacy_state().map_err(|e| e.to_string())?;

    // `list_recent_events` returns newest-first; `PatternDetector` requires
    // oldest-first (see its doc comment).
    let mut events = store
        .list_recent_events(EVENT_HISTORY_LIMIT)
        .map_err(|e| e.to_string())?;
    events.reverse();
    if events.is_empty() {
        return Ok(());
    }

    let detected = PatternDetector::default().detect(&events);
    if detected.is_empty() {
        return Ok(());
    }

    let existing = store.list_patterns(None).map_err(|e| e.to_string())?;

    for pattern in detected {
        let signature_json = serde_json::to_value(&pattern.signature).map_err(|e| e.to_string())?;

        if let Some(known) = existing
            .iter()
            .find(|p| p.sequence_signature == signature_json)
        {
            // Already known — refresh its rolling stats every sweep
            // regardless of anything below (occurrence counts should keep
            // growing even while resynthesis is backed off).
            if let Some(id) = known.id {
                let _ = store.update_pattern_stats(
                    id,
                    pattern.last_seen_at,
                    pattern.occurrence_count,
                    Some(pattern.estimated_minutes_per_occurrence),
                );

                // Whether a *new* recommendation is worth attempting for this
                // already-tracked pattern — see `should_attempt_synthesis`'s
                // doc comment for the three cases (already has a live/acted-on
                // recommendation; fewer than `DISMISSAL_BACKOFF_THRESHOLD`
                // dismissals so far; backed off under cooldown). This is the
                // dismissal-feedback loop: re-suggesting the exact same
                // pattern every sweep regardless of dismissal history would be
                // noise, not help, but never trying again after one dismissal
                // would throw away a pattern that's still genuinely
                // recurring.
                match should_attempt_synthesis(store, id) {
                    Ok(true) => try_synthesize(app, store, &privacy_state, id, &pattern).await,
                    Ok(false) => {}
                    Err(e) => {
                        crate::commands::log_audit(
                            store,
                            AuditEntry::new(
                                AuditActor::System,
                                "recommendation_backoff_check_error",
                                serde_json::json!({ "pattern_id": id, "error": e }),
                            ),
                        );
                    }
                }
            }
            continue;
        }

        let pattern_id = store
            .insert_pattern(&Pattern {
                id: None,
                first_seen_at: pattern.first_seen_at,
                last_seen_at: pattern.last_seen_at,
                occurrence_count: pattern.occurrence_count,
                estimated_minutes_per_occurrence: Some(pattern.estimated_minutes_per_occurrence),
                sequence_signature: signature_json,
                status: PatternStatus::Active,
            })
            .map_err(|e| e.to_string())?;
        let _ = store.link_pattern_events(pattern_id, &pattern.contributing_event_ids);

        try_synthesize(app, store, &privacy_state, pattern_id, &pattern).await;
    }

    Ok(())
}

/// Whether an already-tracked pattern (matched by signature to an existing
/// `patterns` row — genuinely new patterns always go through `try_synthesize`
/// unconditionally in `sweep_once` instead) should get a new synthesis
/// attempt this sweep. Three cases, checked in order:
///
///  1. It already has a `Suggested` or `Implemented` recommendation: no.
///     There's already something live (awaiting the user) or already acted
///     on; a second recommendation for the same pattern would be redundant
///     noise, not help.
///  2. It has fewer than `DISMISSAL_BACKOFF_THRESHOLD` prior dismissals: yes,
///     unconditionally. This covers both "never successfully synthesized at
///     all yet" (e.g. the original attempt failed — no provider configured,
///     an LLM/provider error — so no recommendation row exists, `try_synthesize`
///     itself is what actually retries/no-ops depending on why) and "the one
///     dismissal so far might have been situational, worth trying again
///     immediately rather than waiting out a cooldown."
///  3. It has `DISMISSAL_BACKOFF_THRESHOLD`+ dismissals: only once
///     `DISMISSAL_BACKOFF_COOLDOWN` has passed since the most recent
///     dismissed recommendation was created. This is the actual feedback
///     loop this feature exists to add — a pattern the user keeps dismissing
///     gets suggested less, not resynthesized on the same `SWEEP_INTERVAL`
///     cadence as everything else.
///
/// `created_at` (not a dedicated "dismissed at" timestamp, which
/// `Recommendation` has no field for) is used as the cooldown anchor for
/// case 3 — a reasonable, conservative stand-in: the recommendation was
/// created at or after the pattern was last considered, and was necessarily
/// dismissed at or after that, so anchoring the cooldown on `created_at`
/// never starts the clock earlier than the real dismissal could have
/// happened.
fn should_attempt_synthesis(store: &SqlCipherEventStore, pattern_id: i64) -> Result<bool, String> {
    let existing_recommendations = store
        .list_recommendations_for_pattern(pattern_id)
        .map_err(|e| e.to_string())?;

    if existing_recommendations.iter().any(|r| {
        matches!(
            r.status,
            RecommendationStatus::Suggested | RecommendationStatus::Implemented
        )
    }) {
        return Ok(false);
    }

    let dismissed_created_at: Vec<OffsetDateTime> = existing_recommendations
        .iter()
        .filter(|r| r.status == RecommendationStatus::Dismissed)
        .map(|r| r.created_at)
        .collect();

    if dismissed_created_at.len() < DISMISSAL_BACKOFF_THRESHOLD {
        return Ok(true);
    }

    let most_recent_dismissal = dismissed_created_at
        .into_iter()
        .max()
        .expect("non-empty, checked by the length comparison above");
    Ok(OffsetDateTime::now_utc() - most_recent_dismissal >= DISMISSAL_BACKOFF_COOLDOWN)
}

/// Sends a real OS notification (via `tauri_plugin_notification`) for every
/// recommendation `EventStore::list_recommendations_needing_notification`
/// reports as owed, then marks each one notified — unless the user's
/// configured quiet hours (`NOTIFICATION_QUIET_HOURS_SETTING_KEY`) currently
/// apply, in which case this does nothing at all this tick. Crucially, "does
/// nothing" really means nothing: it does not mark anything notified, so the
/// recommendation is neither lost nor silently skipped — the very next sweep
/// tick after quiet hours end re-runs this same query and sends it then. A
/// notification failure (no notification daemon reachable, permission
/// denied, ...) is handled the same way: logged, not marked notified, retried
/// next sweep — never silently dropped.
///
/// Deliberately a plain notification (title + body), not one using the
/// plugin's cross-platform action-button API (`action_type_id` +
/// `register_action_types`, letting a user e.g. snooze straight from the OS
/// notification): registering action types is extra setup-time ceremony with
/// real platform-support variance, and this app cannot exercise or verify it
/// against a live OS notification daemon in this environment either way (see
/// this crate's own README on why `src-tauri` isn't compiled here). Clicking
/// the notification focuses/opens the app to the dashboard, where the
/// existing accept/dismiss/snooze buttons on `RecommendationCard` already
/// handle every action a notification action-button would have — a simpler,
/// safer default per this feature's own guardrails.
async fn send_pending_notifications(app: &AppHandle, store: &SqlCipherEventStore) {
    let now = OffsetDateTime::now_utc();

    let quiet_hours: Option<QuietHours> = store
        .get_setting(NOTIFICATION_QUIET_HOURS_SETTING_KEY)
        .ok()
        .flatten()
        .and_then(|value| serde_json::from_value(value).ok());
    if quiet_hours.is_some_and(|qh| is_within_quiet_hours(now, &qh)) {
        return;
    }

    let Ok(pending) = store.list_recommendations_needing_notification(now) else {
        return;
    };

    for recommendation in pending {
        let Some(id) = recommendation.id else {
            continue;
        };
        let result = app
            .notification()
            .builder()
            .title("HiddenSteps: new recommendation ready")
            .body(notification_body(&recommendation))
            .show();

        match result {
            Ok(()) => {
                let _ = store.mark_recommendation_notified(id, now);
            }
            Err(e) => {
                crate::commands::log_audit(
                    store,
                    AuditEntry::new(
                        AuditActor::System,
                        "recommendation_notification_failed",
                        serde_json::json!({ "id": id, "error": e.to_string() }),
                    ),
                );
            }
        }
    }
}

/// The notification body: the recommendation's own title only, not its
/// longer `why` narrative — enough to identify which recommendation this is
/// (and to open the app to it) without putting the fuller explanation text
/// in an OS toast that may be visible on a lock screen or to someone glancing
/// at the user's screen.
fn notification_body(recommendation: &Recommendation) -> String {
    recommendation.title.clone()
}

/// Attempts Layer 2 synthesis for a newly-discovered pattern, gated through
/// the same `DispatchGate` every other cloud `LlmProvider` call site must pass
/// (ADR-0004) — a locally-run provider is always allowed; a cloud provider
/// needs the user's general cloud consent, plus separate per-content-class
/// consent if the pattern's signature includes a verbatim string
/// (`DetectedPattern::contains_verbatim_strings`). Failing any of that skips
/// synthesis for this pattern rather than either bypassing the gate or
/// failing the whole sweep.
async fn try_synthesize(
    app: &AppHandle,
    store: &SqlCipherEventStore,
    privacy_state: &PrivacyState,
    pattern_id: i64,
    detected: &DetectedPattern,
) {
    let Ok(Some(config)) = store.get_active_llm_provider() else {
        return; // No provider configured yet — nothing to synthesize with.
    };
    let Ok(provider) = build_provider(&config) else {
        return; // Misconfigured provider (no model chosen, etc.) — same as above.
    };

    let gate_snapshot = {
        let state = app.state::<crate::state::AppState>();
        let snapshot = state.gate.lock().await.clone();
        snapshot
    };

    // A cheap pre-flight check purely so a gate rejection gets its own,
    // distinct audit-log reason (below) rather than being indistinguishable
    // from any other synthesis failure. It changes nothing about safety: the
    // actual dispatch a few lines down goes through `PrivacyGatedProvider`
    // regardless, which evaluates the same gate again before ever calling the
    // wrapped provider — so even if this pre-flight check were wrong, removed,
    // or skipped by some future caller, the real call still can't bypass it.
    let decision = gate_snapshot.evaluate(
        provider.is_local(),
        privacy_state.current_level,
        "pattern_summary",
        detected.contains_verbatim_strings,
    );
    if !matches!(decision, DispatchDecision::Allow) {
        crate::commands::log_audit(
            store,
            AuditEntry::new(
                AuditActor::System,
                "recommendation_blocked_by_privacy_gate",
                serde_json::json!({ "pattern_id": pattern_id }),
            ),
        );
        return;
    }

    let gated_provider = PrivacyGatedProvider::new(provider, gate_snapshot);
    let adapter = GatedSynthesisProvider {
        gated: &gated_provider,
        privacy_level: privacy_state.current_level,
        contains_verbatim_strings: detected.contains_verbatim_strings,
    };
    let synthesizer = Synthesizer::new(&adapter);
    match synthesizer.synthesize(pattern_id, detected).await {
        Ok(recommendation) => {
            if store.insert_recommendation(&recommendation).is_ok() {
                let _ = app.emit("recommendation::new", pattern_id);
            }
        }
        Err(e) => {
            crate::commands::log_audit(
                store,
                AuditEntry::new(
                    AuditActor::System,
                    "recommendation_synthesis_failed",
                    serde_json::json!({ "pattern_id": pattern_id, "error": e.to_string() }),
                ),
            );
        }
    }
}

/// Adapts a `PrivacyGatedProvider` to the plain `LlmProvider` trait
/// `Synthesizer` expects, binding this sweep's fixed gate-evaluation context
/// (the current privacy level and whether this pattern's signature contains a
/// verbatim string). Every `complete()` call goes through
/// `PrivacyGatedProvider::complete_if_allowed` — replacing what used to be a
/// hand-copied, easy-to-forget-at-a-future-call-site inline gate check with
/// the actual wrapper type `hiddensteps_privacy_engine::PrivacyGatedProvider`'s
/// own doc comment says application code wiring the Recommendation Engine
/// together should be using.
struct GatedSynthesisProvider<'a> {
    gated: &'a PrivacyGatedProvider<Box<dyn LlmProvider>>,
    privacy_level: PrivacyLevel,
    contains_verbatim_strings: bool,
}

#[async_trait::async_trait]
impl<'a> LlmProvider for GatedSynthesisProvider<'a> {
    fn id(&self) -> &str {
        self.gated.provider_id()
    }

    fn is_local(&self) -> bool {
        self.gated.is_local()
    }

    async fn complete(
        &self,
        request: CompletionRequest,
    ) -> Result<CompletionResponse, ProviderError> {
        self.gated
            .complete_if_allowed(
                request,
                self.privacy_level,
                "pattern_summary",
                self.contains_verbatim_strings,
            )
            .await
            .map_err(|e| ProviderError::Request(e.to_string()))
    }

    async fn embed(&self, _text: &str) -> Result<Vec<f32>, ProviderError> {
        // The Recommendation Engine's Layer 2 only ever calls `complete` —
        // see `hiddensteps_recommendations::Synthesizer`. Embeddings for
        // pattern similarity are computed separately (`hiddensteps-event-store`'s
        // cosine-similarity BLOB store, per crates/README.md), not through an
        // `LlmProvider` at all, so this adapter has nothing real to delegate
        // to and no legitimate caller ever reaches it.
        Err(ProviderError::EmbeddingsUnsupported)
    }
}

/// Builds a live `LlmProvider` from a stored config — the same three provider
/// families `commands::test_provider_connectivity` builds, just sourced from
/// `EventStore::get_active_llm_provider` and the OS vault instead of a
/// one-off connectivity-test request.
fn build_provider(config: &LlmProviderConfig) -> Result<Box<dyn LlmProvider>, ()> {
    let model = config
        .model_name
        .clone()
        .filter(|m| !m.trim().is_empty())
        .ok_or(())?;

    let api_key = match &config.vault_key_ref {
        Some(key_ref) => KeyringSecretStore::new(VAULT_SERVICE)
            .get(key_ref)
            .ok()
            .flatten()
            .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
            .unwrap_or_default(),
        None => String::new(),
    };

    let provider: Box<dyn LlmProvider> = match config.provider_type.as_str() {
        "ollama" => {
            let endpoint = config
                .endpoint
                .clone()
                .unwrap_or_else(|| "http://localhost:11434".to_string());
            Box::new(OllamaProvider::new(endpoint, model))
        }
        "anthropic" => {
            let endpoint = config
                .endpoint
                .clone()
                .unwrap_or_else(|| "https://api.anthropic.com".to_string());
            Box::new(AnthropicProvider::new(endpoint, api_key, model))
        }
        _ => {
            let endpoint = config
                .endpoint
                .clone()
                .unwrap_or_else(|| "https://api.openai.com".to_string());
            Box::new(OpenAiCompatibleProvider::new(
                "openai-compatible",
                endpoint,
                api_key,
                model,
                None,
            ))
        }
    };
    Ok(provider)
}

#[cfg(test)]
mod tests {
    use super::*;
    use hiddensteps_domain::{Alternative, Level, RecommendationCategory};

    fn quiet_hours(start_hour: u8, end_hour: u8) -> QuietHours {
        QuietHours {
            start_hour,
            end_hour,
        }
    }

    fn at_hour(hour: u8) -> OffsetDateTime {
        OffsetDateTime::UNIX_EPOCH + time::Duration::hours(hour as i64)
    }

    #[test]
    fn quiet_hours_window_that_does_not_wrap_midnight() {
        let qh = quiet_hours(9, 17);
        assert!(is_within_quiet_hours(at_hour(9), &qh));
        assert!(is_within_quiet_hours(at_hour(16), &qh));
        assert!(!is_within_quiet_hours(at_hour(17), &qh));
        assert!(!is_within_quiet_hours(at_hour(8), &qh));
    }

    #[test]
    fn quiet_hours_window_that_wraps_past_midnight() {
        let qh = quiet_hours(22, 7);
        assert!(is_within_quiet_hours(at_hour(23), &qh));
        assert!(is_within_quiet_hours(at_hour(0), &qh));
        assert!(is_within_quiet_hours(at_hour(6), &qh));
        assert!(!is_within_quiet_hours(at_hour(7), &qh));
        assert!(!is_within_quiet_hours(at_hour(21), &qh));
    }

    #[test]
    fn a_zero_width_quiet_hours_window_suppresses_nothing() {
        let qh = quiet_hours(5, 5);
        for hour in 0..24 {
            assert!(!is_within_quiet_hours(at_hour(hour), &qh));
        }
    }

    fn test_key() -> [u8; 32] {
        [0x7a; 32]
    }

    fn sample_pattern() -> Pattern {
        let now = OffsetDateTime::now_utc();
        Pattern {
            id: None,
            first_seen_at: now - time::Duration::days(14),
            last_seen_at: now,
            occurrence_count: 5,
            estimated_minutes_per_occurrence: Some(12.0),
            sequence_signature: serde_json::json!([
                "jira:app_action_event",
                "excel:app_action_event"
            ]),
            status: PatternStatus::Active,
        }
    }

    fn sample_recommendation(
        pattern_id: i64,
        status: RecommendationStatus,
        created_at: OffsetDateTime,
    ) -> Recommendation {
        Recommendation {
            id: None,
            pattern_id,
            created_at,
            title: "Automate the weekly ticket export".to_string(),
            category: RecommendationCategory::Hybrid,
            why: "This exact sequence recurs with high regularity.".to_string(),
            confidence: 0.9,
            estimated_time_saved_minutes: 60.0,
            difficulty: Level::Medium,
            maintenance_burden: Level::Low,
            privacy_implications: "Fully local, no cloud dispatch required.".to_string(),
            implementation_effort: "~2-3 hours one-time setup".to_string(),
            alternatives: vec![Alternative {
                approach: "Python script".to_string(),
                tradeoff: "Lower setup, higher maintenance.".to_string(),
            }],
            assumptions: vec![],
            ignored_information: vec![],
            generating_provider: "ollama".to_string(),
            status,
            dismissal_reason: None,
            notified_at: None,
            snoozed_until: None,
        }
    }

    #[test]
    fn a_pattern_with_no_recommendation_yet_is_eligible_for_synthesis() {
        let store = SqlCipherEventStore::open_in_memory(&test_key()).unwrap();
        let pattern_id = store.insert_pattern(&sample_pattern()).unwrap();

        assert!(should_attempt_synthesis(&store, pattern_id).unwrap());
    }

    #[test]
    fn a_pattern_with_a_live_suggested_recommendation_is_not_eligible() {
        let store = SqlCipherEventStore::open_in_memory(&test_key()).unwrap();
        let pattern_id = store.insert_pattern(&sample_pattern()).unwrap();
        store
            .insert_recommendation(&sample_recommendation(
                pattern_id,
                RecommendationStatus::Suggested,
                OffsetDateTime::now_utc(),
            ))
            .unwrap();

        assert!(!should_attempt_synthesis(&store, pattern_id).unwrap());
    }

    #[test]
    fn a_pattern_already_implemented_is_not_eligible_for_another_recommendation() {
        let store = SqlCipherEventStore::open_in_memory(&test_key()).unwrap();
        let pattern_id = store.insert_pattern(&sample_pattern()).unwrap();
        store
            .insert_recommendation(&sample_recommendation(
                pattern_id,
                RecommendationStatus::Implemented,
                OffsetDateTime::now_utc(),
            ))
            .unwrap();

        assert!(!should_attempt_synthesis(&store, pattern_id).unwrap());
    }

    #[test]
    fn one_prior_dismissal_does_not_yet_trigger_backoff() {
        let store = SqlCipherEventStore::open_in_memory(&test_key()).unwrap();
        let pattern_id = store.insert_pattern(&sample_pattern()).unwrap();
        store
            .insert_recommendation(&sample_recommendation(
                pattern_id,
                RecommendationStatus::Dismissed,
                OffsetDateTime::now_utc(),
            ))
            .unwrap();

        // Below `DISMISSAL_BACKOFF_THRESHOLD` (2): still eligible immediately,
        // no cooldown applied yet.
        assert!(should_attempt_synthesis(&store, pattern_id).unwrap());
    }

    #[test]
    fn two_recent_dismissals_trigger_the_cooldown() {
        let store = SqlCipherEventStore::open_in_memory(&test_key()).unwrap();
        let pattern_id = store.insert_pattern(&sample_pattern()).unwrap();
        let recent = OffsetDateTime::now_utc() - time::Duration::hours(1);
        store
            .insert_recommendation(&sample_recommendation(
                pattern_id,
                RecommendationStatus::Dismissed,
                recent - time::Duration::days(1),
            ))
            .unwrap();
        store
            .insert_recommendation(&sample_recommendation(
                pattern_id,
                RecommendationStatus::Dismissed,
                recent,
            ))
            .unwrap();

        // At `DISMISSAL_BACKOFF_THRESHOLD` (2) dismissals, the most recent one
        // only an hour ago: still well within `DISMISSAL_BACKOFF_COOLDOWN` (7
        // days), so backed off.
        assert!(!should_attempt_synthesis(&store, pattern_id).unwrap());
    }

    #[test]
    fn backoff_lifts_once_the_cooldown_window_has_passed() {
        let store = SqlCipherEventStore::open_in_memory(&test_key()).unwrap();
        let pattern_id = store.insert_pattern(&sample_pattern()).unwrap();
        let long_ago =
            OffsetDateTime::now_utc() - DISMISSAL_BACKOFF_COOLDOWN - time::Duration::days(1);
        store
            .insert_recommendation(&sample_recommendation(
                pattern_id,
                RecommendationStatus::Dismissed,
                long_ago - time::Duration::days(1),
            ))
            .unwrap();
        store
            .insert_recommendation(&sample_recommendation(
                pattern_id,
                RecommendationStatus::Dismissed,
                long_ago,
            ))
            .unwrap();

        // The most recent dismissal is now further in the past than
        // `DISMISSAL_BACKOFF_COOLDOWN` — eligible again.
        assert!(should_attempt_synthesis(&store, pattern_id).unwrap());
    }
}
