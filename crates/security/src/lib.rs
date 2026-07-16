//! Key management for HiddenSteps: `SecretStore` (ADR-0008) and the Argon2id
//! Portable Mode key-derivation path (`docs/design/06-security-architecture.md` §2).
//!
//! Nothing in this crate ever writes secret material to a plaintext file — the two
//! `SecretStore` implementations here are "the real OS vault" and "in-memory, for
//! tests that exercise application logic without needing a real vault." There is
//! deliberately no third "write to a config file" implementation, because that
//! implementation shouldn't exist at all (per PROMPT.md's Security requirements).

mod keyring_store;
mod master_key;
mod memory_store;
mod passphrase;

pub use keyring_store::KeyringSecretStore;
pub use master_key::generate_master_key;
pub use memory_store::InMemorySecretStore;
pub use passphrase::{derive_key_from_passphrase, PassphraseKey};

/// A key-value secret store, backed by the OS credential vault in production.
///
/// Values are opaque byte strings (a 256-bit database key, a cloud-provider API
/// key, ...); this trait makes no assumption about what's stored under a given key,
/// so the `EventStore`'s `vault_key_ref` column (per
/// `docs/design/07-database-schema.md`) can be an opaque lookup key into whichever
/// implementation is active.
pub trait SecretStore: Send + Sync {
    fn get(&self, key: &str) -> Result<Option<Vec<u8>>, SecretStoreError>;
    fn set(&self, key: &str, value: &[u8]) -> Result<(), SecretStoreError>;
    fn delete(&self, key: &str) -> Result<(), SecretStoreError>;
}

#[derive(Debug, thiserror::Error)]
pub enum SecretStoreError {
    #[error("backend error: {0}")]
    Backend(String),
}
