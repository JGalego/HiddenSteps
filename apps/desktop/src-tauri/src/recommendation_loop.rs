use std::sync::Arc;
use std::time::Duration;

use hiddensteps_domain::{
    AuditActor, AuditEntry, LlmProviderConfig, Pattern, PatternStatus, PrivacyLevel, PrivacyState,
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
use time::OffsetDateTime;

use crate::VAULT_SERVICE;

const SWEEP_INTERVAL: Duration = Duration::from_secs(5 * 60);
/// How far back to look each sweep. Generous rather than exact: re-detecting
/// an already-known pattern is cheap (a stats update, not a fresh LLM call —
/// see `sweep_once` below), so there's no correctness cost to overlap between
/// sweeps, only a little wasted CPU on the deterministic detector.
const EVENT_HISTORY_LIMIT: i64 = 2000;

/// Periodically runs Layer 1 (deterministic pattern detection,
/// `hiddensteps_patterns`) and Layer 2 (LLM synthesis,
/// `hiddensteps_recommendations`) over stored events. Before this, both crates
/// were fully implemented and tested but nothing in the running app ever
/// called either one — the `patterns`/`recommendations` tables stayed empty no
/// matter how long observation ran.
pub async fn run(app: AppHandle, store: Arc<SqlCipherEventStore>) {
    loop {
        if let Err(e) = sweep_once(&app, &store).await {
            let _ = store.append_audit_entry(&AuditEntry::new(
                AuditActor::System,
                "recommendation_sweep_error",
                serde_json::json!({ "error": e }),
            ));
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
            let _ = store.append_audit_entry(&AuditEntry::new(
                AuditActor::System,
                "deep_mode_ttl_expired_events_deleted",
                serde_json::json!({ "count": count }),
            ));
        }
        Ok(_) => {}
        Err(e) => {
            let _ = store.append_audit_entry(&AuditEntry::new(
                AuditActor::System,
                "deep_mode_ttl_sweep_error",
                serde_json::json!({ "error": e.to_string() }),
            ));
        }
    }

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
            // Already known — refresh its rolling stats. Deliberately no new
            // synthesis call here: re-suggesting the same recommendation
            // every sweep would be noise, not help.
            if let Some(id) = known.id {
                let _ = store.update_pattern_stats(
                    id,
                    pattern.last_seen_at,
                    pattern.occurrence_count,
                    Some(pattern.estimated_minutes_per_occurrence),
                );
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
        let _ = store.append_audit_entry(&AuditEntry::new(
            AuditActor::System,
            "recommendation_blocked_by_privacy_gate",
            serde_json::json!({ "pattern_id": pattern_id }),
        ));
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
            let _ = store.append_audit_entry(&AuditEntry::new(
                AuditActor::System,
                "recommendation_synthesis_failed",
                serde_json::json!({ "pattern_id": pattern_id, "error": e.to_string() }),
            ));
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
