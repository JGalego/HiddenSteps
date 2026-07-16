// See ../README.md — real, complete source, not compiled in this environment.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod observation_loop;
mod state;

use std::sync::Arc;

use hiddensteps_enterprise_policy::EnterprisePolicy;
use hiddensteps_event_store::SqlCipherEventStore;
use hiddensteps_privacy_engine::DispatchGate;
use hiddensteps_security::{generate_master_key, KeyringSecretStore, SecretStore};
use tokio::sync::Mutex;

const VAULT_SERVICE: &str = "com.hiddensteps.app";
const MASTER_KEY_ENTRY: &str = "db-master-key";

/// Retrieves the existing master key from the OS credential vault, or generates
/// and stores a fresh one on first run (ADR-0008) — the non-Portable-Mode path.
/// Portable Mode's Argon2id-passphrase-derived path
/// (`hiddensteps_security::derive_key_from_passphrase`) is a distinct startup
/// flow this `main` doesn't implement; wiring the onboarding choice between the
/// two is UI/first-run-flow work layered on top of this function, not part of
/// it.
fn resolve_master_key(secret_store: &KeyringSecretStore) -> [u8; 32] {
    if let Ok(Some(existing)) = secret_store.get(MASTER_KEY_ENTRY) {
        if existing.len() == 32 {
            let mut key = [0u8; 32];
            key.copy_from_slice(&existing);
            return key;
        }
    }
    let key = generate_master_key();
    // Best-effort: if the vault write fails, the app still runs against an
    // in-memory-only key for this session rather than refusing to start —
    // real UX handling of a vault failure (surfacing
    // `security::key_or_vault_error`) belongs in the setup step below, once an
    // `AppHandle` exists to emit on.
    let _ = secret_store.set(MASTER_KEY_ENTRY, &key);
    key
}

pub(crate) fn data_dir() -> std::path::PathBuf {
    dirs_next_data_dir().join("hiddensteps.db")
}

/// A minimal stand-in for the `dirs`/`directories` crate's platform-appropriate
/// app-data-directory resolution — written inline rather than adding a
/// dependency for one path. Respects each platform's real convention
/// (`%APPDATA%` on Windows, `~/Library/Application Support` on macOS,
/// `XDG_DATA_HOME`/`~/.local/share` on Linux) rather than falling through to a
/// temp directory whenever `HOME` happens to be unset — which is the normal
/// case on Windows, where this previously always landed in `%TEMP%`.
fn dirs_next_data_dir() -> std::path::PathBuf {
    #[cfg(target_os = "windows")]
    {
        if let Ok(appdata) = std::env::var("APPDATA") {
            return std::path::PathBuf::from(appdata).join("hiddensteps");
        }
    }
    #[cfg(target_os = "macos")]
    {
        if let Ok(home) = std::env::var("HOME") {
            return std::path::PathBuf::from(home)
                .join("Library/Application Support/hiddensteps");
        }
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
            return std::path::PathBuf::from(xdg).join("hiddensteps");
        }
        if let Ok(home) = std::env::var("HOME") {
            return std::path::PathBuf::from(home).join(".local/share/hiddensteps");
        }
    }
    std::env::temp_dir().join("hiddensteps")
}

fn main() {
    let secret_store = KeyringSecretStore::new(VAULT_SERVICE);
    let master_key = resolve_master_key(&secret_store);

    let dir = dirs_next_data_dir();
    std::fs::create_dir_all(&dir).expect("failed to create the HiddenSteps data directory");
    let store = Arc::new(
        SqlCipherEventStore::open(&data_dir(), &master_key)
            .expect("failed to open the encrypted HiddenSteps store"),
    );

    let app_state = state::AppState {
        store,
        gate: Mutex::new(DispatchGate::new()),
        enterprise_policy: Mutex::new(EnterprisePolicy::default()),
        observation_task: Mutex::new(None),
    };

    tauri::Builder::default()
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            commands::get_onboarding_state,
            commands::get_provider_detection,
            commands::test_provider_connectivity,
            commands::list_llm_providers,
            commands::set_ai_provider,
            commands::set_privacy_level,
            commands::complete_onboarding,
            commands::get_observation_status,
            commands::pause_observation,
            commands::resume_observation,
            commands::get_recent_events,
            commands::delete_events,
            commands::export_data,
            commands::delete_all_data,
            commands::list_patterns,
            commands::list_recommendations,
            commands::get_recommendation_detail,
            commands::set_recommendation_status,
            commands::get_settings,
            commands::update_settings,
            commands::get_audit_log,
            commands::get_diagnostics,
        ])
        .run(tauri::generate_context!())
        .expect("error while running the HiddenSteps Tauri application");
}
