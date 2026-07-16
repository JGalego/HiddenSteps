use hiddensteps_domain::{
    Alternative, Level, Recommendation, RecommendationCategory, RecommendationStatus,
};
use hiddensteps_llm_provider::{CompletionRequest, LlmProvider, ProviderError};
use hiddensteps_patterns::DetectedPattern;
use time::OffsetDateTime;

use crate::prompt::{build_prompt, extract_json_object, SynthesizedFields, SYSTEM_PROMPT};
use crate::validate::contradicts_occurrence_count;

#[derive(Debug, thiserror::Error)]
pub enum SynthesisError {
    #[error("provider error: {0}")]
    Provider(#[from] ProviderError),
    #[error("could not parse a valid response after {0} attempt(s): {1}")]
    ExhaustedRetries(u32, String),
}

/// Layer 2 of ADR-0010's recommendation engine: takes a `DetectedPattern` (Layer
/// 1's deterministic output) and an `LlmProvider`, and produces a fully-populated
/// `Recommendation` — every field PROMPT.md's Recommendation Engine section and
/// FR-10 require, with the trust-critical numeric fields sourced from
/// `DetectedPattern` alone.
pub struct Synthesizer<'a, P: LlmProvider + ?Sized> {
    provider: &'a P,
    max_attempts: u32,
}

impl<'a, P: LlmProvider + ?Sized> Synthesizer<'a, P> {
    pub fn new(provider: &'a P) -> Self {
        Self {
            provider,
            max_attempts: 2,
        }
    }

    pub fn with_max_attempts(provider: &'a P, max_attempts: u32) -> Self {
        Self {
            provider,
            max_attempts: max_attempts.max(1),
        }
    }

    pub async fn synthesize(
        &self,
        pattern_id: i64,
        detected: &DetectedPattern,
    ) -> Result<Recommendation, SynthesisError> {
        let prompt = build_prompt(&pattern_id.to_string(), detected);
        let mut last_error = String::new();

        for _attempt in 0..self.max_attempts {
            let response = self
                .provider
                .complete(CompletionRequest {
                    system: Some(SYSTEM_PROMPT.to_string()),
                    prompt: prompt.clone(),
                    max_tokens: Some(1024),
                    // The synthesis prompt asks for a single JSON object and
                    // nothing else — a reasoning trace would only slow this
                    // down (measured: minutes vs. seconds against a real local
                    // hybrid-thinking model) without improving the output the
                    // parser in `try_build` actually uses.
                    think: Some(false),
                })
                .await?;

            match self.try_build(pattern_id, detected, &response.text) {
                Ok(recommendation) => return Ok(recommendation),
                Err(reason) => last_error = reason,
            }
        }

        Err(SynthesisError::ExhaustedRetries(
            self.max_attempts,
            last_error,
        ))
    }

    fn try_build(
        &self,
        pattern_id: i64,
        detected: &DetectedPattern,
        raw_response: &str,
    ) -> Result<Recommendation, String> {
        let json = extract_json_object(raw_response)
            .ok_or_else(|| "no JSON object found in response".to_string())?;
        let fields: SynthesizedFields = serde_json::from_str(json)
            .map_err(|e| format!("JSON did not match expected shape: {e}"))?;

        if contradicts_occurrence_count(&fields.why, detected.occurrence_count) {
            return Err(format!(
                "response narrative contradicts the real occurrence count ({})",
                detected.occurrence_count
            ));
        }

        let category = parse_category(&fields.category)?;
        let difficulty = parse_level(&fields.difficulty)?;
        let maintenance_burden = parse_level(&fields.maintenance_burden)?;

        if !(0.0..=1.0).contains(&fields.confidence) {
            return Err(format!(
                "confidence {} out of range [0,1]",
                fields.confidence
            ));
        }

        let estimated_time_saved_minutes =
            detected.estimated_minutes_per_occurrence * detected.occurrence_count as f64;

        Ok(Recommendation {
            id: None,
            pattern_id,
            created_at: OffsetDateTime::now_utc(),
            title: fields.title,
            category,
            why: fields.why,
            confidence: fields.confidence,
            // Sourced from Layer 1 (`detected`), never from `fields` — see
            // `prompt.rs`'s doc comment on `SynthesizedFields`.
            estimated_time_saved_minutes,
            difficulty,
            maintenance_burden,
            privacy_implications: fields.privacy_implications,
            implementation_effort: fields.implementation_effort,
            alternatives: fields
                .alternatives
                .into_iter()
                .map(|a| Alternative {
                    approach: a.approach,
                    tradeoff: a.tradeoff,
                })
                .collect(),
            assumptions: fields.assumptions,
            ignored_information: fields.ignored_information,
            generating_provider: self.provider.id().to_string(),
            status: RecommendationStatus::Suggested,
            dismissal_reason: None,
        })
    }
}

fn parse_category(value: &str) -> Result<RecommendationCategory, String> {
    match value {
        "shortcut" => Ok(RecommendationCategory::Shortcut),
        "template" => Ok(RecommendationCategory::Template),
        "script" => Ok(RecommendationCategory::Script),
        "browser_automation" => Ok(RecommendationCategory::BrowserAutomation),
        "rpa" => Ok(RecommendationCategory::Rpa),
        "workflow_platform" => Ok(RecommendationCategory::WorkflowPlatform),
        "ai_agent" => Ok(RecommendationCategory::AiAgent),
        "hybrid" => Ok(RecommendationCategory::Hybrid),
        other => Err(format!("unknown category '{other}'")),
    }
}

