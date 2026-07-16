# ADR-0002: Clean Architecture as a modular monolith (single Rust core, plugin-extensible)

Status: Accepted

## Context

PROMPT.md lists 14 suggested modules (Observation Engine, Event Pipeline, Privacy Engine, Redaction Engine, Pattern Detection, Workflow Graph, Recommendation Engine, Knowledge Base, Embedding Layer, LLM Provider Layer, Security Layer, Plugin Framework, Enterprise Policy Engine, UI) and asks for Clean Architecture and extensibility. A desktop product with no server backend rules out microservices; the question is how to get modularity and testability without operational complexity.

## Decision

Structure the Rust core as a **modular monolith**: one process, organized as independent crates with Clean Architecture layering (domain → application/use-cases → infrastructure/adapters), one crate per PROMPT.md module. Dependencies point inward only (infrastructure depends on application depends on domain; never the reverse). Cross-cutting extensibility (new observation sources, new LLM providers, new pattern detectors) happens through the Plugin Framework (ADR-0009), not by adding new inter-process services.

Layering, concretely:

- **Domain**: `WorkflowEvent`, `Pattern`, `Recommendation`, `PrivacyLevel`, `PluginManifest` — pure data + invariants, no I/O.
- **Application**: use-cases — `ClassifyEvent`, `DetectPattern`, `GenerateRecommendation`, `ExportUserData`, `ChangePrivacyLevel` — orchestrate domain objects via trait-defined ports (`ObservationSource`, `LlmProvider`, `EventStore`, `EmbeddingStore`).
- **Infrastructure**: concrete adapters — the SQLCipher-backed `EventStore`, the `keyring`-backed `SecretStore`, the WASM plugin host, the Ollama/OpenAI `LlmProvider` implementations, OS-specific `ObservationSource` implementations.

## Consequences

- Each module can be tested in isolation (mock the ports) without spinning up OS-level observation hooks or a real LLM.
- A single process avoids the operational burden (IPC latency, partial-failure handling, service discovery) that a microservice split would add for zero benefit in a single-user desktop context.
- Enterprise/air-gapped deployment stays simple: one process, one set of permissions, no inter-service network traffic to secure or audit.
- Risk: a modular monolith can decay into a "big ball of mud" without discipline — mitigated by enforcing the dependency-direction rule in CI (a lint/build check that infrastructure crates aren't imported by domain/application crates).

## Alternatives considered

- **Microservices / separate background daemon + UI process**: rejected — no multi-tenancy or independent-scaling need exists in a single-user desktop app; it would only add IPC surface area to secure (relevant per the zero-trust threat model, [../../research/06-threat-model.md](../../research/06-threat-model.md)) without a corresponding benefit.
- **Monolithic script-style app with no layering**: rejected — PROMPT.md explicitly requires extensibility and avoidance of technical debt; an unstructured core would make the Plugin Framework's port/adapter boundaries impossible to define cleanly.
