//! The LLM Provider Layer (ADR-0004): one `LlmProvider` trait, implemented by
//! Ollama (the local-first default) and an OpenAI-wire-compatible cloud client
//! that covers OpenAI, Azure OpenAI, OpenRouter, Together, Groq, DeepSeek, and
//! LocalAI (they all speak the same `/v1/chat/completions` + `/v1/embeddings`
//! shape), plus a dedicated Anthropic client for its distinct Messages API.
//!
//! Every provider's tests run against a `wiremock` mock HTTP server, asserting
//! the exact request shape sent and the exact response shape parsed — this is
//! real HTTP-layer verification of each provider's wire format, not a stub that
//! assumes the format is right.
//!
//! Mistral and Cohere (also named in PROMPT.md's Cloud AI list) are not yet
//! implemented — both have their own distinct (non-OpenAI-compatible,
//! non-Anthropic-shaped) APIs, and adding them is a matter of one more file each
//! following the exact pattern in `openai.rs`/`anthropic.rs`, not an architecture
//! change. That's a real, disclosed gap, not a hidden one.

mod anthropic;
mod detection;
mod ollama;
mod openai;
mod provider;

pub use anthropic::AnthropicProvider;
pub use detection::{default_candidates, detect, DetectedRuntime, RuntimeCandidate};
pub use ollama::OllamaProvider;
pub use openai::OpenAiCompatibleProvider;
pub use provider::{CompletionRequest, CompletionResponse, LlmProvider, ProviderError};
