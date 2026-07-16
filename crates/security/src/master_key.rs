use rand::RngCore;

/// A 256-bit key, generated via the OS CSPRNG.
///
/// Per ADR-0008: the master key is generated randomly at first run and never
/// derived from a user-memorable password by default — that's this function.
/// The Portable Mode passphrase-derived alternative lives in `passphrase.rs`.
pub fn generate_master_key() -> [u8; 32] {
    let mut key = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut key);
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
    fn does_not_repeat_across_calls() {
        // Not a proof of CSPRNG quality, just a sanity check against an
        // accidental all-zero or static-key regression.
        let a = generate_master_key();
        let b = generate_master_key();
        assert_ne!(a, b);
    }
}
