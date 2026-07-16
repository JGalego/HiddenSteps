use async_trait::async_trait;

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
