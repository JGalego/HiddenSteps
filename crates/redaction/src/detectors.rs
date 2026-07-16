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
static SSN: Lazy<Regex> = Lazy::new(|| Regex::new(r"\b\d{3}-\d{2}-\d{4}\b").unwrap());
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
    push_matches(
        &mut detections,
        text,
        &SSN,
        Category::GovernmentId,
        Confidence::High,
    );

    for m in CREDIT_CARD_CANDIDATE.find_iter(text) {
        let digits: String = m.as_str().chars().filter(|c| c.is_ascii_digit()).collect();
        if digits.len() >= 13 && digits.len() <= 19 && luhn_valid(&digits) {
            detections.push(Detection {
                start: m.start(),
                end: m.end(),
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
        let mixed_case = token.chars().any(|c| c.is_ascii_uppercase())
            && token.chars().any(|c| c.is_ascii_lowercase());
        if has_letter && has_digit_or_symbol && mixed_case && shannon_entropy(token) >= 4.0 {
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
}
