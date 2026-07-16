# HiddenSteps — Phase 2: Design

Phase 2 deliverable set, per `PROMPT.md`. Builds directly on the Phase 1 research (`../research/`) — every non-obvious decision below traces back to a specific finding, risk, or principle established there.

## Reading order

1. [01-prd.md](01-prd.md) — Product Requirements Document: goals, non-goals, personas, functional/non-functional requirements, success metrics, explicit exclusions.
2. [adr/](adr/) — Architecture Decision Records: the concrete technology choices (Tauri, Rust, SQLCipher, `sqlite-vec`, OS-vault key management, WASM plugin sandbox, hybrid deterministic+LLM recommendation engine) and why each was chosen over its alternatives.
3. [02-system-architecture.md](02-system-architecture.md) — the module map (mirrors PROMPT.md's 14 suggested modules) as Rust crates under Clean Architecture layering, with a component diagram.
4. [03-data-flow-diagrams.md](03-data-flow-diagrams.md) — the core capture→recommendation loop, onboarding consent, privacy-level changes, export/delete, and cloud-dispatch gating, as Mermaid diagrams.
5. [04-trust-model.md](04-trust-model.md) — who can see what, and how a user independently verifies HiddenSteps is doing what it claims (distinct from the privacy model — this is about verifiability, not data minimization).
6. [05-privacy-model.md](05-privacy-model.md) — the enforceable specification of privacy levels (exact signal lists), retention rules, cloud-eligibility thresholds, redaction guarantees, and consent versioning.
7. [06-security-architecture.md](06-security-architecture.md) — encryption at rest, key lifecycle, signed updates, audit log, local-only/air-gapped/enterprise modes, plugin security, and the security test suite this all implies for Phase 4.
8. [07-database-schema.md](07-database-schema.md) — the actual SQL DDL for the single encrypted SQLite file, annotated with which privacy/security guarantees each table structurally enforces.
9. [08-plugin-architecture.md](08-plugin-architecture.md) — plugin types, manifest schema, capability-grant lifecycle, and the WIT host interface.
10. [09-api-specification.md](09-api-specification.md) — the UI↔Core IPC contract (commands and events); explicitly not a public network API, since there is no server component in this phase.

## ADR index

| ADR | Decision |
|---|---|
| [0001](adr/0001-desktop-shell-tauri.md) | Tauri (Rust core + native WebView UI) as the desktop shell |
| [0002](adr/0002-clean-architecture-modular-core.md) | Clean Architecture as a modular monolith, one crate per module, no microservices |
| [0003](adr/0003-encrypted-sqlite-single-file-store.md) | SQLCipher-encrypted single-file SQLite as the only durable store |
| [0004](adr/0004-llm-provider-trait-local-first.md) | `LlmProvider` trait, local-first default, providers as plugins |
| [0005](adr/0005-observation-plugin-per-platform.md) | Observation sources as capability-scoped plugins, one per platform/signal-type |
| [0006](adr/0006-capture-classify-redact-summarize-pipeline.md) | Event Pipeline as a strict, in-memory-first, one-directional pipeline |
| [0007](adr/0007-sqlite-vec-embedding-store.md) | `sqlite-vec` in-process vector search over summary embeddings |
| [0008](adr/0008-os-credential-vault-key-management.md) | OS credential vault as the sole root of key material |
| [0009](adr/0009-wasm-plugin-sandbox.md) | WASM component-model sandbox with structural capability enforcement for all plugins |
| [0010](adr/0010-hybrid-deterministic-plus-llm-recommendation-engine.md) | Recommendation engine: deterministic pattern detection (facts) + LLM synthesis (judgment), validated against drift |

## What's deliberately not in Phase 2

Per [01-prd.md](01-prd.md) §8: cross-device sync, any aggregate/team view, and a public plugin marketplace are out of scope for this design and would each need their own ADRs, threat-model addenda, and (for the aggregate view) a fresh ethical review before being considered at all — not an oversight, a boundary.

## Next: Phase 3

UX, wireframes, user journeys, onboarding flow detail, accessibility, privacy dashboard design, settings — per `PROMPT.md`'s Phase 3.
