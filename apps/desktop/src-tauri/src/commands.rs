//! Tauri commands, per `docs/design/09-api-specification.md` §2. Every command
//! here calls straight into an already-tested core crate — this file is glue,
//! not logic; if a command looks like it's making a decision, that decision
//! should already live in the crate it calls, not be duplicated here.

use std::time::Duration;

use hiddensteps_domain::{AuditActor, AuditEntry, LlmProviderConfig, PrivacyLevel, PrivacyState};
use hiddensteps_llm_provider::{
    default_candidates, detect, AnthropicProvider, CompletionRequest, DetectedRuntime, LlmProvider,
    OllamaProvider, OpenAiCompatibleProvider,
};
use hiddensteps_security::SecretStore;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};
use time::OffsetDateTime;

use crate::state::AppState;

fn to_err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

/// Appends an audit-log entry without letting a write failure there turn an
/// already-succeeded primary action into a command error. Every call site
/// below does the real mutation first and logs it second — before this, a
/// failed audit-log write (a full disk, a poisoned mutex — rare, but
/// possible) made `delete_events`/`set_privacy_level`/etc. return `Err` even
/// though the deletion/level-change/etc. had already gone through, which
/// could prompt a user to retry an action that had, in fact, already taken
/// effect.
pub(crate) fn log_audit(store: &hiddensteps_event_store::SqlCipherEventStore, entry: AuditEntry) {
    if let Err(e) = store.append_audit_entry(&entry) {
        eprintln!(
            "audit log write failed for action '{}': {e}",
            entry.action_type
        );
    }
}

/// The settings-table key general cloud-dispatch consent is persisted under —
/// there's no dedicated schema column for it (see `state::AppState`'s doc
/// comment on why the in-memory `DispatchGate` is rebuilt from this at every
/// launch rather than carried across restarts as separate state).
pub const CLOUD_CONSENT_SETTING_KEY: &str = "cloud_consent_general";

/// Whether the user has separately opted into Level 4's screenshot+OCR
/// capture (`hiddensteps_observation::ScreenshotSource` +
/// `hiddensteps_pipeline::OcrsTextExtractor`), per
/// `docs/design/05-privacy-model.md` §1's requirement that each Level-4
/// sub-capability be "explicit, separately-opted-in... each independently
/// toggleable" rather than bundled into the coarse level-4 selection alone.
/// `observation_loop` reads this every tick (alongside the current privacy
/// level) before ever calling `poll` on the screenshot source — turning this
/// off stops capture immediately, without needing a privacy-level change or
/// an app restart.
pub const DEEP_MODE_SCREENSHOT_OCR_SETTING_KEY: &str = "deep_mode_screenshot_ocr_enabled";

/// The settings-table key `observation_loop::resolve_browser_bridge_token`
/// persists the browser-extension pairing token under — generated once, on
/// first run, before this app's Tauri builder even starts (see that
/// function's doc comment). Not on `ALLOWED_SETTING_KEYS` below: unlike
/// `DEEP_MODE_SCREENSHOT_OCR_SETTING_KEY`, there's no legitimate reason for
/// the UI to *write* this key through the generic settings commands — only
/// read it, via the dedicated `get_browser_bridge_status` below.
pub const BROWSER_BRIDGE_TOKEN_SETTING_KEY: &str = "browser_bridge_token";

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
            error: Some(
                "No model selected — choose one before testing the connection.".to_string(),
            ),
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
            let endpoint = request
                .endpoint
                .unwrap_or_else(|| "http://localhost:11434".to_string());
            OllamaProvider::new(endpoint, model).complete(probe).await
        }
        "anthropic" => {
            let endpoint = request
                .endpoint
                .unwrap_or_else(|| "https://api.anthropic.com".to_string());
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
        Ok(_) => Ok(TestProviderConnectivityResponse {
            ok: true,
            error: None,
        }),
        Err(e) => Ok(TestProviderConnectivityResponse {
            ok: false,
            error: Some(e.to_string()),
        }),
    }
}

