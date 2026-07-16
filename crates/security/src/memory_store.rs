use std::collections::HashMap;
use std::sync::Mutex;

use crate::{SecretStore, SecretStoreError};

/// A `SecretStore` backed by process memory only — never touches disk or the OS
/// vault. Exists solely so Application-layer code can be unit-tested against the
/// `SecretStore` port (per ADR-0002) without requiring a real credential vault to be
/// available, which — per `docs/roadmap/03-testing-strategy.md` — is exactly the
/// kind of port a unit test should mock, in contrast to ports like the redaction
/// engine or the plugin sandbox that must be tested against real implementations.
///
/// Never used in production: nothing in this crate wires it up as the default.
#[derive(Default)]
pub struct InMemorySecretStore {
    values: Mutex<HashMap<String, Vec<u8>>>,
}

impl InMemorySecretStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl SecretStore for InMemorySecretStore {
    fn get(&self, key: &str) -> Result<Option<Vec<u8>>, SecretStoreError> {
        let values = self
            .values
            .lock()
            .map_err(|_| SecretStoreError::Backend("lock poisoned".into()))?;
        Ok(values.get(key).cloned())
    }

    fn set(&self, key: &str, value: &[u8]) -> Result<(), SecretStoreError> {
        let mut values = self
            .values
            .lock()
            .map_err(|_| SecretStoreError::Backend("lock poisoned".into()))?;
        values.insert(key.to_string(), value.to_vec());
        Ok(())
    }

    fn delete(&self, key: &str) -> Result<(), SecretStoreError> {
        let mut values = self
            .values
            .lock()
            .map_err(|_| SecretStoreError::Backend("lock poisoned".into()))?;
        values.remove(key);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_then_get_round_trips() {
        let store = InMemorySecretStore::new();
        store.set("db-master-key", b"secret-bytes").unwrap();
        assert_eq!(
            store.get("db-master-key").unwrap(),
            Some(b"secret-bytes".to_vec())
        );
    }

    #[test]
    fn missing_key_returns_none_not_an_error() {
        let store = InMemorySecretStore::new();
        assert_eq!(store.get("nonexistent").unwrap(), None);
    }

    #[test]
    fn delete_removes_the_value() {
        let store = InMemorySecretStore::new();
        store.set("key", b"value").unwrap();
        store.delete("key").unwrap();
        assert_eq!(store.get("key").unwrap(), None);
    }
}
