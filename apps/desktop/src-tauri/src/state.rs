use std::sync::Arc;

use hiddensteps_enterprise_policy::EnterprisePolicy;
use hiddensteps_event_store::SqlCipherEventStore;
use hiddensteps_privacy_engine::DispatchGate;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

/// Everything a Tauri command needs, managed via `tauri::Builder::manage`.
/// `store` is the one encrypted file (ADR-0003); everything else is
/// in-memory state that doesn't survive a restart by design (the gate's
/// consent grants and the observation loop handle are re-established from
/// `store`'s persisted `privacy_state` each launch, not carried across restarts
/// as separate state).
pub struct AppState {
    pub store: Arc<SqlCipherEventStore>,
    pub gate: Mutex<DispatchGate>,
    pub enterprise_policy: Mutex<EnterprisePolicy>,
    pub observation_task: Mutex<Option<JoinHandle<()>>>,
}
