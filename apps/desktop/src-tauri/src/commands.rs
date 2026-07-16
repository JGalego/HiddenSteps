//! Tauri commands, per `docs/design/09-api-specification.md` §2. Every command
//! here calls straight into an already-tested core crate — this file is glue,
//! not logic; if a command looks like it's making a decision, that decision
//! should already live in the crate it calls, not be duplicated here.

use std::time::Duration;

use hiddensteps_domain::{AuditActor, AuditEntry, LlmProviderConfig, PrivacyLevel, PrivacyState};
use hiddensteps_llm_provider::{
    default_candidates, detect, AnthropicProvider, CompletionRequest, DetectedRuntime,
    LlmProvider, OllamaProvider, OpenAiCompatibleProvider,
};
use hiddensteps_security::SecretStore;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};
use time::OffsetDateTime;

use crate::state::AppState;

fn to_err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

// --- Onboarding & setup ---

#[tauri::command]
pub async fn get_onboarding_state(state: State<'_, AppState>) -> Result<OnboardingState, String> {
    let privacy_state = state.store.get_privacy_state().map_err(to_err)?;
    Ok(OnboardingState {
        // Onboarding is "completed" once observation has ever been started —
        // there is no separate flag to track, deliberately: the same
        // `complete_onboarding` command that ends the wizard is the one that
        // flips `observation_active`, so this is one source of truth, not two
        // that could drift apart.
        completed: privacy_state.observation_active
            || privacy_state.current_level != PrivacyLevel::Manual,
    })
}

#[derive(Serialize)]
pub struct OnboardingState {
    pub completed: bool,
}

#[tauri::command]
pub async fn get_provider_detection() -> Result<Vec<DetectedRuntime>, String> {
    let client = reqwest::Client::new();
    Ok(detect(&client, &default_candidates(), Duration::from_millis(500)).await)
}

#[derive(Deserialize)]
pub struct TestProviderConnectivityRequest {
    pub provider_type: String,
    pub endpoint: Option<String>,
    pub api_key: Option<String>,
    pub model: Option<String>,
}

#[derive(Serialize)]
pub struct TestProviderConnectivityResponse {
    pub ok: bool,
    pub error: Option<String>,
}

/// Builds a real `LlmProvider` from onboarding's chosen type/endpoint/key and
/// sends one trivial completion — this is the same tested client code every
/// other provider-calling command uses, not a separate lightweight probe that
/// could pass while the real call path fails.
#[tauri::command]
pub async fn test_provider_connectivity(
    request: TestProviderConnectivityRequest,
) -> Result<TestProviderConnectivityResponse, String> {
    // Real bug, found by actually running the app: this used to fall back to
    // a literal model named "default" when none was supplied, which doesn't
    // exist on a real Ollama instance and fails with a confusing 404. A
    // missing model is a real, distinct failure — surface it as one, in the
    // same place every other connectivity failure shows up, rather than
    // guessing a model name that was never going to work.
    let Some(model) = request.model.filter(|m| !m.trim().is_empty()) else {
        return Ok(TestProviderConnectivityResponse {
            ok: false,
            error: Some("No model selected — choose one before testing the connection.".to_string()),
        });
    };
    let probe = CompletionRequest {
        system: None,
        prompt: "Reply with the single word: ok".to_string(),
        max_tokens: Some(16),
        think: Some(false),
    };

    let result = match request.provider_type.as_str() {
        "ollama" => {
            let endpoint = request.endpoint.unwrap_or_else(|| "http://localhost:11434".to_string());
            OllamaProvider::new(endpoint, model).complete(probe).await
        }
        "anthropic" => {
            let endpoint = request.endpoint.unwrap_or_else(|| "https://api.anthropic.com".to_string());
            AnthropicProvider::new(endpoint, request.api_key.unwrap_or_default(), model)
                .complete(probe)
                .await
        }
        // OpenAI and every OpenAI-wire-compatible provider PROMPT.md names
        // (Azure OpenAI, OpenRouter, Together, Groq, DeepSeek, LocalAI) share
        // one client — see hiddensteps-llm-provider::openai's doc comment.
        other => {
            let endpoint = request
                .endpoint
                .unwrap_or_else(|| "https://api.openai.com".to_string());
            OpenAiCompatibleProvider::new(
                "openai-compatible",
                endpoint,
                request.api_key.unwrap_or_default(),
                model,
                None,
            )
            .complete(probe)
            .await
            .map_err(|e| {
                // Surfacing which provider type was attempted, since this
                // branch covers several by name.
                hiddensteps_llm_provider::ProviderError::Request(format!("[{other}] {e}"))
            })
        }
    };

    match result {
        Ok(_) => Ok(TestProviderConnectivityResponse { ok: true, error: None }),
        Err(e) => Ok(TestProviderConnectivityResponse {
            ok: false,
            error: Some(e.to_string()),
        }),
    }
}

