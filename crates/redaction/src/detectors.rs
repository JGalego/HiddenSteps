use once_cell::sync::Lazy;
use regex::Regex;

use crate::entropy::{looks_like_common_hex_id, shannon_entropy};

/// What kind of sensitive content a detector found — mirrors the categories named
/// in `docs/design/05-privacy-model.md` §4 and PROMPT.md's Sensitive Information
/// Protection section.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Category {
    ApiKeyOrToken,
    Password,
    PrivateKey,
    Email,
    CreditCard,
    GovernmentId,
    Medical,
    /// A high-entropy token that doesn't match any *known* secret format but still
    /// looks secret-shaped. Per `docs/design/05-privacy-model.md` §4, this
    /// confidence tier triggers a full event drop, not a partial redaction —
    /// see `Confidence::Ambiguous`.
    AmbiguousSecret,
}

impl Category {
    pub fn label(self) -> &'static str {
        match self {
            Category::ApiKeyOrToken => "api_key_or_token",
            Category::Password => "password",
            Category::PrivateKey => "private_key",
            Category::Email => "email",
            Category::CreditCard => "credit_card",
            Category::GovernmentId => "government_id",
            Category::Medical => "medical",
            Category::AmbiguousSecret => "ambiguous_secret",
        }
    }
}

/// High-confidence detections are safely redactable in place (the match's exact
/// boundaries are trustworthy). Ambiguous detections are not — per the drop-on-
/// uncertainty policy, an ambiguous match anywhere in the text drops the whole
/// event rather than risk storing a mis-bounded partial redaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Confidence {
    High,
    Ambiguous,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Detection {
    pub start: usize,
    pub end: usize,
    pub category: Category,
    pub confidence: Confidence,
}

// --- Concrete, high-confidence secret formats ---

static AWS_ACCESS_KEY: Lazy<Regex> = Lazy::new(|| Regex::new(r"\bAKIA[0-9A-Z]{16}\b").unwrap());
static GITHUB_TOKEN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\bgh[pousr]_[0-9A-Za-z]{36,}\b").unwrap());
static SLACK_TOKEN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\bxox[baprs]-[0-9A-Za-z-]{10,}\b").unwrap());
static JWT: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\bey[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\b").unwrap()
});
static PEM_PRIVATE_KEY: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"-----BEGIN [A-Z ]*PRIVATE KEY-----[\s\S]*?-----END [A-Z ]*PRIVATE KEY-----")
        .unwrap()
});
/// `key=value`/`key: value` style assignments where the key name signals a secret.
/// Deliberately generous on the key side (password, api_key, apikey, secret,
/// token, access_key, ...) since the whole point is catching secrets the
/// format-specific regexes above miss.
static KEY_VALUE_SECRET: Lazy<Regex> = Lazy::new(|| {
    // Deliberately no leading `\b`: real-world keys are routinely written
    // `db_password`, `DB_PASSWORD`, `apiKey`, etc., where the keyword is not at a
    // `\w`/non-`\w` boundary. The trailing `\s*[:=]` requirement (immediately, or
    // separated only by whitespace) is what keeps this from false-firing on
    // unrelated words like "passwordless" — "less" sits between the keyword and
    // the required `:`/`=`, so it never matches there.
    Regex::new(
        r#"(?i)(?:password|passwd|pwd|api[_-]?key|apikey|secret|access[_-]?key|auth[_-]?token|bearer)\s*[:=]\s*['"]?([^\s'",;]{4,})['"]?"#,
    )
    .unwrap()
});

