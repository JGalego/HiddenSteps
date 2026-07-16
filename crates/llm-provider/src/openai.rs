use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::provider::{CompletionRequest, CompletionResponse, LlmProvider, ProviderError};

/// OpenAI-compatible chat + embeddings API (`/v1/chat/completions`,
/// `/v1/embeddings`). `base_url` is injectable both for testing (against
/// `wiremock`) and because several other providers in scope for HiddenSteps
/// (OpenRouter, Together, Groq, DeepSeek, Azure OpenAI, LocalAI) speak the same
/// wire format — this one implementation covers all of them by construction, not
/// by copy-paste, given the right base URL and model name.
pub struct OpenAiCompatibleProvider {
    id: &'static str,
    base_url: String,
    api_key: String,
    model: String,
    embedding_model: Option<String>,
    client: reqwest::Client,
}

impl OpenAiCompatibleProvider {
    pub fn new(
        id: &'static str,
        base_url: impl Into<String>,
        api_key: impl Into<String>,
        model: impl Into<String>,
        embedding_model: Option<String>,
    ) -> Self {
        Self {
            id,
            base_url: base_url.into(),
            api_key: api_key.into(),
            model: model.into(),
            embedding_model,
            client: reqwest::Client::new(),
        }
    }

    pub fn openai(api_key: impl Into<String>, model: impl Into<String>) -> Self {
        Self::new(
            "openai",
            "https://api.openai.com",
            api_key,
            model,
            Some("text-embedding-3-small".to_string()),
        )
    }
}

#[derive(Serialize)]
struct ChatMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: Vec<ChatMessage<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: ChatChoiceMessage,
}

#[derive(Deserialize)]
struct ChatChoiceMessage {
    content: String,
}

#[derive(Serialize)]
struct EmbeddingsRequest<'a> {
    model: &'a str,
    input: &'a str,
}

#[derive(Deserialize)]
struct EmbeddingsResponse {
    data: Vec<EmbeddingsDatum>,
}

#[derive(Deserialize)]
struct EmbeddingsDatum {
    embedding: Vec<f32>,
}

#[async_trait]
impl LlmProvider for OpenAiCompatibleProvider {
    fn id(&self) -> &str {
        self.id
    }

    fn is_local(&self) -> bool {
        false
    }

    async fn complete(
        &self,
        request: CompletionRequest,
    ) -> Result<CompletionResponse, ProviderError> {
        let mut messages = Vec::new();
        if let Some(system) = &request.system {
            messages.push(ChatMessage {
                role: "system",
                content: system,
            });
        }
        messages.push(ChatMessage {
            role: "user",
            content: &request.prompt,
        });

        let body = ChatRequest {
            model: &self.model,
            messages,
            max_tokens: request.max_tokens,
        };

        let response = self
            .client
            .post(format!("{}/v1/chat/completions", self.base_url))
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::Request(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(ProviderError::ProviderResponse(format!("{status}: {text}")));
        }

        let mut parsed: ChatResponse = response
            .json()
            .await
            .map_err(|e| ProviderError::UnexpectedResponse(e.to_string()))?;
        let choice = parsed.choices.pop().ok_or_else(|| {
            ProviderError::UnexpectedResponse("no choices in response".to_string())
        })?;
        Ok(CompletionResponse {
            text: choice.message.content,
        })
    }

    async fn embed(&self, text: &str) -> Result<Vec<f32>, ProviderError> {
        let Some(embedding_model) = &self.embedding_model else {
            return Err(ProviderError::EmbeddingsUnsupported);
        };
        let body = EmbeddingsRequest {
            model: embedding_model,
            input: text,
        };
        let response = self
            .client
            .post(format!("{}/v1/embeddings", self.base_url))
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::Request(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(ProviderError::ProviderResponse(format!("{status}: {text}")));
        }

        let mut parsed: EmbeddingsResponse = response
            .json()
            .await
            .map_err(|e| ProviderError::UnexpectedResponse(e.to_string()))?;
        let datum = parsed.data.pop().ok_or_else(|| {
            ProviderError::UnexpectedResponse("no embedding data in response".to_string())
        })?;
        Ok(datum.embedding)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn complete_sends_bearer_auth_and_parses_message_content() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .and(header("authorization", "Bearer test-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "choices": [{"message": {"role": "assistant", "content": "Consider a Playwright script."}}]
            })))
            .mount(&server)
            .await;

        let provider =
            OpenAiCompatibleProvider::new("openai", server.uri(), "test-key", "gpt-4o-mini", None);
        let result = provider
            .complete(CompletionRequest {
                system: Some("You are a workflow analyst.".to_string()),
                prompt: "what should I automate?".to_string(),
                max_tokens: Some(200),
                think: None,
            })
            .await
            .unwrap();

        assert_eq!(result.text, "Consider a Playwright script.");
    }

    #[tokio::test]
    async fn embed_without_a_configured_embedding_model_is_unsupported() {
        let provider = OpenAiCompatibleProvider::new(
            "anthropic-shaped",
            "http://unused",
            "key",
            "model",
            None,
        );
        let result = provider.embed("text").await;
        assert!(matches!(result, Err(ProviderError::EmbeddingsUnsupported)));
    }

    #[tokio::test]
    async fn embed_parses_the_embedding_vector() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/embeddings"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": [{"embedding": [0.4, 0.5, 0.6]}]
            })))
            .mount(&server)
            .await;

        let provider = OpenAiCompatibleProvider::new(
            "openai",
            server.uri(),
            "test-key",
            "gpt-4o-mini",
            Some("text-embedding-3-small".to_string()),
        );
        let embedding = provider.embed("copy ticket data").await.unwrap();
        assert_eq!(embedding, vec![0.4, 0.5, 0.6]);
    }

    #[test]
    fn cloud_provider_is_never_local() {
        let provider = OpenAiCompatibleProvider::openai("key", "gpt-4o-mini");
        assert!(!provider.is_local());
    }
}