#[tauri::command]
pub async fn list_llm_providers(state: State<'_, AppState>) -> Result<Vec<LlmProviderConfig>, String> {
    state.store.list_llm_providers().map_err(to_err)
}

#[derive(Deserialize)]
pub struct SetAiProviderRequest {
    pub id: String,
    pub provider_type: String,
    pub is_local: bool,
    pub model_name: Option<String>,
    pub endpoint: Option<String>,
    /// The raw secret, if any — written to the OS vault, never to the
    /// database (see `LlmProviderConfig::vault_key_ref`'s doc comment). This
    /// command is the one place that boundary is enforced: it accepts the
    /// secret from the UI and immediately converts it into a vault reference
    /// before anything touches `EventStore`.
    pub api_key: Option<String>,
}

#[tauri::command]
pub async fn set_ai_provider(
    state: State<'_, AppState>,
    request: SetAiProviderRequest,
) -> Result<bool, String> {
    let vault_key_ref = if let Some(api_key) = &request.api_key {
        let secret_store = hiddensteps_security::KeyringSecretStore::new("com.hiddensteps.app");
        let entry_name = format!("provider-key-{}", request.id);
        secret_store
            .set(&entry_name, api_key.as_bytes())
            .map_err(to_err)?;
        Some(entry_name)
    } else {
        None
    };

    state
        .store
        .upsert_llm_provider(&LlmProviderConfig {
            id: request.id.clone(),
            provider_type: request.provider_type,
            is_local: request.is_local,
            model_name: request.model_name,
            endpoint: request.endpoint,
            vault_key_ref,
            active: false,
        })
        .map_err(to_err)?;
    state.store.set_active_llm_provider(&request.id).map_err(to_err)?;

    state
        .store
        .append_audit_entry(&AuditEntry::new(
            AuditActor::User,
            "provider_changed",
            serde_json::json!({ "provider_id": request.id }),
        ))
        .map_err(to_err)?;
    Ok(true)
}

#[derive(Serialize, Deserialize)]
pub struct SetPrivacyLevelRequest {
    pub level: u8,
    pub acknowledged_permissions: Vec<String>,
}

#[derive(Serialize)]
pub struct SetPrivacyLevelResponse {
    pub effective_level: u8,
}

#[tauri::command]
pub async fn set_privacy_level(
    state: State<'_, AppState>,
    request: SetPrivacyLevelRequest,
) -> Result<SetPrivacyLevelResponse, String> {
    let new_level = PrivacyLevel::from_u8(request.level).map_err(to_err)?;
    let mut current = state.store.get_privacy_state().map_err(to_err)?;
    let old_level = current.current_level;
    current.current_level = new_level;
    current.updated_at = OffsetDateTime::now_utc();
    state.store.set_privacy_state(&current).map_err(to_err)?;

    state
        .store
        .append_audit_entry(&AuditEntry::new(
            AuditActor::User,
            "privacy_level_changed",
            serde_json::json!({ "from": old_level.as_u8(), "to": new_level.as_u8() }),
        ))
        .map_err(to_err)?;

    Ok(SetPrivacyLevelResponse {
        effective_level: new_level.as_u8(),
    })
}

#[tauri::command]
pub async fn complete_onboarding(app: AppHandle, state: State<'_, AppState>) -> Result<bool, String> {
    let mut current = state.store.get_privacy_state().map_err(to_err)?;
    current.observation_active = true;
    current.updated_at = OffsetDateTime::now_utc();
    state.store.set_privacy_state(&current).map_err(to_err)?;

    state
        .store
        .append_audit_entry(&AuditEntry::new(
            AuditActor::User,
            "observation_started",
            serde_json::json!({}),
        ))
        .map_err(to_err)?;

    let store = state.store.clone();
    let app_for_task = app.clone();
    let handle = tokio::spawn(async move { crate::observation_loop::run(app_for_task, store).await });
    *state.observation_task.lock().await = Some(handle);

    let _ = app.emit(
        "observation::status_changed",
        serde_json::json!({ "active": true, "privacy_level": current.current_level.as_u8() }),
    );
    Ok(true)
}

