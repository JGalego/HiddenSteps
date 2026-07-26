// See ../README.md — real, complete source, not compiled in this environment.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod observation_loop;
mod ocr_models;
mod recommendation_loop;
mod state;

use std::sync::Arc;

use hiddensteps_enterprise_policy::EnterprisePolicy;
use hiddensteps_event_store::SqlCipherEventStore;
use hiddensteps_privacy_engine::DispatchGate;
use hiddensteps_security::{generate_master_key, KeyringSecretStore, SecretStore};
use tauri::Manager;
use tokio::sync::Mutex;
use zeroize::Zeroizing;

const VAULT_SERVICE: &str = "com.hiddensteps.app";
const MASTER_KEY_ENTRY: &str = "db-master-key";

/// Retrieves the existing master key from the OS credential vault, or generates
/// and stores a fresh one on first run (ADR-0008) — the non-Portable-Mode path.
/// Portable Mode's Argon2id-passphrase-derived path
/// (`hiddensteps_security::derive_key_from_passphrase`) is a distinct startup
/// flow this `main` doesn't implement; wiring the onboarding choice between the
/// two is UI/first-run-flow work layered on top of this function, not part of
/// it.
fn resolve_master_key(secret_store: &KeyringSecretStore) -> Zeroizing<[u8; 32]> {
    if let Ok(Some(existing)) = secret_store.get(MASTER_KEY_ENTRY) {
        if existing.len() == 32 {
            let mut key = Zeroizing::new([0u8; 32]);
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
    let _ = secret_store.set(MASTER_KEY_ENTRY, &*key);
    key
}

/// Applies an enterprise policy per `docs/design/05-privacy-model.md` §6 and
/// `docs/design/08-plugin-architecture.md` §6. The full `PolicyLoader` plugin
/// interface those docs describe (a connector to an enterprise
/// config-management system) isn't built — this is the interim, real
/// mechanism: an IT admin (or a Portable Mode / enterprise deployment script)
/// drops a `enterprise-policy.json` file into the app's data directory. If
/// present, it's parsed, persisted (so it keeps applying even if the file is
/// later removed — matching how an MDM-pushed profile behaves), and used for
/// this launch. If absent, whatever policy was last persisted is reused, so a
/// removed file doesn't silently revert an already-applied floor/allowlist.
/// No policy ever having been applied — the common case — falls through to
/// `EnterprisePolicy::default()`, which imposes no constraints at all.
fn resolve_enterprise_policy(
    data_dir: &std::path::Path,
    store: &SqlCipherEventStore,
) -> EnterprisePolicy {
    let policy_file = data_dir.join("enterprise-policy.json");
    if let Ok(contents) = std::fs::read_to_string(&policy_file) {
        match EnterprisePolicy::parse(&contents) {
            Ok(policy) => {
                let _ = store.set_enterprise_policy(&policy);
                return policy;
            }
            Err(_) => {
                // An unparsable policy file is not fatal to app startup —
                // fall through to whatever was last persisted (or the
                // no-constraints default) rather than refusing to launch.
            }
        }
    }
    store
        .get_enterprise_policy()
        .ok()
        .flatten()
        .unwrap_or_default()
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
            return std::path::PathBuf::from(home).join("Library/Application Support/hiddensteps");
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
    // The key has been applied to the open connection's PRAGMA; drop our copy
    // (wiping it, since it's `Zeroizing`) rather than holding it for the
    // process's whole lifetime.
    drop(master_key);

    // Consent granted through `commands::set_cloud_consent` is persisted as a
    // setting (there being no dedicated schema for it — see `state::AppState`'s
    // doc comment) and re-applied to a fresh, in-memory `DispatchGate` here on
    // every launch; the gate itself never survives a restart as an object.
    let mut gate = DispatchGate::new();
    if let Ok(Some(serde_json::Value::Bool(true))) =
        store.get_setting(commands::CLOUD_CONSENT_SETTING_KEY)
    {
        gate.grant_general_cloud_consent();
    }

    let enterprise_policy = resolve_enterprise_policy(&dir, &store);

    let app_state = state::AppState {
        store: store.clone(),
        gate: Mutex::new(gate),
        enterprise_policy: Mutex::new(enterprise_policy),
        observation_task: Mutex::new(None),
    };

    tauri::Builder::default()
        .manage(app_state)
        .setup(move |app| {
            let handle = app.handle().clone();
            let recommendation_store = store.clone();
            tauri::async_runtime::spawn(async move {
                recommendation_loop::run(handle, recommendation_store).await;
            });

            // The observation loop is started unconditionally here rather than
            // only from `complete_onboarding` — it re-reads persisted
            // `privacy_state` (observation_active/current_level) on every tick
            // and simply idles when observation hasn't been turned on yet, so
            // this is safe before onboarding completes and is what makes
            // observation actually resume on every subsequent launch, not just
            // the first one.
            let observation_handle = app.handle().clone();
            let observation_state_handle = app.handle().clone();
            let observation_task = tokio::spawn(async move {
                observation_loop::run(observation_handle, store).await;
            });
            tauri::async_runtime::spawn(async move {
                let state = observation_state_handle.state::<state::AppState>();
                *state.observation_task.lock().await = Some(observation_task);
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_onboarding_state,
            commands::get_provider_detection,
            commands::test_provider_connectivity,
            commands::list_llm_providers,
            commands::set_ai_provider,
            commands::set_privacy_level,
            commands::complete_onboarding,
            commands::get_observation_status,
            commands::get_privacy_manifest_status,
            commands::acknowledge_privacy_manifest,
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
            commands::get_cloud_consent,
            commands::set_cloud_consent,
            commands::get_settings,
            commands::update_settings,
            commands::get_audit_log,
            commands::get_diagnostics,
        ])
        .run(tauri::generate_context!())
        .expect("error while running the HiddenSteps Tauri application");
}
