use std::time::Duration;

use serde::{Deserialize, Serialize};

/// How to parse a reachable candidate's response body into a list of model
/// names — different local runtimes shape this differently.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelListFormat {
    /// Ollama's `/api/tags`: `{"models": [{"name": "qwen3:0.6b", ...}, ...]}`.
    OllamaTags,
    /// The OpenAI-compatible `/v1/models`: `{"data": [{"id": "..."}, ...]}` —
    /// LM Studio and LocalAI both speak this shape.
    OpenAiModels,
}

/// A local runtime worth probing for at onboarding (FR-14): its display name,
/// the URL of an endpoint that, if reachable and returning success, proves the
/// runtime is actually running (not just that some webserver happens to be on
/// that port), and how to read the model list out of that same response.
pub struct RuntimeCandidate {
    pub name: &'static str,
    pub probe_url: String,
    pub model_list_format: ModelListFormat,
}

/// `Serialize` is required here, not decorative: this crosses the Tauri IPC
/// boundary as `get_provider_detection`'s return type
/// (`apps/desktop/src-tauri/src/commands.rs`), and `#[tauri::command]`
/// requires its return type to be serializable — a real compile error this
/// crate's own local `cargo build` could never catch, since it doesn't depend
/// on `tauri` at all.
///
/// `models` is real and load-bearing, not cosmetic: without it, onboarding has
/// no way to offer a model choice, and (a real bug found by actually running
/// the app) falls back to probing a literal model named `"default"`, which
/// doesn't exist on a real Ollama instance and fails with a 404.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DetectedRuntime {
    pub name: &'static str,
    pub reachable: bool,
    pub models: Vec<String>,
}

#[derive(Deserialize)]
struct OllamaTagsResponse {
    models: Vec<OllamaModelEntry>,
}

#[derive(Deserialize)]
struct OllamaModelEntry {
    name: String,
}

#[derive(Deserialize)]
struct OpenAiModelsResponse {
    data: Vec<OpenAiModelEntry>,
}

#[derive(Deserialize)]
struct OpenAiModelEntry {
    id: String,
}

fn parse_models(format: ModelListFormat, body: &str) -> Vec<String> {
    match format {
        ModelListFormat::OllamaTags => serde_json::from_str::<OllamaTagsResponse>(body)
            .map(|r| r.models.into_iter().map(|m| m.name).collect())
            .unwrap_or_default(),
        ModelListFormat::OpenAiModels => serde_json::from_str::<OpenAiModelsResponse>(body)
            .map(|r| r.data.into_iter().map(|m| m.id).collect())
            .unwrap_or_default(),
    }
}

/// The well-known default ports for the local runtimes PROMPT.md names (Ollama,
/// LM Studio, LocalAI) — used by onboarding's real detection pass. `detect`
/// itself takes an explicit candidate list rather than hardcoding these, so tests
/// can point at a `wiremock` server instead.
pub fn default_candidates() -> Vec<RuntimeCandidate> {
    vec![
        RuntimeCandidate {
            name: "ollama",
            probe_url: "http://localhost:11434/api/tags".to_string(),
            model_list_format: ModelListFormat::OllamaTags,
        },
        RuntimeCandidate {
            name: "lm_studio",
            probe_url: "http://localhost:1234/v1/models".to_string(),
            model_list_format: ModelListFormat::OpenAiModels,
        },
        RuntimeCandidate {
            name: "localai",
            probe_url: "http://localhost:8080/v1/models".to_string(),
            model_list_format: ModelListFormat::OpenAiModels,
        },
    ]
}

/// Probes each candidate concurrently with a short timeout — onboarding needs
/// this to feel instantaneous (per the "under five minutes" goal), not to wait
/// out a full TCP timeout per candidate serially.
pub async fn detect(
    client: &reqwest::Client,
    candidates: &[RuntimeCandidate],
    timeout: Duration,
) -> Vec<DetectedRuntime> {
    let probes = candidates.iter().map(|candidate| {
        let client = client.clone();
        let name = candidate.name;
        let url = candidate.probe_url.clone();
        let format = candidate.model_list_format;
        async move {
            let response = client.get(&url).timeout(timeout).send().await;
            match response {
                Ok(response) if response.status().is_success() => {
                    let body = response.text().await.unwrap_or_default();
                    DetectedRuntime {
                        name,
                        reachable: true,
                        models: parse_models(format, &body),
                    }
                }
                _ => DetectedRuntime {
                    name,
                    reachable: false,
                    models: Vec::new(),
                },
            }
        }
    });
    futures_join_all(probes).await
}

