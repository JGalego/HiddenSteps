use serde::{Deserialize, Serialize};

/// Mirrors the `llm_providers` table in `docs/design/07-database-schema.md`.
/// The actual secret (a cloud API key) never lives on this type or in this
/// table — `vault_key_ref` is an opaque reference into the OS credential vault
/// (ADR-0008); compromising this row alone yields no credential.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LlmProviderConfig {
    pub id: String,
    pub provider_type: String,
    pub is_local: bool,
    pub model_name: Option<String>,
    pub endpoint: Option<String>,
    pub vault_key_ref: Option<String>,
    pub active: bool,
}