static EMAIL: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}\b").unwrap());
/// SSN-shaped 9-digit sequences in each of the three ways a real one is
/// commonly written: dashed, space-separated, and with no separator at all.
/// The dashless/space forms are much more generic-looking than the dashed
/// one (any isolated 9-digit number could be an order id, a phone number
/// without formatting, ...), so all three are additionally checked against
/// `is_plausible_ssn`'s real SSA structural rules rather than accepted on
/// shape alone — cutting down false positives without giving up on catching
/// a real SSN just because it happens to be missing its dashes.
static SSN_DASHED: Lazy<Regex> = Lazy::new(|| Regex::new(r"\b(\d{3})-(\d{2})-(\d{4})\b").unwrap());
static SSN_SPACED: Lazy<Regex> = Lazy::new(|| Regex::new(r"\b(\d{3}) (\d{2}) (\d{4})\b").unwrap());
static SSN_DASHLESS: Lazy<Regex> = Lazy::new(|| Regex::new(r"\b(\d{9})\b").unwrap());
static CREDIT_CARD_CANDIDATE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\b(?:\d[ -]?){13,19}\b").unwrap());
static MEDICAL_KEY_VALUE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?i)(?:diagnosis|patient(?:[_ ]?id)?|mrn|prescription|medical[_ ]record)\s*[:=]\s*['"]?([^\s'",;]{2,})['"]?"#)
        .unwrap()
});
/// A candidate ambiguous-secret token: long, mixed-character-class, no whitespace.
static HIGH_ENTROPY_CANDIDATE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\b[A-Za-z0-9+/_=.-]{20,}\b").unwrap());

pub fn detect_all(text: &str) -> Vec<Detection> {
    let mut detections = Vec::new();

    push_matches(
        &mut detections,
        text,
        &PEM_PRIVATE_KEY,
        Category::PrivateKey,
        Confidence::High,
    );
    push_matches(
        &mut detections,
        text,
        &AWS_ACCESS_KEY,
        Category::ApiKeyOrToken,
        Confidence::High,
    );
    push_matches(
        &mut detections,
        text,
        &GITHUB_TOKEN,
        Category::ApiKeyOrToken,
        Confidence::High,
    );
    push_matches(
        &mut detections,
        text,
        &SLACK_TOKEN,
        Category::ApiKeyOrToken,
        Confidence::High,
    );
    push_matches(
        &mut detections,
        text,
        &JWT,
        Category::ApiKeyOrToken,
        Confidence::High,
    );

    for m in KEY_VALUE_SECRET.captures_iter(text) {
        if let Some(value) = m.get(1) {
            detections.push(Detection {
                start: value.start(),
                end: value.end(),
                category: Category::Password,
                confidence: Confidence::High,
            });
        }
    }

    for m in MEDICAL_KEY_VALUE.captures_iter(text) {
        if let Some(value) = m.get(1) {
            detections.push(Detection {
                start: value.start(),
                end: value.end(),
                category: Category::Medical,
                confidence: Confidence::High,
            });
        }
    }

    push_matches(
        &mut detections,
        text,
        &EMAIL,
        Category::Email,
        Confidence::High,
    );
    for pattern in [&*SSN_DASHED, &*SSN_SPACED] {
        for m in pattern.captures_iter(text) {
            let full = m.get(0).unwrap();
            let (area, group, serial) = (&m[1], &m[2], &m[3]);
            if is_plausible_ssn(area, group, serial) {
                detections.push(Detection {
                    start: full.start(),
                    end: full.end(),
                    category: Category::GovernmentId,
                    confidence: Confidence::High,
                });
            }
        }
    }
    for m in SSN_DASHLESS.captures_iter(text) {
        let full = m.get(0).unwrap();
        let digits = &m[1];
        let (area, group, serial) = (&digits[0..3], &digits[3..5], &digits[5..9]);
        if is_plausible_ssn(area, group, serial) {
            detections.push(Detection {
                start: full.start(),
                end: full.end(),
                category: Category::GovernmentId,
                confidence: Confidence::High,
            });
        }
    }

    for m in CREDIT_CARD_CANDIDATE.find_iter(text) {
        if let Some((start, end)) = find_luhn_valid_window(m.as_str(), m.start()) {
            detections.push(Detection {
                start,
                end,
                category: Category::CreditCard,
                confidence: Confidence::High,
            });
        }
    }

    // Ambiguous high-entropy tokens: only consider spans not already claimed by a
    // high-confidence detector above, and skip common non-secret hex-id shapes.
    for m in HIGH_ENTROPY_CANDIDATE.find_iter(text) {
        let already_claimed = detections
            .iter()
            .any(|d| ranges_overlap(d.start, d.end, m.start(), m.end()));
        if already_claimed {
            continue;
        }
        let token = m.as_str();
        if looks_like_common_hex_id(token) {
            continue;
        }
        let has_letter = token.chars().any(|c| c.is_ascii_alphabetic());
        let has_digit_or_symbol = token.chars().any(|c| !c.is_ascii_alphabetic());
        // Deliberately no mixed-case requirement: a real high-entropy secret
        // that happens to be all-lowercase or all-uppercase (many real-world
        // API/session tokens are lowercase hex or uppercase base32, e.g. TOTP
        // seeds) is exactly as much of a secret as a mixed-case one, and case
        // alone says nothing about randomness. `looks_like_common_hex_id`
        // above already carves out the common non-secret hex-id lengths
        // (git SHAs, MD5/SHA hashes) that would otherwise be the main source
        // of false positives this used to lean on case-mixing to avoid.
        if has_letter && has_digit_or_symbol && shannon_entropy(token) >= 4.0 {
            detections.push(Detection {
                start: m.start(),
                end: m.end(),
                category: Category::AmbiguousSecret,
                confidence: Confidence::Ambiguous,
            });
        }
    }

    detections.sort_by_key(|d| d.start);
    detections
}

