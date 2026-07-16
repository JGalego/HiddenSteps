use hiddensteps_domain::PrivacyLevel;
use hiddensteps_llm_provider::{CompletionRequest, CompletionResponse, LlmProvider, ProviderError};

use crate::gate::{DispatchDecision, DispatchGate};

#[derive(Debug, thiserror::Error)]
pub enum GateError {
    #[error("blocked by privacy gate: cloud dispatch not permitted for this content")]
    Blocked,
    #[error("requires explicit consent for content class '{0}' before this can be sent to a cloud provider")]
    RequiresConsent(String),
    #[error("provider error: {0}")]
    Provider(#[from] ProviderError),
}

/// Wraps an `LlmProvider` so every call is checked against a `DispatchGate` first
/// — the structural embodiment of ADR-0004's "this gate wraps every LlmProvider
/// call site" requirement. `inner` is private: code holding only a
/// `PrivacyGatedProvider` has no way to reach the wrapped provider's
/// `complete`/`embed` methods directly and skip the gate.
///
/// (This is a per-component contract, not a sandboxing guarantee — code that
/// separately holds its own reference to the same underlying provider could still
/// call it ungated. The WASM plugin sandbox in a later milestone is where
/// capability access is enforced at a boundary nothing on the inside can route
/// around; this wrapper's job is narrower: make the *correct* call path the only
/// *convenient* one for application code wiring the Recommendation Engine
/// together.)
pub struct PrivacyGatedProvider<P: LlmProvider> {
    inner: P,
    gate: DispatchGate,
}

impl<P: LlmProvider> PrivacyGatedProvider<P> {
    pub fn new(inner: P, gate: DispatchGate) -> Self {
        Self { inner, gate }
    }

    pub fn gate_mut(&mut self) -> &mut DispatchGate {
        &mut self.gate
    }

    pub fn provider_id(&self) -> &str {
        self.inner.id()
    }

    pub async fn complete_if_allowed(
        &self,
        request: CompletionRequest,
        privacy_level: PrivacyLevel,
        content_class: &str,
        contains_verbatim_strings: bool,
    ) -> Result<CompletionResponse, GateError> {
        match self.gate.evaluate(
            self.inner.is_local(),
            privacy_level,
            content_class,
            contains_verbatim_strings,
        ) {
            DispatchDecision::Allow => Ok(self.inner.complete(request).await?),
            DispatchDecision::RequiresConsent(class) => Err(GateError::RequiresConsent(class)),
            DispatchDecision::Blocked => Err(GateError::Blocked),
        }
    }

    pub async fn embed_if_allowed(
        &self,
        text: &str,
        privacy_level: PrivacyLevel,
        content_class: &str,
        contains_verbatim_strings: bool,
    ) -> Result<Vec<f32>, GateError> {
        match self.gate.evaluate(
            self.inner.is_local(),
            privacy_level,
            content_class,
            contains_verbatim_strings,
        ) {
            DispatchDecision::Allow => Ok(self.inner.embed(text).await?),
            DispatchDecision::RequiresConsent(class) => Err(GateError::RequiresConsent(class)),
            DispatchDecision::Blocked => Err(GateError::Blocked),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;

    struct StubProvider {
        local: bool,
    }

    #[async_trait]
    impl LlmProvider for StubProvider {
        fn id(&self) -> &str {
            "stub"
        }
        fn is_local(&self) -> bool {
            self.local
        }
        async fn complete(
            &self,
            _request: CompletionRequest,
        ) -> Result<CompletionResponse, ProviderError> {
            Ok(CompletionResponse {
                text: "ok".to_string(),
            })
        }
        async fn embed(&self, _text: &str) -> Result<Vec<f32>, ProviderError> {
            Ok(vec![0.1, 0.2])
        }
    }

    fn request() -> CompletionRequest {
        CompletionRequest {
            system: None,
            prompt: "test".to_string(),
            max_tokens: None,
        }
    }

    #[tokio::test]
    async fn local_provider_dispatch_always_succeeds() {
        let provider = PrivacyGatedProvider::new(StubProvider { local: true }, DispatchGate::new());
        let result = provider
            .complete_if_allowed(request(), PrivacyLevel::MaximumAssistance, "ocr_text", true)
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn cloud_provider_without_consent_is_blocked_not_silently_sent() {
        let provider =
            PrivacyGatedProvider::new(StubProvider { local: false }, DispatchGate::new());
        let result = provider
            .complete_if_allowed(
                request(),
                PrivacyLevel::ApplicationMetadata,
                "pattern_summary",
                false,
            )
            .await;
        assert!(matches!(result, Err(GateError::Blocked)));
    }

    #[tokio::test]
    async fn cloud_provider_with_verbatim_content_requires_consent_before_dispatch() {
        let mut gate = DispatchGate::new();
        gate.grant_general_cloud_consent();
        let mut provider = PrivacyGatedProvider::new(StubProvider { local: false }, gate);

        let blocked = provider
            .complete_if_allowed(request(), PrivacyLevel::ContextAware, "window_title", true)
            .await;
        assert!(matches!(blocked, Err(GateError::RequiresConsent(_))));

        provider.gate_mut().grant_class_consent("window_title");
        let allowed = provider
            .complete_if_allowed(request(), PrivacyLevel::ContextAware, "window_title", true)
            .await;
        assert!(allowed.is_ok());
    }

    #[tokio::test]
    async fn deep_mode_content_is_blocked_for_cloud_providers_even_with_full_consent() {
        let mut gate = DispatchGate::new();
        gate.grant_general_cloud_consent();
        gate.grant_class_consent("ocr_text");
        let provider = PrivacyGatedProvider::new(StubProvider { local: false }, gate);

        let result = provider
            .complete_if_allowed(
                request(),
                PrivacyLevel::MaximumAssistance,
                "ocr_text",
                false,
            )
            .await;
        assert!(matches!(result, Err(GateError::Blocked)));
    }
}
