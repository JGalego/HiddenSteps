//! Real integration tests against a **real, running Ollama instance** — not
//! `wiremock`. `#[ignore]`d by default for the same reason
//! `hiddensteps-security`'s real-vault test and
//! `hiddensteps-observation::linux::shortcuts`'s real-XGrabKey test are: it
//! depends on something outside this repo (a running local service, here)
//! that most dev/CI environments won't have, so a silent runtime skip would be
//! misleading — an `#[ignore]`d test that's visibly not run is more honest
//! than one that "passes" without exercising anything.
//!
//! Run explicitly with a real Ollama instance up:
//! `cargo test -p hiddensteps-llm-provider -- --ignored`
//!
//! Defaults to `qwen3:0.6b` against `http://localhost:11434`, overridable via
//! `HIDDENSTEPS_TEST_OLLAMA_MODEL` / `HIDDENSTEPS_TEST_OLLAMA_URL` — this is
//! exactly the model+instance this was developed and verified against, a
//! small (0.6B) hybrid-reasoning model.

use hiddensteps_llm_provider::{CompletionRequest, LlmProvider, OllamaProvider, ProviderError};

fn test_model() -> String {
    std::env::var("HIDDENSTEPS_TEST_OLLAMA_MODEL").unwrap_or_else(|_| "qwen3:0.6b".to_string())
}

fn test_url() -> String {
    std::env::var("HIDDENSTEPS_TEST_OLLAMA_URL")
        .unwrap_or_else(|_| "http://localhost:11434".to_string())
}

#[tokio::test]
#[ignore = "requires a real, running local Ollama instance with the model pulled"]
async fn completes_a_real_prompt_against_a_real_local_model() {
    let provider = OllamaProvider::new(test_url(), test_model());

    let response = provider
        .complete(CompletionRequest {
            system: Some(
                "Reply with exactly one word and nothing else: the word \"acknowledged\"."
                    .to_string(),
            ),
            prompt: "Confirm you received this.".to_string(),
            max_tokens: Some(64),
            // Thinking disabled: see CompletionRequest::think's doc comment —
            // this is the setting that took the same real request from over
            // two minutes to a few seconds against this exact model on this
            // exact (CPU-only) machine.
            think: Some(false),
        })
        .await
        .expect("a real Ollama instance should complete a trivial prompt");

    assert!(
        !response.text.trim().is_empty(),
        "expected real, non-empty model output"
    );
}

#[tokio::test]
#[ignore = "requires a real, running local Ollama instance"]
async fn surfaces_a_real_backend_error_without_panicking_or_misparsing() {
    // At the time this was written, the local Ollama instance available in
    // the dev environment had embeddings disabled server-side (a real,
    // observed condition: `{"error":"This server does not support
    // embeddings. Start it with `--embeddings`"}` from both `/api/embeddings`
    // and `/api/embed`). Rather than special-case around that by restarting
    // someone else's running service with different flags, this test asserts
    // the client's actual, real contract: a non-2xx JSON error response
    // becomes a `ProviderError::ProviderResponse` carrying the real server
    // message, not a panic, not a silently-empty vector.
    let provider = OllamaProvider::new(test_url(), test_model());
    let result = provider.embed("copy ticket data into report").await;

    match result {
        Err(ProviderError::ProviderResponse(message)) => {
            assert!(
                !message.is_empty(),
                "expected the real backend error message to be preserved"
            );
        }
        Ok(embedding) => {
            // If this environment's Ollama instance *does* have embeddings
            // enabled, that's also a valid, real outcome — assert it's a
            // real, non-empty vector rather than assuming failure.
            assert!(
                !embedding.is_empty(),
                "expected a real, non-empty embedding vector"
            );
        }
        Err(other) => panic!("expected a provider-response error or success, got {other:?}"),
    }
}
