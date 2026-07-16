# ADR-0010: Recommendation engine is deterministic-first, LLM-for-synthesis-and-explanation

Status: Accepted

## Context

[../../research/03-risk-analysis.md](../../research/03-risk-analysis.md) flags "local LLM quality insufficient for useful recommendations" as a high-likelihood technical risk. PROMPT.md requires every recommendation to carry confidence, assumptions, alternatives, and estimated time saved — properties that are easier to guarantee from structured detection logic than to reliably extract from free-form LLM generation alone.

## Decision

Split recommendation generation into two layers that always run in this order:

1. **Deterministic Pattern Detection** (rule-based + statistical): sequence-mining over the Workflow Graph and `sqlite-vec` similarity search (ADR-0007) produce a structured `DetectedPattern` — frequency, time span, estimated time cost, the actual observed action sequence — with no LLM involved. This layer must work with **no LLM configured at all** (satisfies graceful degradation) and is the sole source of the *quantitative* claims ("observed 31 times, ~11 hours/month") that appear in a recommendation.
2. **LLM Synthesis** (whichever provider is configured, local-first per ADR-0004): takes a `DetectedPattern` as structured input and produces the *qualitative* judgment — candidate solution categories (shortcut, script, RPA, workflow platform, AI agent, hybrid), a recommended approach with stated reasoning, and the explanation fields (why, what was ignored, what was assumed). The LLM is never the source of the frequency/timing numbers — those are always passed in from layer 1 and must appear verbatim in the output, checked by a post-generation validator that rejects/regenerates if the LLM alters them.

## Consequences

- A user with no local model installed and no cloud provider configured still gets real, numerically grounded pattern observations ("you did this 31 times") even before choosing an AI provider — directly serving the Zero-to-Value requirement without depending on AI quality.
- Recommendation quality scales with the configured LLM's reasoning ability, but correctness of the underlying facts (the part most damaging to trust if wrong) does not — bounding the blast radius of a weak local model to "less insightful phrasing," not "wrong numbers."
- The explainability requirement (why/confidence/assumptions/what was ignored) becomes a structured output contract the LLM is prompted against and validated on, not a hope that the model volunteers this information.
- Validation/regeneration on numeric mismatch adds latency and occasional retry cost; acceptable given recommendations are generated on a batched/idle-time cadence (ADR-0006), not synchronously blocking user interaction.

## Alternatives considered

- **Pure LLM-driven recommendation generation** (feed raw pattern history to the model, let it produce everything): rejected — makes the most trust-critical numbers (time saved, frequency) hostage to model hallucination, and produces inconsistent explainability structure across different configured providers/models.
- **Pure rule-based, no LLM**: rejected as the ceiling — misses the qualitative judgment (which of six possible automation approaches actually fits this specific pattern, expressed in the "why" a user needs) that only a language model reasonably provides; would make HiddenSteps a task-mining report generator, not the "Automation Architect" PROMPT.md envisions.