#[tauri::command]
pub async fn list_llm_providers(
    state: State<'_, AppState>,
) -> Result<Vec<LlmProviderConfig>, String> {
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
    if !state
        .enterprise_policy
        .lock()
        .await
        .is_provider_allowed(&request.id)
    {
        return Err(format!(
            "'{}' is not on this device's enterprise-approved provider list",
            request.id
        ));
    }

    let vault_key_ref = if let Some(api_key) = &request.api_key {
        let secret_store = hiddensteps_security::KeyringSecretStore::new(crate::VAULT_SERVICE);
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
    state
        .store
        .set_active_llm_provider(&request.id)
        .map_err(to_err)?;

    log_audit(
        &state.store,
        AuditEntry::new(
            AuditActor::User,
            "provider_changed",
            serde_json::json!({ "provider_id": request.id }),
        ),
    );
    Ok(true)
}

#[derive(Serialize, Deserialize)]
pub struct SetPrivacyLevelRequest {
    pub level: u8,
    /// The specific permission descriptors the user was shown and
    /// acknowledged for the level they're moving to (e.g.
    /// `["app_focus", "window_title"]`). Recorded verbatim in the audit log
    /// so the acknowledgment is an actual, inspectable record rather than a
    /// cosmetic gesture — and required to be non-empty when raising to any
    /// observing level, so a level change can't slip through claiming an
    /// acknowledgment that never carried what was acknowledged.
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
    let requested_level = PrivacyLevel::from_u8(request.level).map_err(to_err)?;

    // Raising to any level that actually observes requires the caller to
    // carry the permissions the user acknowledged for it (FR-17: no
    // observation without informed consent). Manual (level 0) observes
    // nothing, so it needs no acknowledgment. This makes the field a real
    // precondition rather than the cosmetic, never-read literal it used to be.
    if requested_level != PrivacyLevel::Manual && request.acknowledged_permissions.is_empty() {
        return Err(format!(
            "raising to privacy level {} requires acknowledging the permissions it introduces",
            requested_level.as_u8()
        ));
    }

    // An enterprise policy's privacy-level floor (docs/design/05-privacy-model.md
    // §6) can only raise what the user picked, never lower it — enforced here,
    // not silently accepted and left for some other layer to catch, since this
    // command is the one place a privacy level is actually written.
    let effective_level = state
        .enterprise_policy
        .lock()
        .await
        .effective_privacy_level(requested_level);

    let mut current = state.store.get_privacy_state().map_err(to_err)?;
    let old_level = current.current_level;
    current.current_level = effective_level;
    current.updated_at = OffsetDateTime::now_utc();
    state.store.set_privacy_state(&current).map_err(to_err)?;

    log_audit(
        &state.store,
        AuditEntry::new(
            AuditActor::User,
            "privacy_level_changed",
            serde_json::json!({
                "from": old_level.as_u8(),
                "requested": requested_level.as_u8(),
                "to": effective_level.as_u8(),
                "acknowledged_permissions": request.acknowledged_permissions,
            }),
        ),
    );

    Ok(SetPrivacyLevelResponse {
        effective_level: effective_level.as_u8(),
    })
}

#[tauri::command]
pub async fn complete_onboarding(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    let mut current = state.store.get_privacy_state().map_err(to_err)?;
    current.observation_active = true;
    current.updated_at = OffsetDateTime::now_utc();
    state.store.set_privacy_state(&current).map_err(to_err)?;

    log_audit(
        &state.store,
        AuditEntry::new(
            AuditActor::User,
            "observation_started",
            serde_json::json!({}),
        ),
    );

    // The observation loop itself is started once, unconditionally, at app
    // startup (see `main.rs`'s `setup`) — it idles until `observation_active`
    // flips true, which the write above just did. Spawning another instance
    // here would run two loops against the same sources concurrently.
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

#[derive(Serialize)]
pub struct PrivacyManifestStatus {
    pub current_manifest_version: i64,
    pub consented_manifest_version: i64,
    pub reconsent_required: bool,
}

/// Whether the user needs to re-consent before observation resumes — per
/// `docs/design/05-privacy-model.md` §5, required whenever a release has
/// changed what a privacy level captures since the user last consented.
/// `observation_loop` independently enforces this (it won't observe while
/// this is true); this command is how the UI surfaces *why* observation
/// might be paused instead of it looking indistinguishable from a plain
/// user-initiated pause.
#[tauri::command]
pub async fn get_privacy_manifest_status(
    state: State<'_, AppState>,
) -> Result<PrivacyManifestStatus, String> {
    let privacy_state = state.store.get_privacy_state().map_err(to_err)?;
    let current_manifest_version = hiddensteps_privacy_engine::CURRENT_MANIFEST_VERSION;
    Ok(PrivacyManifestStatus {
        current_manifest_version,
        consented_manifest_version: privacy_state.consented_manifest_version,
        reconsent_required: hiddensteps_privacy_engine::requires_reconsent(
            privacy_state.consented_manifest_version,
            current_manifest_version,
        ),
    })
}

/// Records that the user has seen and accepted whatever changed in the
/// current privacy manifest version, letting `observation_loop` resume.
#[tauri::command]
pub async fn acknowledge_privacy_manifest(state: State<'_, AppState>) -> Result<bool, String> {
    let mut current = state.store.get_privacy_state().map_err(to_err)?;
    current.consented_manifest_version = hiddensteps_privacy_engine::CURRENT_MANIFEST_VERSION;
    current.updated_at = OffsetDateTime::now_utc();
    state.store.set_privacy_state(&current).map_err(to_err)?;
    log_audit(
        &state.store,
        AuditEntry::new(
            AuditActor::User,
            "privacy_manifest_reconsented",
            serde_json::json!({ "version": hiddensteps_privacy_engine::CURRENT_MANIFEST_VERSION }),
        ),
    );
    Ok(true)
}

/// What Settings shows for the browser-extension bridge
/// (`hiddensteps_observation::BrowserBridgeSource`): the pairing token and
/// port the extension's options page needs, plus a best-effort read on
/// whether it looks like the extension is actually reaching this app.
#[derive(Serialize)]
pub struct BrowserBridgeStatus {
    pub token: String,
    pub port: u16,
    /// When a `browser_bridge.extension`-sourced event was last recorded, if
    /// ever. `None` doesn't necessarily mean the extension has never
    /// connected — see `receiving_data`'s doc comment for the same caveat.
    pub last_seen: Option<OffsetDateTime>,
    /// Derived, not tracked live: true when `last_seen` falls within a short
    /// recency window. This is a real, measured signal (an actual persisted
    /// event, the same `get_diagnostics` "real, measured data" standard
    /// applies) but an approximate one — a paired extension that's simply
    /// had no tab change in the last few minutes (or is capturing only a
    /// signal type the current privacy level doesn't allow yet) will show as
    /// not-yet-receiving here even though it's correctly paired and idle,
    /// not broken. A live connection tracker (the bridge's HTTP server
    /// reporting "request received at T" independent of whether that request
    /// produced a storable event) would be more precise and is a reasonable
    /// follow-up, not attempted here to avoid threading a second piece of
    /// live cross-task state through `AppState` for what recent-event
    /// recency already answers well enough for a status display.
    pub receiving_data: bool,
}

#[tauri::command]
pub async fn get_browser_bridge_status(
    state: State<'_, AppState>,
) -> Result<BrowserBridgeStatus, String> {
    let token = state
        .store
        .get_setting(BROWSER_BRIDGE_TOKEN_SETTING_KEY)
        .map_err(to_err)?
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default();

    // A bounded recent-events scan, not a dedicated "last event per source"
    // query — `EventStore` has no such lookup today (see this function's own
    // `receiving_data` doc comment on the resulting approximation), and
    // adding one is more schema/API surface than a status display needs.
    let recent = state.store.list_recent_events(50).map_err(to_err)?;
    let last_seen = recent
        .iter()
        .filter(|e| e.source_id == hiddensteps_observation::BrowserBridgeSource::SOURCE_ID)
        .map(|e| e.occurred_at)
        .max();
    let receiving_data = last_seen
        .map(|seen_at| OffsetDateTime::now_utc() - seen_at < time::Duration::minutes(5))
        .unwrap_or(false);

    Ok(BrowserBridgeStatus {
        token,
        port: hiddensteps_observation::BrowserBridgeSource::DEFAULT_PORT,
        last_seen,
        receiving_data,
    })
}

#[tauri::command]
pub async fn pause_observation(app: AppHandle, state: State<'_, AppState>) -> Result<bool, String> {
    let mut current = state.store.get_privacy_state().map_err(to_err)?;
    current.observation_active = false;
    current.updated_at = OffsetDateTime::now_utc();
    state.store.set_privacy_state(&current).map_err(to_err)?;
    log_audit(
        &state.store,
        AuditEntry::new(
            AuditActor::User,
            "observation_paused",
            serde_json::json!({}),
        ),
    );
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
    log_audit(
        &state.store,
        AuditEntry::new(
            AuditActor::User,
            "observation_resumed",
            serde_json::json!({}),
        ),
    );
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
pub async fn delete_events(
    state: State<'_, AppState>,
    event_ids: Vec<i64>,
) -> Result<usize, String> {
    let count = state.store.delete_events(&event_ids).map_err(to_err)?;
    log_audit(
        &state.store,
        AuditEntry::new(
            AuditActor::User,
            "events_deleted",
            serde_json::json!({ "count": count }),
        ),
    );
    Ok(count)
}

#[tauri::command]
pub async fn export_data(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let data = state.store.export_data().map_err(to_err)?;
    log_audit(
        &state.store,
        AuditEntry::new(AuditActor::User, "data_exported", serde_json::json!({})),
    );
    Ok(data)
}

#[tauri::command]
pub async fn delete_all_data(state: State<'_, AppState>) -> Result<bool, String> {
    // Per docs/design/03-data-flow-diagrams.md §4: clear the store AND
    // invalidate the encryption key a surviving copy of the file was
    // encrypted under, so that copy is unreadable. This rekeys the live
    // database in place to a brand new key and persists *that* new key to
    // the vault — rather than deleting the vault entry outright — because
    // the on-disk file itself isn't deleted here (the running connection
    // still has it open), so a launch that generated yet another,
    // unrelated random key would never be able to open it again. Rekeying
    // keeps the vault and the file in sync while still making any earlier
    // copy of the file permanently unreadable.
    state.store.delete_all_data().map_err(to_err)?;
    let new_key = hiddensteps_security::generate_master_key();
    state.store.rekey(&new_key).map_err(to_err)?;
    let secret_store = hiddensteps_security::KeyringSecretStore::new(crate::VAULT_SERVICE);
    secret_store
        .set(crate::MASTER_KEY_ENTRY, &*new_key)
        .map_err(to_err)?;
    Ok(true)
}

// --- Patterns & recommendations ---

#[tauri::command]
pub async fn list_patterns(
    state: State<'_, AppState>,
    status_filter: Option<String>,
) -> Result<Vec<hiddensteps_domain::Pattern>, String> {
    let filter = status_filter
        .map(|s| parse_pattern_status(&s))
        .transpose()?;
    state.store.list_patterns(filter).map_err(to_err)
}

#[tauri::command]
pub async fn list_recommendations(
    state: State<'_, AppState>,
    status_filter: Option<String>,
) -> Result<Vec<hiddensteps_domain::Recommendation>, String> {
    let filter = status_filter
        .map(|s| parse_recommendation_status(&s))
        .transpose()?;
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

// --- Cloud dispatch consent ---

/// Whether the user has granted general consent for pattern summaries to be
/// sent to a *cloud* `LlmProvider` for recommendation synthesis — checked by
/// `recommendation_loop`'s privacy gate before every cloud dispatch (ADR-0004).
/// Local providers never consult this at all; it only ever gates cloud calls.
#[tauri::command]
pub async fn get_cloud_consent(state: State<'_, AppState>) -> Result<bool, String> {
    Ok(matches!(
        state
            .store
            .get_setting(CLOUD_CONSENT_SETTING_KEY)
            .map_err(to_err)?,
        Some(serde_json::Value::Bool(true))
    ))
}

#[tauri::command]
pub async fn set_cloud_consent(state: State<'_, AppState>, granted: bool) -> Result<bool, String> {
    state
        .store
        .set_setting(CLOUD_CONSENT_SETTING_KEY, &serde_json::Value::Bool(granted))
        .map_err(to_err)?;

    let mut gate = state.gate.lock().await;
    if granted {
        gate.grant_general_cloud_consent();
    } else {
        gate.revoke_general_cloud_consent();
    }
    drop(gate);

    log_audit(
        &state.store,
        AuditEntry::new(
            AuditActor::User,
            "cloud_consent_changed",
            serde_json::json!({ "granted": granted }),
        ),
    );
    Ok(granted)
}

// --- Settings ---

/// The only setting keys the generic get/update commands may touch. The
/// settings table is a genuine key/value store, but exposing arbitrary
/// key access across the IPC boundary would let any webview code (or a
/// future plugin) read or clobber any key — including ones other commands
/// treat as trusted, like cloud-consent. Every key a UI legitimately needs
/// belongs on this list; anything else is a bug or an attempt to reach
/// somewhere it shouldn't, and is rejected rather than silently served.
const ALLOWED_SETTING_KEYS: &[&str] = &[
    CLOUD_CONSENT_SETTING_KEY,
    DEEP_MODE_SCREENSHOT_OCR_SETTING_KEY,
];

fn check_setting_key(key: &str) -> Result<(), String> {
    if ALLOWED_SETTING_KEYS.contains(&key) {
        Ok(())
    } else {
        Err(format!(
            "setting key '{key}' is not accessible via this command"
        ))
    }
}

#[tauri::command]
pub async fn get_settings(
    state: State<'_, AppState>,
    key: String,
) -> Result<Option<serde_json::Value>, String> {
    check_setting_key(&key)?;
    state.store.get_setting(&key).map_err(to_err)
}

#[tauri::command]
pub async fn update_settings(
    state: State<'_, AppState>,
    key: String,
    value: serde_json::Value,
) -> Result<bool, String> {
    check_setting_key(&key)?;
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

fn parse_recommendation_status(
    value: &str,
) -> Result<hiddensteps_domain::RecommendationStatus, String> {
    match value {
        "suggested" => Ok(hiddensteps_domain::RecommendationStatus::Suggested),
        "implemented" => Ok(hiddensteps_domain::RecommendationStatus::Implemented),
        "dismissed" => Ok(hiddensteps_domain::RecommendationStatus::Dismissed),
        other => Err(format!("unknown recommendation status '{other}'")),
    }
}
