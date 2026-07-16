use once_cell::sync::Lazy;
use regex::Regex;

/// Catches the LLM narratively contradicting the Layer 1 occurrence count in its
/// own prose (e.g. writing "you've only done this twice" when the real count is
/// 31). This does **not** guard against numeric drift in the stored
/// `estimated_time_saved_minutes` field — that field is never populated from the
/// LLM's output at all (see `prompt.rs`'s doc comment), so there is no value for
/// it to drift. This check exists for a different, real failure mode: a
/// contradiction would still be a wrong, trust-damaging statement even though it
/// can't corrupt a stored number.
static OCCURRENCE_MENTION: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)\b(\d+)\s*(times|occurrences|instances)\b").unwrap());

pub fn contradicts_occurrence_count(text: &str, actual_count: u32) -> bool {
    OCCURRENCE_MENTION.captures_iter(text).any(|m| {
        m.get(1)
            .and_then(|n| n.as_str().parse::<u32>().ok())
            .is_some_and(|mentioned| mentioned != actual_count)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_contradiction_when_the_mentioned_count_matches() {
        assert!(!contradicts_occurrence_count(
            "You've repeated this 31 times over two weeks.",
            31
        ));
    }

    #[test]
    fn flags_a_contradicting_count() {
        assert!(contradicts_occurrence_count(
            "You've only done this twice, so this is low priority. 2 occurrences total.",
            31
        ));
    }

    #[test]
    fn text_with_no_occurrence_language_is_never_flagged() {
        assert!(!contradicts_occurrence_count(
            "This looks like a good candidate for a Playwright script.",
            31
        ));
    }

    #[test]
    fn unrelated_numbers_do_not_trigger_a_false_positive() {
        assert!(!contradicts_occurrence_count(
            "This could save about 11 hours per month across 4 different reports.",
            31
        ));
    }
}
