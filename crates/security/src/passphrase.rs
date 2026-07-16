use argon2::{Algorithm, Argon2, Params, Version};
use rand::RngCore;

/// A key derived from a user-supplied passphrase, plus the salt needed to re-derive
/// it later. Used only by Portable Mode (ADR-0008) — every other deployment mode
/// uses `generate_master_key` + the OS credential vault instead.
pub struct PassphraseKey {
    pub key: [u8; 32],
    pub salt: [u8; 16],
}

#[derive(Debug, thiserror::Error)]
pub enum PassphraseError {
    #[error("key derivation failed: {0}")]
    Derivation(String),
}

/// Derives a 256-bit key from `passphrase` using Argon2id, per
/// `docs/design/06-security-architecture.md` §1/§2.
///
/// If `salt` is `None`, a fresh random salt is generated (first-time setup); pass
/// the previously generated salt back in on every subsequent unlock attempt.
pub fn derive_key_from_passphrase(
    passphrase: &str,
    salt: Option<[u8; 16]>,
) -> Result<PassphraseKey, PassphraseError> {
    let salt = salt.unwrap_or_else(|| {
        let mut s = [0u8; 16];
        rand::rngs::OsRng.fill_bytes(&mut s);
        s
    });

    // Argon2id, memory-hard parameters chosen to resist offline brute-force against
    // a stolen Portable Mode data directory, while staying tolerable on modest
    // hardware at unlock time (this is a one-time-per-launch cost, not per-event).
    let params = Params::new(19 * 1024, 2, 1, Some(32))
        .map_err(|e| PassphraseError::Derivation(e.to_string()))?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

    let mut key = [0u8; 32];
    argon2
        .hash_password_into(passphrase.as_bytes(), &salt, &mut key)
        .map_err(|e| PassphraseError::Derivation(e.to_string()))?;

    Ok(PassphraseKey { key, salt })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_passphrase_and_salt_yields_the_same_key() {
        let first = derive_key_from_passphrase("correct-horse-battery-staple", None).unwrap();
        let second =
            derive_key_from_passphrase("correct-horse-battery-staple", Some(first.salt)).unwrap();
        assert_eq!(first.key, second.key);
    }

    #[test]
    fn different_passphrases_yield_different_keys() {
        let first = derive_key_from_passphrase("correct-horse-battery-staple", None).unwrap();
        let second = derive_key_from_passphrase("wrong-passphrase", Some(first.salt)).unwrap();
        assert_ne!(first.key, second.key);
    }

    #[test]
    fn fresh_salts_differ_across_calls() {
        let first = derive_key_from_passphrase("same-passphrase", None).unwrap();
        let second = derive_key_from_passphrase("same-passphrase", None).unwrap();
        assert_ne!(first.salt, second.salt);
        // Different salts mean different keys even for the same passphrase.
        assert_ne!(first.key, second.key);
    }
}
