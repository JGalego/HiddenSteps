use base64::Engine;
use keyring::Entry;

use crate::{SecretStore, SecretStoreError};

/// The production `SecretStore`: Keychain on macOS, Credential Manager/DPAPI on
/// Windows, Secret Service/libsecret on Linux — via the `keyring` crate (ADR-0008).
///
/// Values are arbitrary bytes, but the underlying OS vault APIs the `keyring` crate
/// wraps store passwords as text, so values are base64-encoded before storage and
/// decoded on read. This is an encoding detail, not a security boundary — the vault
/// itself is what provides confidentiality here, not the base64 step.
///
/// Per-OS-user scoping (and therefore the shared-device isolation guarantee in
/// `docs/research/06-threat-model.md`) comes for free from the vault backend: each
/// OS user account has its own credential store.
pub struct KeyringSecretStore {
    service: String,
}

impl KeyringSecretStore {
    /// `service` namespaces every key this store touches within the OS vault (e.g.
    /// `"com.hiddensteps.app"`), so HiddenSteps never collides with another
    /// application's entries under the same OS user account.
    pub fn new(service: impl Into<String>) -> Self {
        Self {
            service: service.into(),
        }
    }

    fn entry(&self, key: &str) -> Result<Entry, SecretStoreError> {
        Entry::new(&self.service, key).map_err(|e| SecretStoreError::Backend(e.to_string()))
    }
}

impl SecretStore for KeyringSecretStore {
    fn get(&self, key: &str) -> Result<Option<Vec<u8>>, SecretStoreError> {
        let entry = self.entry(key)?;
        match entry.get_password() {
            Ok(encoded) => {
                let decoded = base64::engine::general_purpose::STANDARD
                    .decode(encoded)
                    .map_err(|e| SecretStoreError::Backend(e.to_string()))?;
                Ok(Some(decoded))
            }
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(SecretStoreError::Backend(e.to_string())),
        }
    }

    fn set(&self, key: &str, value: &[u8]) -> Result<(), SecretStoreError> {
        let entry = self.entry(key)?;
        let encoded = base64::engine::general_purpose::STANDARD.encode(value);
        entry
            .set_password(&encoded)
            .map_err(|e| SecretStoreError::Backend(e.to_string()))
    }

    fn delete(&self, key: &str) -> Result<(), SecretStoreError> {
        let entry = self.entry(key)?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(SecretStoreError::Backend(e.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // These exercise the real OS credential vault and are therefore not run as
    // part of the normal unit-test suite (a headless CI/dev container has no
    // Secret Service / Keychain / DPAPI session available — see
    // `docs/roadmap/03-testing-strategy.md` §2, which explicitly calls out that
    // OS-vault behavior belongs in integration testing on real target platforms,
    // not in a mocked unit test). Run manually with `cargo test -- --ignored` on a
    // machine with a real desktop session.
    #[test]
    #[ignore = "requires a real OS credential vault / desktop session"]
    fn set_get_delete_round_trip_against_the_real_vault() {
        let store = KeyringSecretStore::new("com.hiddensteps.test");
        store.set("integration-test-key", b"round-trip-me").unwrap();
        assert_eq!(
            store.get("integration-test-key").unwrap(),
            Some(b"round-trip-me".to_vec())
        );
        store.delete("integration-test-key").unwrap();
        assert_eq!(store.get("integration-test-key").unwrap(), None);
    }
}
