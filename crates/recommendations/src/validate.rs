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

/// A spelled-out compound number ("thirty-one", "thirty one") immediately
/// followed by an occurrence keyword. Checked before `SPELLED_SINGLE` below so
/// its span can be excluded there — otherwise "thirty-one times" would also
/// spuriously match "one times" via the single-word pattern.
static SPELLED_COMPOUND: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"(?i)\b(twenty|thirty|forty|fifty|sixty|seventy|eighty|ninety)[- ](one|two|three|four|five|six|seven|eight|nine)\s*(times|occurrences|instances)\b",
    )
    .unwrap()
});

/// A single spelled-out number word immediately followed by an occurrence
/// keyword — the class of contradiction the original digit-only regex missed
/// entirely (e.g. "you've only done this thirty-one times" catches the digit
/// form fine, but "only happened once" or "on seven occasions" have no digits
/// at all for `OCCURRENCE_MENTION` to see).
static SPELLED_SINGLE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"(?i)\b(one|two|three|four|five|six|seven|eight|nine|ten|eleven|twelve|thirteen|fourteen|fifteen|sixteen|seventeen|eighteen|nineteen|twenty|thirty|forty|fifty|sixty|seventy|eighty|ninety)\s*(times|occurrences|instances)\b",
    )
    .unwrap()
});

/// "once"/"twice" are themselves a complete count statement ("I did this
/// once") — they don't pair with "times"/"occurrences"/"instances" at all, so
/// neither pattern above would ever see them.
static ONCE_OR_TWICE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)\bonce\b|\btwice\b").unwrap());

fn ones_word_value(word: &str) -> Option<u32> {
    Some(match word.to_ascii_lowercase().as_str() {
        "one" => 1,
        "two" => 2,
        "three" => 3,
        "four" => 4,
        "five" => 5,
        "six" => 6,
        "seven" => 7,
        "eight" => 8,
        "nine" => 9,
        "ten" => 10,
        "eleven" => 11,
        "twelve" => 12,
        "thirteen" => 13,
        "fourteen" => 14,
        "fifteen" => 15,
        "sixteen" => 16,
        "seventeen" => 17,
        "eighteen" => 18,
        "nineteen" => 19,
        _ => return None,
    })
}

fn tens_word_value(word: &str) -> Option<u32> {
    Some(match word.to_ascii_lowercase().as_str() {
        "twenty" => 20,
        "thirty" => 30,
        "forty" => 40,
        "fifty" => 50,
        "sixty" => 60,
        "seventy" => 70,
        "eighty" => 80,
        "ninety" => 90,
        _ => return None,
    })
}

/// Every occurrence-count mention found in `text`, as `(start, end, value)`
/// byte-range-tagged matches — used so a single-word match already covered by
/// a compound match (see `SPELLED_COMPOUND`'s doc comment) can be skipped
/// rather than double-counted or misread.
fn mentioned_counts(text: &str) -> Vec<(usize, usize, u32)> {
    let mut mentions = Vec::new();

    for m in OCCURRENCE_MENTION.captures_iter(text) {
        if let Some(n) = m.get(1).and_then(|n| n.as_str().parse::<u32>().ok()) {
            let whole = m.get(0).unwrap();
            mentions.push((whole.start(), whole.end(), n));
        }
    }

    for m in SPELLED_COMPOUND.captures_iter(text) {
        if let (Some(tens), Some(ones)) = (tens_word_value(&m[1]), ones_word_value(&m[2])) {
            let whole = m.get(0).unwrap();
            mentions.push((whole.start(), whole.end(), tens + ones));
        }
    }

    for m in SPELLED_SINGLE.captures_iter(text) {
        let whole = m.get(0).unwrap();
        let inside_compound = mentions
            .iter()
            .any(|(start, end, _)| whole.start() >= *start && whole.end() <= *end);
        if inside_compound {
            continue;
        }
        if let Some(n) = ones_word_value(&m[1]).or_else(|| tens_word_value(&m[1])) {
            mentions.push((whole.start(), whole.end(), n));
        }
    }

    for m in ONCE_OR_TWICE.find_iter(text) {
        let n = if m.as_str().eq_ignore_ascii_case("once") {
            1
        } else {
            2
        };
        mentions.push((m.start(), m.end(), n));
    }

    mentions
}

pub fn contradicts_occurrence_count(text: &str, actual_count: u32) -> bool {
    mentioned_counts(text)
        .iter()
        .any(|(_, _, mentioned)| *mentioned != actual_count)
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

    #[test]
    fn flags_a_spelled_out_contradicting_count() {
        // Regression test: the original regex only matched literal digits, so
        // a purely spelled-out contradiction like this had nothing to trigger
        // on at all.
        assert!(contradicts_occurrence_count(
            "This has only happened seven times, so automation may not be worth it.",
            31
        ));
    }

    #[test]
    fn flags_only_once_as_a_contradiction_when_the_real_count_is_higher() {
        assert!(contradicts_occurrence_count(
            "Since this only happened once, it's probably not worth automating.",
            31
        ));
    }

    #[test]
    fn flags_twice_as_a_contradiction_when_the_real_count_is_higher() {
        assert!(contradicts_occurrence_count(
            "You've done this twice recently.",
            31
        ));
    }

    #[test]
    fn a_matching_spelled_out_compound_count_is_not_a_false_positive() {
        assert!(!contradicts_occurrence_count(
            "You've repeated this thirty-one times over two weeks.",
            31
        ));
    }

    #[test]
    fn a_matching_spelled_out_compound_count_with_a_space_is_not_a_false_positive() {
        assert!(!contradicts_occurrence_count(
            "You've repeated this thirty one times over two weeks.",
            31
        ));
    }

    #[test]
    fn a_compound_count_does_not_spuriously_match_its_trailing_ones_word_alone() {
        // Regression test for the compound-vs-single-word overlap: without
        // excluding spans already covered by a compound match, "thirty-one
        // times" would also match the single-word pattern on "one times"
        // and be misread as a contradiction (31 != 1) even though 31 is
        // exactly the real count.
        assert!(!contradicts_occurrence_count(
            "You've repeated this thirty-one times over two weeks.",
            31
        ));
    }

    #[test]
    fn flags_a_contradicting_spelled_out_compound_count() {
        assert!(contradicts_occurrence_count(
            "You've repeated this forty-two times over two weeks.",
            31
        ));
    }
}