fn push_matches(
    out: &mut Vec<Detection>,
    text: &str,
    pattern: &Regex,
    category: Category,
    confidence: Confidence,
) {
    for m in pattern.find_iter(text) {
        out.push(Detection {
            start: m.start(),
            end: m.end(),
            category,
            confidence,
        });
    }
}

fn ranges_overlap(a_start: usize, a_end: usize, b_start: usize, b_end: usize) -> bool {
    a_start < b_end && b_start < a_end
}

/// Real SSA structural rules for a Social Security Number: the area group
/// (first 3 digits) is never 000, 666, or 900-999; the group number (middle
/// 2 digits) is never 00; the serial (last 4 digits) is never 0000. Used to
/// keep the dashless/space-separated SSN detectors (which otherwise match
/// any isolated 9-digit number) from over-firing on unrelated ids and phone
/// numbers that happen to be 9 digits long.
fn is_plausible_ssn(area: &str, group: &str, serial: &str) -> bool {
    let area: u32 = area.parse().unwrap_or(0);
    let group: u32 = group.parse().unwrap_or(0);
    let serial: u32 = serial.parse().unwrap_or(0);
    area != 0 && area != 666 && area < 900 && group != 0 && serial != 0
}

/// Checks a credit-card candidate span for a Luhn-valid card number, allowing
/// for up to one stray extra digit adjacent to it (a mistyped check digit, a
/// concatenated short id) rather than only the whole span's digit count.
/// Checking only the full span misses a real card number that's otherwise
/// valid: e.g. a 16-digit card with one extra digit appended makes the
/// *combined* 17-digit span fail Luhn even though the genuine card number
/// within it still validates on its own.
///
/// Deliberately bounded to the full span's length and one-shorter, not all
/// the way down to 13 regardless of the candidate's total length — the
/// point is tolerating one stray digit next to a real card, not searching an
/// arbitrarily long digit run for any 13-digit substring that happens to
/// pass Luhn by chance, which would make the detector fire on far more
/// coincidental non-card digit runs (order numbers, phone numbers, ids).
/// Windows are tried longest-first so a shorter coincidental match doesn't
/// pre-empt the genuine full-length one. Returns the byte range (in the
/// original text) of the first valid window found.
fn find_luhn_valid_window(candidate: &str, candidate_start: usize) -> Option<(usize, usize)> {
    let digit_positions: Vec<(char, usize)> = candidate
        .char_indices()
        .filter(|(_, c)| c.is_ascii_digit())
        .map(|(i, c)| (c, candidate_start + i))
        .collect();
    let total = digit_positions.len();
    if total < 13 {
        return None;
    }
    let max_len = total.min(19);
    let min_len = max_len.saturating_sub(1).max(13);

    for window_len in (min_len..=max_len).rev() {
        for start_idx in 0..=(digit_positions.len() - window_len) {
            let window = &digit_positions[start_idx..start_idx + window_len];
            let digits: String = window.iter().map(|(c, _)| *c).collect();
            if luhn_valid(&digits) {
                let start = window[0].1;
                let end = window[window.len() - 1].1 + 1;
                return Some((start, end));
            }
        }
    }
    None
}

