# ADR-0004: `LlmProvider` trait with local-first default, providers loaded as plugins

Status: Accepted

## Context

PROMPT.md mandates local AI support first (Ollama, LM Studio, llama.cpp, LocalAI, vLLM) with cloud providers (OpenAI, Anthropic, Google, Azure OpenAI, OpenRouter, Together, Groq, Mistral, Cohere, DeepSeek) as opt-in, and requires the architecture to "support additional providers through plugins." The recommendation engine (ADR-0010) needs a uniform way to call whichever provider the user has configured, without hardcoding provider-specific logic into core business logic.

## Decision

Define a single core trait, `LlmProvider`, with methods for chat/completion, embedding generation, and capability introspection (context window, supports-tool-calling, is-local). Every provider — local or cloud — is an implementation of this trait, shipped either in-tree (Ollama, OpenAI, Anthropic are first-class/in-tree given their centrality to the product's positioning) or as a WASM plugin (ADR-0009) for everything else. At startup, an **auto-detection routine** probes for locally-running Ollama/LM Studio/LocalAI/vLLM endpoints and installed llama.cpp binaries, and surfaces what it finds to onboarding; no provider is active until the user selects one.

## Consequences

- The Recommendation Engine, Pattern Detection, and any other consumer of AI capability depends only on the `LlmProvider` trait, never on a specific vendor SDK — swapping providers is a configuration change, not a code change.
- Local-first is enforced structurally, not just by default configuration: onboarding cannot proceed to "select cloud provider" without first showing local-runtime detection results and the tradeoffs (per PROMPT.md's AI Provider Setup requirements).
- Adding a new cloud provider (e.g., a future one not in the initial list) requires only a new plugin implementing `LlmProvider`, not a core-code change — satisfying the extensibility requirement without re-litigating the trust boundary each time.
- Because Deep-mode (Level 4) content must never reach a cloud provider without separate explicit consent ([../../research/05-privacy-analysis.md](../../research/05-privacy-analysis.md)), the trait's call sites are wrapped by the Privacy Engine, which inspects the privacy level of the data being sent and blocks/warns before dispatch to any provider not marked `is_local()`.

## Alternatives considered

- **Direct integration with one SDK (e.g., LangChain) as the provider abstraction layer**: rejected — pulls in a large dependency surface and framework-specific assumptions (agent loops, tool-calling conventions) the product doesn't need; a narrow, purpose-built trait is easier to audit for the privacy-gating behavior above.
- **Cloud-first with local as an alternative**: rejected outright — contradicts PROMPT.md's "Local AI (Mandatory)" requirement and the differentiation strategy in [../../research/02-market-gaps-and-differentiation.md](../../research/02-market-gaps-and-differentiation.md).
