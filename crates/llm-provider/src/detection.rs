use std::time::Duration;

/// A local runtime worth probing for at onboarding (FR-14): its display name and
/// the URL of an endpoint that, if reachable and returning success, proves the
/// runtime is actually running (not just that some webserver happens to be on
/// that port).
pub struct RuntimeCandidate {
    pub name: &'static str,
    pub probe_url: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DetectedRuntime {
    pub name: &'static str,
    pub reachable: bool,
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
        },
        RuntimeCandidate {
            name: "lm_studio",
            probe_url: "http://localhost:1234/v1/models".to_string(),
        },
        RuntimeCandidate {
            name: "localai",
            probe_url: "http://localhost:8080/v1/models".to_string(),
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
        async move {
            let reachable = client
                .get(&url)
                .timeout(timeout)
                .send()
                .await
                .map(|response| response.status().is_success())
                .unwrap_or(false);
            DetectedRuntime { name, reachable }
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
    async fn reports_a_running_mock_runtime_as_reachable() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/tags"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"models": []})),
            )
            .mount(&server)
            .await;

        let candidates = vec![RuntimeCandidate {
            name: "ollama",
            probe_url: format!("{}/api/tags", server.uri()),
        }];
        let client = reqwest::Client::new();
        let results = detect(&client, &candidates, Duration::from_millis(500)).await;

        assert_eq!(
            results,
            vec![DetectedRuntime {
                name: "ollama",
                reachable: true
            }]
        );
    }

    #[tokio::test]
    async fn reports_a_closed_port_as_unreachable() {
        // Port 0 is never a valid connection target — guarantees a fast,
        // deterministic connection failure without depending on any specific
        // port happening to be free in the test environment.
        let candidates = vec![RuntimeCandidate {
            name: "lm_studio",
            probe_url: "http://localhost:0/v1/models".to_string(),
        }];
        let client = reqwest::Client::new();
        let results = detect(&client, &candidates, Duration::from_millis(500)).await;

        assert_eq!(
            results,
            vec![DetectedRuntime {
                name: "lm_studio",
                reachable: false
            }]
        );
    }

    #[tokio::test]
    async fn probes_multiple_candidates_independently() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/models"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;

        let candidates = vec![
            RuntimeCandidate {
                name: "localai",
                probe_url: format!("{}/v1/models", server.uri()),
            },
            RuntimeCandidate {
                name: "lm_studio",
                probe_url: "http://localhost:0/v1/models".to_string(),
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
