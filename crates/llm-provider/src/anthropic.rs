use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::provider::{CompletionRequest, CompletionResponse, LlmProvider, ProviderError};

const ANTHROPIC_VERSION: &str = "2023-06-01";
/// Anthropic's Messages API requires `max_tokens`; this is the fallback used when
/// a caller's `CompletionRequest` doesn't specify one.
const DEFAULT_MAX_TOKENS: u32 = 1024;

/// Anthropic's Messages API (`/v1/messages`). Anthropic does not publish an
/// embeddings endpoint, so `embed()` always returns
/// `ProviderError::EmbeddingsUnsupported` — the Embedding Layer must fall back to
/// a different configured provider (typically the local one) when the user's
/// chosen completion provider is Anthropic, per ADR-0004.
pub struct AnthropicProvider {
    base_url: String,
    api_key: String,
    model: String,
    client: reqwest::Client,
}

impl AnthropicProvider {
    pub fn new(
        base_url: impl Into<String>,
        api_key: impl Into<String>,
        model: impl Into<String>,
    ) -> Self {
        Self {
            base_url: base_url.into(),
            api_key: api_key.into(),
            model: model.into(),
            client: reqwest::Client::new(),
        }
    }

    pub fn cloud(api_key: impl Into<String>, model: impl Into<String>) -> Self {
        Self::new("https://api.anthropic.com", api_key, model)
    }
}

#[derive(Serialize)]
struct MessagesRequest<'a> {
    model: &'a str,
    max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<&'a str>,
    messages: Vec<Message<'a>>,
}

#[derive(Serialize)]
struct Message<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Deserialize)]
struct MessagesResponse {
    content: Vec<ContentBlock>,
}

#[derive(Deserialize)]
struct ContentBlock {
    #[serde(rename = "type")]
    block_type: String,
    text: Option<String>,
}

#[async_trait]
impl LlmProvider for AnthropicProvider {
    fn id(&self) -> &str {
        "anthropic"
    }

    fn is_local(&self) -> bool {
        false
    }

    async fn complete(
        &self,
        request: CompletionRequest,
    ) -> Result<CompletionResponse, ProviderError> {
        let body = MessagesRequest {
            model: &self.model,
            max_tokens: request.max_tokens.unwrap_or(DEFAULT_MAX_TOKENS),
            system: request.system.as_deref(),
            messages: vec![Message {
                role: "user",
                content: &request.prompt,
            }],
        };

        let response = self
            .client
            .post(format!("{}/v1/messages", self.base_url))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::Request(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(ProviderError::ProviderResponse(format!("{status}: {text}")));
        }

        let parsed: MessagesResponse = response
            .json()
            .await
            .map_err(|e| ProviderError::UnexpectedResponse(e.to_string()))?;

        let text = parsed
            .content
            .into_iter()
            .find(|block| block.block_type == "text")
            .and_then(|block| block.text)
            .ok_or_else(|| {
                ProviderError::UnexpectedResponse("no text content block in response".to_string())
            })?;

        Ok(CompletionResponse { text })
    }

    async fn embed(&self, _text: &str) -> Result<Vec<f32>, ProviderError> {
        Err(ProviderError::EmbeddingsUnsupported)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn complete_sends_anthropic_headers_and_parses_the_text_block() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .and(header("x-api-key", "test-key"))
            .and(header("anthropic-version", ANTHROPIC_VERSION))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "msg_1",
                "role": "assistant",
                "content": [{"type": "text", "text": "This pattern is a strong RPA candidate."}]
            })))
            .mount(&server)
            .await;

        let provider = AnthropicProvider::new(server.uri(), "test-key", "claude-sonnet-5");
        let result = provider
            .complete(CompletionRequest {
                system: Some("You are a workflow analyst.".to_string()),
                prompt: "what should I automate?".to_string(),
                max_tokens: Some(512),
                think: None,
            })
            .await
            .unwrap();

        assert_eq!(result.text, "This pattern is a strong RPA candidate.");
    }

    #[tokio::test]
    async fn embed_is_always_unsupported() {
        let provider = AnthropicProvider::cloud("test-key", "claude-sonnet-5");
        let result = provider.embed("text").await;
        assert!(matches!(result, Err(ProviderError::EmbeddingsUnsupported)));
    }

    #[test]
    fn anthropic_is_never_local() {
        let provider = AnthropicProvider::cloud("test-key", "claude-sonnet-5");
        assert!(!provider.is_local());
    }
}
