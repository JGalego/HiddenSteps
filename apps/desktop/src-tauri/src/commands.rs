//! Tauri commands, per `docs/design/09-api-specification.md` §2. Every command
//! here calls straight into an already-tested core crate — this file is glue,
//! not logic; if a command looks like it's making a decision, that decision
//! should already live in the crate it calls, not be duplicated here.

use std::time::Duration;

use hiddensteps_domain::{AuditActor, AuditEntry, PrivacyLevel, PrivacyState};
use hiddensteps_llm_provider::{default_candidates, detect, DetectedRuntime};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};
use time::OffsetDateTime;

use crate::state::AppState;

fn to_err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

// --- Onboarding & setup ---

#[tauri::command]
pub async fn get_provider_detection() -> Result<Vec<DetectedRuntime>, String> {
    let client = reqwest::Client::new();
    Ok(detect(&client, &default_candidates(), Duration::from_millis(500)).await)
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

// --- Diagnostics ---

#[tauri::command]
pub async fn get_audit_log(
    state: State<'_, AppState>,
    limit: i64,
) -> Result<Vec<hiddensteps_domain::AuditEntry>, String> {
    state.store.list_audit_log(limit).map_err(to_err)
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