fn parse_level(value: &str) -> Result<Level, String> {
    match value {
        "low" => Ok(Level::Low),
        "medium" => Ok(Level::Medium),
        "high" => Ok(Level::High),
        other => Err(format!("unknown level '{other}'")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use hiddensteps_llm_provider::CompletionResponse;
    use std::sync::Mutex;

    struct ScriptedProvider {
        responses: Mutex<Vec<String>>,
    }

    impl ScriptedProvider {
        fn new(responses: Vec<&str>) -> Self {
            Self {
                responses: Mutex::new(responses.into_iter().map(String::from).rev().collect()),
            }
        }
    }

    #[async_trait]
    impl LlmProvider for ScriptedProvider {
        fn id(&self) -> &str {
            "scripted-test-provider"
        }
        fn is_local(&self) -> bool {
            true
        }
        async fn complete(
            &self,
            _request: CompletionRequest,
        ) -> Result<CompletionResponse, ProviderError> {
            let mut responses = self.responses.lock().unwrap();
            let text = responses.pop().unwrap_or_default();
            Ok(CompletionResponse { text })
        }
        async fn embed(&self, _text: &str) -> Result<Vec<f32>, ProviderError> {
            unimplemented!("not exercised by these tests")
        }
    }

    fn sample_detected_pattern() -> DetectedPattern {
        DetectedPattern {
            signature: vec![
                "jira:app_action_event".to_string(),
                "excel:app_action_event".to_string(),
            ],
            occurrence_count: 31,
            first_seen_at: OffsetDateTime::now_utc() - time::Duration::days(14),
            last_seen_at: OffsetDateTime::now_utc(),
            estimated_minutes_per_occurrence: 21.3,
            contributing_event_ids: (1..=62).collect(),
        }
    }

    const VALID_RESPONSE: &str = r#"{
        "title": "Automate the weekly ticket export",
        "category": "hybrid",
        "why": "This exact sequence recurs with high regularity, 31 times in the last two weeks.",
        "confidence": 0.85,
        "difficulty": "medium",
        "maintenance_burden": "low",
        "privacy_implications": "Fully local, no cloud dispatch required.",
        "implementation_effort": "About 2-3 hours one-time setup.",
        "alternatives": [{"approach": "Python script", "tradeoff": "Lower setup, higher maintenance."}],
        "assumptions": ["API access to the source system is available."],
        "ignored_information": ["Occurrences on a second device were not correlated."]
    }"#;

    #[tokio::test]
    async fn synthesizes_a_recommendation_with_layer_one_numbers_intact() {
        let provider = ScriptedProvider::new(vec![VALID_RESPONSE]);
        let synthesizer = Synthesizer::new(&provider);
        let detected = sample_detected_pattern();

        let recommendation = synthesizer.synthesize(7, &detected).await.unwrap();

        assert_eq!(recommendation.pattern_id, 7);
        assert_eq!(recommendation.category, RecommendationCategory::Hybrid);
        assert_eq!(recommendation.difficulty, Level::Medium);
        // The numeric field always traces to Layer 1, computed here, never parsed
        // from the LLM's JSON (which has no such field at all).
        assert_eq!(recommendation.estimated_time_saved_minutes, 21.3 * 31.0);
        assert_eq!(recommendation.alternatives.len(), 1);
        assert_eq!(recommendation.assumptions.len(), 1);
        assert_eq!(recommendation.generating_provider, "scripted-test-provider");
    }

    #[tokio::test]
    async fn retries_once_on_malformed_json_then_succeeds() {
        let provider = ScriptedProvider::new(vec!["not json at all", VALID_RESPONSE]);
        let synthesizer = Synthesizer::new(&provider);
        let recommendation = synthesizer
            .synthesize(7, &sample_detected_pattern())
            .await
            .unwrap();
        assert_eq!(recommendation.title, "Automate the weekly ticket export");
    }

    #[tokio::test]
    async fn retries_on_a_narrative_contradiction_of_the_occurrence_count() {
        let contradicting = VALID_RESPONSE.replace(
            "This exact sequence recurs with high regularity, 31 times in the last two weeks.",
            "You've only done this 3 times, so it's low priority.",
        );
        let provider = ScriptedProvider::new(vec![&contradicting, VALID_RESPONSE]);
        let synthesizer = Synthesizer::new(&provider);
        let recommendation = synthesizer
            .synthesize(7, &sample_detected_pattern())
            .await
            .unwrap();
        assert!(!contradicts_occurrence_count(&recommendation.why, 31));
    }

    #[tokio::test]
    async fn exhausts_retries_and_returns_an_error_if_every_attempt_is_bad() {
        let provider = ScriptedProvider::new(vec!["garbage", "still garbage"]);
        let synthesizer = Synthesizer::with_max_attempts(&provider, 2);
        let result = synthesizer.synthesize(7, &sample_detected_pattern()).await;
        assert!(matches!(
            result,
            Err(SynthesisError::ExhaustedRetries(2, _))
        ));
    }

    #[tokio::test]
    async fn rejects_an_out_of_range_confidence_value() {
        let bad_confidence = VALID_RESPONSE.replace("\"confidence\": 0.85", "\"confidence\": 1.5");
        let provider = ScriptedProvider::new(vec![&bad_confidence, &bad_confidence]);
        let synthesizer = Synthesizer::with_max_attempts(&provider, 2);
        let result = synthesizer.synthesize(7, &sample_detected_pattern()).await;
        assert!(matches!(
            result,
            Err(SynthesisError::ExhaustedRetries(2, _))
        ));
    }
}
