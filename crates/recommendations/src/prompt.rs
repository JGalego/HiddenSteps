use hiddensteps_patterns::DetectedPattern;
use serde::Deserialize;
use time::format_description::well_known::Rfc3339;

/// The structured fields the LLM is asked to produce. Deliberately excludes any
/// frequency/timing number — `estimated_time_saved_minutes`,
/// `occurrence_count`, `first_seen_at`, `last_seen_at` are computed by
/// `Synthesizer` directly from `DetectedPattern` (Layer 1) and are never parsed
/// out of the LLM's response, per ADR-0010: "The LLM is never the source of the
/// frequency/timing numbers." There is no field in this struct the LLM's response
/// could populate that would end up overriding a Layer 1 number — the channel for
/// numeric drift doesn't exist here, rather than existing and being checked
/// after the fact.
#[derive(Debug, Clone, Deserialize)]
pub struct SynthesizedFields {
    pub title: String,
    pub category: String,
    pub why: String,
    pub confidence: f32,
    pub difficulty: String,
    pub maintenance_burden: String,
    pub privacy_implications: String,
    pub implementation_effort: String,
    pub alternatives: Vec<AlternativeFields>,
    pub assumptions: Vec<String>,
    pub ignored_information: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AlternativeFields {
    pub approach: String,
    pub tradeoff: String,
}

pub const SYSTEM_PROMPT: &str = "You are the Recommendation Engine inside HiddenSteps, a local-first workflow-intelligence tool. \
You are given a detected repeated workflow pattern — its action sequence, how many times it recurred, and over what time span — \
already computed by a deterministic detector. Your job is ONLY to judge and explain: pick the best remediation category, explain why, \
list alternatives, and state your confidence and assumptions. Do NOT invent or restate the frequency or time-span numbers in a way that \
contradicts what you were given — reason about them, but any specific count or duration you mention in prose must match exactly what \
was provided. Respond with a single JSON object only, no prose outside it, no markdown code fences, matching this shape: \
{\"title\": string, \"category\": one of [\"shortcut\",\"template\",\"script\",\"browser_automation\",\"rpa\",\"workflow_platform\",\"ai_agent\",\"hybrid\"], \
\"why\": string, \"confidence\": number between 0 and 1, \"difficulty\": one of [\"low\",\"medium\",\"high\"], \
\"maintenance_burden\": one of [\"low\",\"medium\",\"high\"], \"privacy_implications\": string, \"implementation_effort\": string, \
\"alternatives\": [{\"approach\": string, \"tradeoff\": string}], \"assumptions\": [string], \"ignored_information\": [string]}";

pub fn build_prompt(pattern_id_hint: &str, detected: &DetectedPattern) -> String {
    let total_minutes =
        detected.estimated_minutes_per_occurrence * detected.occurrence_count as f64;
    format!(
        "Detected pattern (internal id: {pattern_id_hint}):\n\
         - Action sequence: {}\n\
         - Observed {} times\n\
         - First seen: {}\n\
         - Last seen: {}\n\
         - Estimated time per occurrence: {:.1} minutes\n\
         - Estimated total time so far: {:.1} minutes\n\n\
         Produce the JSON object described in your instructions.",
        detected.signature.join(" → "),
        detected.occurrence_count,
        detected.first_seen_at.format(&Rfc3339).unwrap_or_default(),
        detected.last_seen_at.format(&Rfc3339).unwrap_or_default(),
        detected.estimated_minutes_per_occurrence,
        total_minutes,
    )
}

/// Extracts the first balanced `{...}` object from `text` — models routinely wrap
/// JSON in prose or markdown fences despite instructions not to; this recovers
/// the object without requiring the whole response to be nothing but JSON.
pub fn extract_json_object(text: &str) -> Option<&str> {
    let start = text.find('{')?;
    let mut depth = 0i32;
    for (offset, ch) in text[start..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&text[start..start + offset + 1]);
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_a_bare_json_object() {
        let text = r#"{"a": 1, "b": {"c": 2}}"#;
        assert_eq!(extract_json_object(text), Some(text));
    }

    #[test]
    fn extracts_json_wrapped_in_prose_and_markdown_fences() {
        let text = "Sure! Here's the analysis:\n```json\n{\"a\": 1}\n```\nHope that helps.";
        assert_eq!(extract_json_object(text), Some(r#"{"a": 1}"#));
    }

    #[test]
    fn returns_none_when_no_object_is_present() {
        assert_eq!(extract_json_object("no json here"), None);
    }
}
