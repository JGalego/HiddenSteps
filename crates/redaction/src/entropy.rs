/// Shannon entropy in bits/character — used to flag high-entropy tokens that don't
/// match any known secret *format* but still look secret-*shaped* (per
/// `docs/design/05-privacy-model.md` §4's "known secret-shaped contexts").
///
/// A truly random 64-character base62 secret has entropy near `log2(62) ≈ 5.95`;
/// English prose sits far lower (~4.0 and below once you account for letter
/// frequency skew); common non-secret hex identifiers (git SHAs, UUIDs without
/// dashes) sit in a middle band this module treats conservatively (see
/// `looks_like_common_hex_id`).
pub fn shannon_entropy(s: &str) -> f64 {
    if s.is_empty() {
        return 0.0;
    }
    let mut counts = [0usize; 256];
    let mut total = 0usize;
    for byte in s.bytes() {
        counts[byte as usize] += 1;
        total += 1;
    }
    let total = total as f64;
    counts
        .iter()
        .filter(|&&c| c > 0)
        .map(|&c| {
            let p = c as f64 / total;
            -p * p.log2()
        })
        .sum()
}

/// Common non-secret identifiers (git SHAs, UUIDs, hashes) are long hex strings
/// with high nominal entropy but are *not* secrets — flagging every one of them
/// would make the redaction engine useless in practice. This recognizes the most
/// common shapes so the entropy-based ambiguous-secret detector doesn't fire on
/// them.
pub fn looks_like_common_hex_id(s: &str) -> bool {
    let hex_len = s.chars().filter(|c| c.is_ascii_hexdigit()).count();
    let is_pure_hex = hex_len == s.len();
    if !is_pure_hex {
        return false;
    }
    matches!(s.len(), 7 | 8 | 32 | 40 | 64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_string_has_zero_entropy() {
        assert_eq!(shannon_entropy(""), 0.0);
    }

    #[test]
    fn repeated_character_has_zero_entropy() {
        assert_eq!(shannon_entropy("aaaaaaaaaa"), 0.0);
    }

    #[test]
    fn random_looking_mixed_string_has_high_entropy() {
        let entropy = shannon_entropy("Xk9$mQ2#pL7zR4vN8wT1");
        assert!(entropy > 3.5, "expected high entropy, got {entropy}");
    }

    #[test]
    fn low_character_diversity_token_has_lower_entropy_than_high_diversity_token() {
        // Mirrors how the detector actually uses this function: entropy is
        // computed per contiguous token (no whitespace), not over a full sentence
        // — a naive per-string Shannon entropy over a long, space-separated
        // sentence isn't a meaningful "is this a secret" signal (English prose
        // can have plenty of distinct symbols too), so the comparison that
        // matters is diversity within a same-shaped token.
        let low_diversity = shannon_entropy("aaaaaaaaaabbbbbbbbbb");
        let high_diversity = shannon_entropy("xQ9zM3kP7wN2vT8rL5cB");
        assert!(low_diversity < high_diversity);
    }

    #[test]
    fn recognizes_common_hex_id_lengths() {
        assert!(looks_like_common_hex_id("a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6")); // 32 (md5-like)
        assert!(looks_like_common_hex_id(
            "a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0" // 40 (sha1-like)
        ));
        assert!(!looks_like_common_hex_id("not-hex-at-all"));
        assert!(!looks_like_common_hex_id("a1b2c3")); // too short to match a known id length
    }
}
