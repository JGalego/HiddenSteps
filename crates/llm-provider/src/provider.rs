use std::time::Duration;

use async_trait::async_trait;

/// Default per-request timeout for every real `LlmProvider`'s HTTP client.
/// `reqwest::Client::new()` has no timeout at all by default — a hung remote
/// (a stuck local model, a cloud outage) would block a completion/embedding
/// call forever. 120 seconds is generous enough for a slow local model (a
/// real `qwen3` hybrid-reasoning model took over two minutes with `think`
/// left at its default before that was fixed — see `CompletionRequest::think`
/// below) while still guaranteeing every call eventually returns.
pub(crate) const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(120);

/// Builds the `reqwest::Client` every real provider (`OllamaProvider`,
/// `OpenAiCompatibleProvider`, `AnthropicProvider`) uses, so the timeout above
/// is applied in exactly one place rather than duplicated at each call site.
pub(crate) fn build_http_client() -> reqwest::Client {
    build_http_client_with_timeout(DEFAULT_REQUEST_TIMEOUT)
}

/// The general form `build_http_client` calls with the production default —
/// exists separately so a test can use a millisecond-scale timeout and prove
/// the client actually enforces one, rather than waiting out
/// `DEFAULT_REQUEST_TIMEOUT` (2 minutes) to find out.
pub(crate) fn build_http_client_with_timeout(timeout: Duration) -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(timeout)
        .build()
        .expect("building a reqwest client with only a timeout configured should never fail")
}

#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("request failed: {0}")]
    Request(String),
    #[error("provider returned an error: {0}")]
    ProviderResponse(String),
    #[error("unexpected response shape: {0}")]
    UnexpectedResponse(String),
    #[error("this provider does not support embeddings")]
    EmbeddingsUnsupported,
}

#[derive(Debug, Clone)]
pub struct CompletionRequest {
    pub system: Option<String>,
    pub prompt: String,
    pub max_tokens: Option<u32>,
    /// For hybrid reasoning/thinking models (e.g. Ollama's `qwen3` family):
    /// `Some(false)` disables the model's extended chain-of-thought before
    /// answering. Measured against a real local Ollama instance during
    /// development: the same prompt took over two minutes with thinking left
    /// at its default (`None`) and under ten seconds with `Some(false)` — for
    /// the Recommendation Engine's structured-JSON synthesis (which wants a
    /// fast, clean answer, not a reasoning trace), this matters in practice,
    /// not just in theory. Providers that have no such concept (OpenAI,
    /// Anthropic) simply ignore this field.
    pub think: Option<bool>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompletionResponse {
    pub text: String,
}

/// Per ADR-0004: every provider — local or cloud — implements this one trait, so
/// the Recommendation Engine and Embedding Layer depend on no vendor SDK directly.
/// `is_local()` is what the Privacy Engine's cloud-dispatch gate
/// (`docs/design/03-data-flow-diagrams.md` §5) checks before allowing a call to
/// proceed with anything above the cloud-eligible privacy tier.
#[async_trait]
pub trait LlmProvider: Send + Sync {
    fn id(&self) -> &str;

    /// `true` only for providers that never send data off the device (Ollama, LM
    /// Studio, LocalAI, ...). Every cloud provider in this crate returns `false` —
    /// there is no configuration flag that flips a cloud provider to `true`,
    /// because that would defeat the entire point of the gate that reads this.
    fn is_local(&self) -> bool;

    async fn complete(
        &self,
        request: CompletionRequest,
    ) -> Result<CompletionResponse, ProviderError>;

    async fn embed(&self, text: &str) -> Result<Vec<f32>, ProviderError>;
}

/// Lets a `Box<dyn LlmProvider>` (the shape `detect`/config-driven provider
/// construction naturally produces, since the concrete provider type is only
/// known at runtime) satisfy a plain `P: LlmProvider` bound directly — needed
/// by generic wrappers like `hiddensteps_privacy_engine::PrivacyGatedProvider<P>`
/// that are meant to wrap owned providers, not just `&dyn LlmProvider`
/// references.
#[async_trait]
impl LlmProvider for Box<dyn LlmProvider> {
    fn id(&self) -> &str {
        (**self).id()
    }

    fn is_local(&self) -> bool {
        (**self).is_local()
    }

    async fn complete(
        &self,
        request: CompletionRequest,
    ) -> Result<CompletionResponse, ProviderError> {
        (**self).complete(request).await
    }

    async fn embed(&self, text: &str) -> Result<Vec<f32>, ProviderError> {
        (**self).embed(text).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct StubProvider;

    #[async_trait]
    impl LlmProvider for StubProvider {
        fn id(&self) -> &str {
            "stub"
        }
        fn is_local(&self) -> bool {
            true
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
            Ok(vec![0.1])
        }
    }

    /// A boxed trait object needs to satisfy a plain `P: LlmProvider` bound
    /// directly (not just `&dyn LlmProvider`) for generic wrappers like
    /// `hiddensteps_privacy_engine::PrivacyGatedProvider<P>` to wrap an owned,
    /// runtime-selected provider — this is the blanket impl that makes that
    /// possible, exercised here through the trait's full method set.
    #[tokio::test]
    async fn boxed_trait_object_satisfies_the_llm_provider_bound_directly() {
        let boxed: Box<dyn LlmProvider> = Box::new(StubProvider);
        assert_eq!(boxed.id(), "stub");
        assert!(boxed.is_local());
        let response = boxed
            .complete(CompletionRequest {
                system: None,
                prompt: "hi".to_string(),
                max_tokens: None,
                think: None,
            })
            .await
            .unwrap();
        assert_eq!(response.text, "ok");
        assert_eq!(boxed.embed("hi").await.unwrap(), vec![0.1]);
    }

    #[tokio::test]
    async fn build_http_client_with_timeout_actually_times_out_on_a_slow_response() {
        // Proves the timeout is really wired in, not just configured and
        // silently ignored — every real provider's client would otherwise
        // wait forever on a hung remote.
        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .respond_with(
                wiremock::ResponseTemplate::new(200)
                    .set_delay(std::time::Duration::from_millis(300)),
            )
            .mount(&server)
            .await;

        let client = build_http_client_with_timeout(Duration::from_millis(50));
        let result = client.get(server.uri()).send().await;

        assert!(result.is_err());
        assert!(result.unwrap_err().is_timeout());
    }
}