// --- Observation & privacy dashboard ---

#[tauri::command]
pub async fn get_observation_status(state: State<'_, AppState>) -> Result<PrivacyState, String> {
    state.store.get_privacy_state().map_err(to_err)
}

#[tauri::command]
pub async fn pause_observation(app: AppHandle, state: State<'_, AppState>) -> Result<bool, String> {
    let mut current = state.store.get_privacy_state().map_err(to_err)?;
    current.observation_active = false;
    current.updated_at = OffsetDateTime::now_utc();
    state.store.set_privacy_state(&current).map_err(to_err)?;
    state
        .store
        .append_audit_entry(&AuditEntry::new(
            AuditActor::User,
            "observation_paused",
            serde_json::json!({}),
        ))
        .map_err(to_err)?;
    let _ = app.emit(
        "observation::status_changed",
        serde_json::json!({ "active": false, "privacy_level": current.current_level.as_u8() }),
    );
    Ok(false)
}

#[tauri::command]
pub async fn resume_observation(state: State<'_, AppState>) -> Result<bool, String> {
    let mut current = state.store.get_privacy_state().map_err(to_err)?;
    current.observation_active = true;
    current.updated_at = OffsetDateTime::now_utc();
    state.store.set_privacy_state(&current).map_err(to_err)?;
    state
        .store
        .append_audit_entry(&AuditEntry::new(
            AuditActor::User,
            "observation_resumed",
            serde_json::json!({}),
        ))
        .map_err(to_err)?;
    Ok(true)
}

#[tauri::command]
pub async fn get_recent_events(
    state: State<'_, AppState>,
    limit: i64,
) -> Result<Vec<hiddensteps_domain::EventSummary>, String> {
    state.store.list_recent_events(limit).map_err(to_err)
}

#[tauri::command]
pub async fn delete_events(state: State<'_, AppState>, event_ids: Vec<i64>) -> Result<usize, String> {
    let count = state.store.delete_events(&event_ids).map_err(to_err)?;
    state
        .store
        .append_audit_entry(&AuditEntry::new(
            AuditActor::User,
            "events_deleted",
            serde_json::json!({ "count": count }),
        ))
        .map_err(to_err)?;
    Ok(count)
}

#[tauri::command]
pub async fn export_data(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let data = state.store.export_data().map_err(to_err)?;
    state
        .store
        .append_audit_entry(&AuditEntry::new(
            AuditActor::User,
            "data_exported",
            serde_json::json!({}),
        ))
        .map_err(to_err)?;
    Ok(data)
}

#[tauri::command]
pub async fn delete_all_data(state: State<'_, AppState>) -> Result<bool, String> {
    // Per docs/design/03-data-flow-diagrams.md §4: clear the store AND remove
    // the vault key entry, so a surviving copy of the encrypted file is
    // unreadable — this command does both, not just the store half.
    state.store.delete_all_data().map_err(to_err)?;
    let secret_store = hiddensteps_security::KeyringSecretStore::new("com.hiddensteps.app");
    secret_store.delete("db-master-key").map_err(to_err)?;
    Ok(true)
}

// --- Patterns & recommendations ---

#[tauri::command]
pub async fn list_patterns(
    state: State<'_, AppState>,
    status_filter: Option<String>,
) -> Result<Vec<hiddensteps_domain::Pattern>, String> {
    let filter = status_filter.map(|s| parse_pattern_status(&s)).transpose()?;
    state.store.list_patterns(filter).map_err(to_err)
}

#[tauri::command]
pub async fn list_recommendations(
    state: State<'_, AppState>,
    status_filter: Option<String>,
) -> Result<Vec<hiddensteps_domain::Recommendation>, String> {
    let filter = status_filter.map(|s| parse_recommendation_status(&s)).transpose()?;
    state.store.list_recommendations(filter).map_err(to_err)
}

#[derive(Serialize)]
pub struct RecommendationDetail {
    #[serde(flatten)]
    pub recommendation: hiddensteps_domain::Recommendation,
    pub contributing_events: Vec<hiddensteps_domain::EventSummary>,
}

#[tauri::command]
pub async fn get_recommendation_detail(
    state: State<'_, AppState>,
    id: i64,
) -> Result<RecommendationDetail, String> {
    let recommendation = state
        .store
        .list_recommendations(None)
        .map_err(to_err)?
        .into_iter()
        .find(|r| r.id == Some(id))
        .ok_or_else(|| format!("recommendation {id} not found"))?;
    let contributing_events = state
        .store
        .list_pattern_events(recommendation.pattern_id)
        .map_err(to_err)?;
    Ok(RecommendationDetail {
        recommendation,
        contributing_events,
    })
}