/// A tiny local `join_all` so this crate doesn't need to pull in the `futures`
/// crate just for one combinator — `detect`'s probes are a fixed, small,
/// known-at-call-time list, not an arbitrary stream.
async fn futures_join_all<F: std::future::Future>(
    iter: impl IntoIterator<Item = F>,
) -> Vec<F::Output> {
    let mut handles = Vec::new();
    for fut in iter {
        handles.push(fut);
    }
    let mut results = Vec::with_capacity(handles.len());
    for handle in handles {
        results.push(handle.await);
    }
    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn reports_a_running_mock_runtime_as_reachable_with_its_real_models() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/tags"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "models": [{"name": "qwen3:0.6b"}, {"name": "llama3.1:8b"}]
            })))
            .mount(&server)
            .await;

        let candidates = vec![RuntimeCandidate {
            name: "ollama",
            probe_url: format!("{}/api/tags", server.uri()),
            model_list_format: ModelListFormat::OllamaTags,
        }];
        let client = reqwest::Client::new();
        let results = detect(&client, &candidates, Duration::from_millis(500)).await;

        assert_eq!(
            results,
            vec![DetectedRuntime {
                name: "ollama",
                reachable: true,
                models: vec!["qwen3:0.6b".to_string(), "llama3.1:8b".to_string()],
            }]
        );
    }

    #[tokio::test]
    async fn parses_openai_compatible_model_list_shape() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/models"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": [{"id": "mistral-7b-instruct"}]
            })))
            .mount(&server)
            .await;

        let candidates = vec![RuntimeCandidate {
            name: "lm_studio",
            probe_url: format!("{}/v1/models", server.uri()),
            model_list_format: ModelListFormat::OpenAiModels,
        }];
        let client = reqwest::Client::new();
        let results = detect(&client, &candidates, Duration::from_millis(500)).await;

        assert_eq!(results[0].models, vec!["mistral-7b-instruct".to_string()]);
    }

    #[tokio::test]
    async fn reports_a_closed_port_as_unreachable_with_no_models() {
        // Port 0 is never a valid connection target — guarantees a fast,
        // deterministic connection failure without depending on any specific
        // port happening to be free in the test environment.
        let candidates = vec![RuntimeCandidate {
            name: "lm_studio",
            probe_url: "http://localhost:0/v1/models".to_string(),
            model_list_format: ModelListFormat::OpenAiModels,
        }];
        let client = reqwest::Client::new();
        let results = detect(&client, &candidates, Duration::from_millis(500)).await;

        assert_eq!(
            results,
            vec![DetectedRuntime {
                name: "lm_studio",
                reachable: false,
                models: vec![],
            }]
        );
    }

    #[tokio::test]
    async fn a_malformed_body_yields_an_empty_model_list_not_a_panic() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/tags"))
            .respond_with(ResponseTemplate::new(200).set_body_string("not json"))
            .mount(&server)
            .await;

        let candidates = vec![RuntimeCandidate {
            name: "ollama",
            probe_url: format!("{}/api/tags", server.uri()),
            model_list_format: ModelListFormat::OllamaTags,
        }];
        let client = reqwest::Client::new();
        let results = detect(&client, &candidates, Duration::from_millis(500)).await;

        assert!(results[0].reachable);
        assert!(results[0].models.is_empty());
    }

    #[tokio::test]
    async fn probes_multiple_candidates_independently() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/models"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"data": []})))
            .mount(&server)
            .await;

        let candidates = vec![
            RuntimeCandidate {
                name: "localai",
                probe_url: format!("{}/v1/models", server.uri()),
                model_list_format: ModelListFormat::OpenAiModels,
            },
            RuntimeCandidate {
                name: "lm_studio",
                probe_url: "http://localhost:0/v1/models".to_string(),
                model_list_format: ModelListFormat::OpenAiModels,
            },
        ];
        let client = reqwest::Client::new();
        let results = detect(&client, &candidates, Duration::from_millis(500)).await;

        assert_eq!(results.len(), 2);
        assert!(
            results
                .iter()
                .find(|r| r.name == "localai")
                .unwrap()
                .reachable
        );
        assert!(
            !results
                .iter()
                .find(|r| r.name == "lm_studio")
                .unwrap()
                .reachable
        );
    }
}