/// Standard Luhn checksum, used to reject plausible-but-invalid "13-19 digit"
/// candidates (phone numbers, order ids, ...) so the credit-card detector doesn't
/// over-fire on every long digit run.
fn luhn_valid(digits: &str) -> bool {
    let digits: Vec<u32> = digits.chars().filter_map(|c| c.to_digit(10)).collect();
    if digits.len() < 13 {
        return false;
    }
    let mut sum = 0u32;
    for (i, &d) in digits.iter().rev().enumerate() {
        if i % 2 == 1 {
            let doubled = d * 2;
            sum += if doubled > 9 { doubled - 9 } else { doubled };
        } else {
            sum += d;
        }
    }
    sum.is_multiple_of(10)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_aws_access_key() {
        let text = "export AWS_ACCESS_KEY_ID=AKIAIOSFODNN7EXAMPLE";
        let detections = detect_all(text);
        assert!(detections
            .iter()
            .any(|d| d.category == Category::ApiKeyOrToken && d.confidence == Confidence::High));
    }

    #[test]
    fn detects_github_token() {
        let text = "token: ghp_1234567890abcdefghijklmnopqrstuvwxyz";
        let detections = detect_all(text);
        assert!(detections
            .iter()
            .any(|d| d.category == Category::ApiKeyOrToken));
    }

    #[test]
    fn detects_jwt() {
        let text = "Authorization: Bearer eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dozjgNryP4J3jVmNHl0w5N_XgL0n3I9PlFUP0THsR8U";
        let detections = detect_all(text);
        assert!(detections
            .iter()
            .any(|d| d.category == Category::ApiKeyOrToken));
    }

    #[test]
    fn detects_pem_private_key_block() {
        let text =
            "-----BEGIN RSA PRIVATE KEY-----\nMIIBOgIBAAJBAK...\n-----END RSA PRIVATE KEY-----";
        let detections = detect_all(text);
        assert!(detections
            .iter()
            .any(|d| d.category == Category::PrivateKey));
    }

    #[test]
    fn detects_password_assignment() {
        let text = "db_password: SuperSecret123!";
        let detections = detect_all(text);
        assert!(detections.iter().any(|d| d.category == Category::Password));
    }

    #[test]
    fn detects_email_address() {
        let text = "contact jane.doe@example.com for details";
        let detections = detect_all(text);
        assert!(detections.iter().any(|d| d.category == Category::Email));
    }

    #[test]
    fn detects_ssn_shape() {
        let text = "SSN on file: 123-45-6789";
        let detections = detect_all(text);
        assert!(detections
            .iter()
            .any(|d| d.category == Category::GovernmentId));
    }

    #[test]
    fn detects_space_separated_ssn() {
        let text = "SSN on file: 123 45 6789";
        let detections = detect_all(text);
        assert!(detections
            .iter()
            .any(|d| d.category == Category::GovernmentId));
    }

    #[test]
    fn detects_dashless_ssn() {
        // Regression test: only the dashed format used to be recognized at
        // all — a real SSN typed or pasted without its separators passed
        // through completely unredacted, with no entropy fallback either
        // (it's pure digits, so the mixed-case-required ambiguous-secret
        // detector never considers it).
        let text = "SSN on file: 123456789";
        let detections = detect_all(text);
        assert!(detections
            .iter()
            .any(|d| d.category == Category::GovernmentId));
    }

    #[test]
    fn does_not_flag_a_structurally_invalid_ssn_shaped_number() {
        // Area 000 is never issued — an arbitrary 9-digit id/phone-like
        // number with this shape should not be treated as a real SSN.
        let text = "reference number 000456789";
        let detections = detect_all(text);
        assert!(!detections
            .iter()
            .any(|d| d.category == Category::GovernmentId));
    }

    #[test]
    fn detects_valid_luhn_credit_card_but_not_invalid_digit_runs() {
        // 4532015112830366 is a well-known Luhn-valid test number.
        let valid = detect_all("card 4532015112830366 on file");
        assert!(valid.iter().any(|d| d.category == Category::CreditCard));

        // A random 16-digit run that fails Luhn should not be flagged as a card.
        let invalid = detect_all("order number 1234567890123456");
        assert!(!invalid.iter().any(|d| d.category == Category::CreditCard));
    }

    #[test]
    fn detects_medical_key_value() {
        let text = "Diagnosis: type 2 diabetes";
        let detections = detect_all(text);
        assert!(detections.iter().any(|d| d.category == Category::Medical));
    }

    #[test]
    fn flags_high_entropy_token_as_ambiguous_not_high_confidence() {
        let text = "config value: xK9mQ2vP7zR4wN8tL1bH5cD3";
        let detections = detect_all(text);
        assert!(detections
            .iter()
            .any(|d| d.category == Category::AmbiguousSecret
                && d.confidence == Confidence::Ambiguous));
    }

    #[test]
    fn flags_an_all_lowercase_high_entropy_token_as_ambiguous() {
        // Regression test: the detector used to require both an uppercase
        // and a lowercase letter, so a real high-entropy secret that
        // happens to be all one case (many real API/session tokens are
        // lowercase hex or uppercase base32) was never flagged at all,
        // regardless of entropy.
        let text = "config value: xk9mq2vp7zr4wn8tl1bh5cd3";
        let detections = detect_all(text);
        assert!(detections
            .iter()
            .any(|d| d.category == Category::AmbiguousSecret
                && d.confidence == Confidence::Ambiguous));
    }

    #[test]
    fn flags_an_all_uppercase_high_entropy_token_as_ambiguous() {
        let text = "config value: XK9MQ2VP7ZR4WN8TL1BH5CD3";
        let detections = detect_all(text);
        assert!(detections
            .iter()
            .any(|d| d.category == Category::AmbiguousSecret
                && d.confidence == Confidence::Ambiguous));
    }

    #[test]
    fn does_not_flag_common_git_sha_as_ambiguous_secret() {
        let text = "fixed in commit a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0";
        let detections = detect_all(text);
        assert!(!detections
            .iter()
            .any(|d| d.category == Category::AmbiguousSecret));
    }

    #[test]
    fn does_not_flag_plain_english_prose() {
        let text = "opened the quarterly report and reviewed the summary";
        let detections = detect_all(text);
        assert!(detections.is_empty());
    }

    #[test]
    fn luhn_rejects_all_same_digit_runs() {
        assert!(!luhn_valid("1111111111111111"));
    }

    #[test]
    fn detects_a_valid_card_number_padded_with_an_extra_trailing_digit() {
        // Regression test: 4532015112830366 is Luhn-valid on its own, but the
        // Luhn check used to run over the *entire* matched 13-19-digit span,
        // so appending one extra digit (17 digits total) made the combined
        // span fail Luhn and the real card number was never flagged at all.
        let text = "card 45320151128303661 on file";
        let detections = detect_all(text);
        assert!(
            detections
                .iter()
                .any(|d| d.category == Category::CreditCard),
            "a genuine card number padded with an extra digit must still be detected"
        );
    }

    #[test]
    fn detects_a_valid_card_number_with_a_leading_extra_digit() {
        let text = "card 14532015112830366 on file";
        let detections = detect_all(text);
        assert!(detections
            .iter()
            .any(|d| d.category == Category::CreditCard));
    }
}
