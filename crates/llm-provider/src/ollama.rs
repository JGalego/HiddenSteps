use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::provider::{CompletionRequest, CompletionResponse, LlmProvider, ProviderError};

/// The default local runtime, per ADR-0004. Talks to Ollama's HTTP API
/// (`/api/generate`, `/api/embeddings`) — the same API surface Ollama has exposed
/// since its earliest releases.
pub struct OllamaProvider {
    base_url: String,
    model: String,
    client: reqwest::Client,
}

impl OllamaProvider {
    /// `base_url` is injectable (rather than hardcoded to
    /// `http://localhost:11434`) specifically so tests can point this at a
    /// `wiremock` mock server instead of a real running Ollama instance.
    pub fn new(base_url: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            model: model.into(),
            client: reqwest::Client::new(),
        }
    }

    pub fn localhost(model: impl Into<String>) -> Self {
        Self::new("http://localhost:11434", model)
    }
}

#[derive(Serialize)]
struct GenerateRequest<'a> {
    model: &'a str,
    prompt: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<&'a str>,
    stream: bool,
}

#[derive(Deserialize)]
struct GenerateResponse {
    response: String,
}

#[derive(Serialize)]
struct EmbeddingsRequest<'a> {
    model: &'a str,
    prompt: &'a str,
}

#[derive(Deserialize)]
struct EmbeddingsResponse {
    embedding: Vec<f32>,
}

#[async_trait]
impl LlmProvider for OllamaProvider {
    fn id(&self) -> &str {
        "ollama"
    }

    fn is_local(&self) -> bool {
        true
    }

    async fn complete(
        &self,
        request: CompletionRequest,
    ) -> Result<CompletionResponse, ProviderError> {
        let body = GenerateRequest {
            model: &self.model,
            prompt: &request.prompt,
            system: request.system.as_deref(),
            stream: false,
        };
        let response = self
            .client
            .post(format!("{}/api/generate", self.base_url))
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::Request(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(ProviderError::ProviderResponse(format!("{status}: {text}")));
        }

        let parsed: GenerateResponse = response
            .json()
            .await
            .map_err(|e| ProviderError::UnexpectedResponse(e.to_string()))?;
        Ok(CompletionResponse {
            text: parsed.response,
        })
    }

    async fn embed(&self, text: &str) -> Result<Vec<f32>, ProviderError> {
        let body = EmbeddingsRequest {
            model: &self.model,
            prompt: text,
        };
        let response = self
            .client
            .post(format!("{}/api/embeddings", self.base_url))
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::Request(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(ProviderError::ProviderResponse(format!("{status}: {text}")));
        }

        let parsed: EmbeddingsResponse = response
            .json()
            .await
            .map_err(|e| ProviderError::UnexpectedResponse(e.to_string()))?;
        Ok(parsed.embedding)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{body_json, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn complete_sends_the_documented_request_shape_and_parses_the_response() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/generate"))
            .and(body_json(serde_json::json!({
                "model": "llama3.1:8b",
                "prompt": "why do I keep copying this field?",
                "system": "You are a workflow analyst.",
                "stream": false
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "model": "llama3.1:8b",
                "response": "This looks like a repeated manual data-entry step.",
                "done": true
            })))
            .mount(&server)
            .await;

        let provider = OllamaProvider::new(server.uri(), "llama3.1:8b");
        let result = provider
            .complete(CompletionRequest {
                system: Some("You are a workflow analyst.".to_string()),
                prompt: "why do I keep copying this field?".to_string(),
                max_tokens: None,
            })
            .await
            .unwrap();

        assert_eq!(
            result.text,
            "This looks like a repeated manual data-entry step."
        );
    }

    #[tokio::test]
    async fn embed_parses_the_embedding_vector() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/embeddings"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "embedding": [0.1, 0.2, 0.3]
            })))
            .mount(&server)
            .await;

        let provider = OllamaProvider::new(server.uri(), "nomic-embed-text");
        let embedding = provider.embed("copy ticket data").await.unwrap();
        assert_eq!(embedding, vec![0.1, 0.2, 0.3]);
    }

    #[tokio::test]
    async fn non_success_status_becomes_a_provider_response_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/generate"))
            .respond_with(ResponseTemplate::new(500).set_body_string("model not found"))
            .mount(&server)
            .await;

        let provider = OllamaProvider::new(server.uri(), "missing-model");
        let result = provider
            .complete(CompletionRequest {
                system: None,
                prompt: "test".to_string(),
                max_tokens: None,
            })
            .await;

        assert!(matches!(result, Err(ProviderError::ProviderResponse(_))));
    }

    #[test]
    fn ollama_is_always_local() {
        let provider = OllamaProvider::localhost("llama3.1:8b");
        assert!(provider.is_local());
    }
}