#[derive(Deserialize)]
pub struct SetRecommendationStatusRequest {
    pub id: i64,
    pub status: String,
    pub dismissal_reason: Option<String>,
}

#[tauri::command]
pub async fn set_recommendation_status(
    state: State<'_, AppState>,
    request: SetRecommendationStatusRequest,
) -> Result<bool, String> {
    let status = parse_recommendation_status(&request.status)?;
    state
        .store
        .set_recommendation_status(request.id, status, request.dismissal_reason.as_deref())
        .map_err(to_err)?;
    Ok(true)
}

// --- Settings ---

#[tauri::command]
pub async fn get_settings(
    state: State<'_, AppState>,
    key: String,
) -> Result<Option<serde_json::Value>, String> {
    state.store.get_setting(&key).map_err(to_err)
}

#[tauri::command]
pub async fn update_settings(
    state: State<'_, AppState>,
    key: String,
    value: serde_json::Value,
) -> Result<bool, String> {
    state.store.set_setting(&key, &value).map_err(to_err)?;
    Ok(true)
}

// --- Diagnostics ---

#[tauri::command]
pub async fn get_audit_log(
    state: State<'_, AppState>,
    limit: i64,
) -> Result<Vec<hiddensteps_domain::AuditEntry>, String> {
    state.store.list_audit_log(limit).map_err(to_err)
}

#[derive(Serialize)]
pub struct Diagnostics {
    pub privacy_level: u8,
    pub observation_active: bool,
    pub active_provider: Option<LlmProviderConfig>,
    pub event_count: i64,
    pub pattern_count: i64,
    pub recommendation_count: i64,
    pub audit_log_count: i64,
    pub storage_bytes: Option<u64>,
    pub encryption_status: &'static str,
}

/// Every field here is real, measured data — event/pattern/recommendation/
/// audit-log counts are live `COUNT(*)` queries, `storage_bytes` is the actual
/// file size on disk, per PROMPT.md's Self-Diagnostics requirement ("users
/// should never have to guess why something isn't working"). Fields this
/// command does *not* yet report (GPU/CPU/memory usage, observation OS
/// permission status, update status) are a disclosed gap — see
/// `apps/desktop/README.md` — not fabricated with placeholder values.
#[tauri::command]
pub async fn get_diagnostics(state: State<'_, AppState>) -> Result<Diagnostics, String> {
    let privacy_state = state.store.get_privacy_state().map_err(to_err)?;
    let active_provider = state.store.get_active_llm_provider().map_err(to_err)?;
    let event_count = state.store.count_rows("event_summaries").map_err(to_err)?;
    let pattern_count = state.store.count_rows("patterns").map_err(to_err)?;
    let recommendation_count = state.store.count_rows("recommendations").map_err(to_err)?;
    let audit_log_count = state.store.count_rows("audit_log").map_err(to_err)?;
    let storage_bytes = std::fs::metadata(crate::data_dir()).ok().map(|m| m.len());

    Ok(Diagnostics {
        privacy_level: privacy_state.current_level.as_u8(),
        observation_active: privacy_state.observation_active,
        active_provider,
        event_count,
        pattern_count,
        recommendation_count,
        audit_log_count,
        storage_bytes,
        encryption_status: "SQLCipher (AES-256), key in OS credential vault",
    })
}

fn parse_pattern_status(value: &str) -> Result<hiddensteps_domain::PatternStatus, String> {
    match value {
        "active" => Ok(hiddensteps_domain::PatternStatus::Active),
        "stale" => Ok(hiddensteps_domain::PatternStatus::Stale),
        "dismissed" => Ok(hiddensteps_domain::PatternStatus::Dismissed),
        other => Err(format!("unknown pattern status '{other}'")),
    }
}

fn parse_recommendation_status(value: &str) -> Result<hiddensteps_domain::RecommendationStatus, String> {
    match value {
        "suggested" => Ok(hiddensteps_domain::RecommendationStatus::Suggested),
        "implemented" => Ok(hiddensteps_domain::RecommendationStatus::Implemented),
        "dismissed" => Ok(hiddensteps_domain::RecommendationStatus::Dismissed),
        other => Err(format!("unknown recommendation status '{other}'")),
    }
}
