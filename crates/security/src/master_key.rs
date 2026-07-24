use rand::rand_core::UnwrapErr;
use rand::Rng;
use zeroize::Zeroizing;

/// A 256-bit key, generated via the OS CSPRNG.
///
/// Per ADR-0008: the master key is generated randomly at first run and never
/// derived from a user-memorable password by default — that's this function.
/// The Portable Mode passphrase-derived alternative lives in `passphrase.rs`.
///
/// Returned in a `Zeroizing` wrapper so the key bytes are wiped from memory
/// when the holder drops them rather than lingering in freed memory — relevant
/// to the stolen-device threat this key exists to defend against
/// (`docs/research/06-threat-model.md`). It derefs to `[u8; 32]`, so callers
/// pass `&*key` where a `&[u8; 32]`/`&[u8]` is wanted.
pub fn generate_master_key() -> Zeroizing<[u8; 32]> {
    let mut key = Zeroizing::new([0u8; 32]);
    UnwrapErr(rand::rngs::SysRng).fill_bytes(&mut *key);
    key
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_full_width_keys() {
        let key = generate_master_key();
        assert_eq!(key.len(), 32);
    }

    #[test]
    fn generated_key_is_not_all_zero() {
        // A `Zeroizing` value is all-zero *after* drop, never on creation —
        // guards against a regression where the CSPRNG fill is skipped.
        let key = generate_master_key();
        assert!(key.iter().any(|&b| b != 0));
    }

    #[test]
    fn does_not_repeat_across_calls() {
        // Not a proof of CSPRNG quality, just a sanity check against an
        // accidental all-zero or static-key regression.
        let a = generate_master_key();
        let b = generate_master_key();
        assert_ne!(*a, *b);
    }
}
