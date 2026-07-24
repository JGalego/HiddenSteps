# Implementation Roadmap and Milestones

Sequenced so that every milestone ships something genuinely usable and testable against the Phase 2/3 specs — no milestone is "plumbing with nothing to show." Ordering respects dependency reality (you can't test the Recommendation Engine's LLM synthesis without an `EventStore` and at least one `LlmProvider`), and front-loads the pieces that most directly de-risk the items in [../research/03-risk-analysis.md](../research/03-risk-analysis.md).

## M0 — Foundation (infrastructure, no user-visible behavior)

- Repo/workspace scaffolding: Rust workspace with crate boundaries from [../design/02-system-architecture.md](../design/02-system-architecture.md); Tauri shell skeleton; CI (build matrix across Windows/macOS/Linux, the dependency-direction lint from [../design/adr/0002-clean-architecture-modular-core.md](../design/adr/0002-clean-architecture-modular-core.md)).
- `EventStore` (SQLCipher, ADR-0003) with the schema from [../design/07-database-schema.md](../design/07-database-schema.md), migrations, and the delete-all/export operations.
- `SecretStore` (OS credential vault, ADR-0008), including the Portable Mode passphrase-derived path.
- **Exit criteria:** a headless integration test can create a profile, write/read encrypted rows, export, and fully delete-all, on all three platforms in CI.

## M1 — Minimal observation loop (no AI yet)

- In-tree Minimal/Standard `ObservationSource` plugins per platform (app focus, window title, keyboard shortcuts, browser domain, clipboard metadata, file-op metadata).
- Event Pipeline (Classify → Redact → Summarize, ADR-0006) with the drop-on-uncertainty redaction policy.
- Deterministic Pattern Detection (exact sequence matching only — embeddings come in M2).
- Onboarding flow through privacy-level selection (skip AI-provider screen for now — hardcode "no AI yet"); privacy dashboard with live recent-events feed, pause/exclude/delete/export.
- **Exit criteria:** a real user can install, consent, and watch their own app-switching/shortcut patterns show up honestly in the dashboard, with zero AI dependency. This alone should already validate or invalidate the trust-UX decisions in [../ux/](../ux/) before any AI complexity is added.

## M2 — Local AI and the recommendation engine

- `LlmProvider` trait + Ollama implementation (ADR-0004); provider auto-detection and the onboarding AI-provider screen.
- `sqlite-vec` embedding layer (ADR-0007); pattern detection upgraded to similarity search.
- Recommendation Engine Layer 1 (deterministic facts) fully wired to real detected patterns; Layer 2 (LLM synthesis) with the numeric-drift validator (ADR-0010).
- Recommendations UI ([../ux/04-recommendations-ux.md](../ux/04-recommendations-ux.md)) end to end: card, detail view, dismiss-with-reason.
- **Exit criteria:** Zero-to-Value (G1) is testable end-to-end with a real local model — a fresh install, a day of synthetic or dogfooded usage, and a correct first recommendation with full explainability fields populated.

## M3 — Cloud providers, full privacy-level range, trust hardening

- Remaining privacy levels (2-4), including Deep-mode OCR/screenshot capture with in-pipeline redaction and the Deep-mode TTL sweep ([../design/05-privacy-model.md](../design/05-privacy-model.md) §2).
- Cloud `LlmProvider` implementations (OpenAI, Anthropic first; others per demand) with the cloud-dispatch gating flow ([../design/03-data-flow-diagrams.md](../design/03-data-flow-diagrams.md) §5) and per-content-class consent UI.
- Audit log, consent versioning/re-consent-on-manifest-change, sensitive-application exclusion suggestions.
- **Exit criteria:** every trust feature in [../design/04-trust-model.md](../design/04-trust-model.md) §2 exists and is independently testable; a security/privacy reviewer can complete the full adversarial checklist in [04-security-testing.md](04-security-testing.md) and [05-privacy-testing.md](05-privacy-testing.md) against a real build for the first time.

## M4 — Plugin framework

- WASM host (`wasmtime`), manifest schema and validation, capability-grant lifecycle (ADR-0009, [../design/08-plugin-architecture.md](../design/08-plugin-architecture.md)).
- Migrate at least one in-tree observation source to run through the plugin path as a dogfood/proof that first-party and third-party plugins share one real mechanism, not just a shared schema on paper.
- One reference third-party-style plugin built and installed via the file-based distribution path, to validate the manifest/consent/capability UX in Settings ([../ux/05-settings-and-complexity-tiers.md](../ux/05-settings-and-complexity-tiers.md)).
- **Exit criteria:** the capability-escape test suite ([../design/06-security-architecture.md](../design/06-security-architecture.md) §7) passes against the real sandbox, not a mock.

## M5 — Installation, updates, enterprise, portability, accessibility polish

- Signed installers per platform, Portable Mode, auto-updater with signature verification, enterprise policy loading and silent install, air-gapped mode with the network-activity audit view.
- Full accessibility pass against [../ux/06-accessibility.md](../ux/06-accessibility.md)'s testing bar; localization scaffolding (string externalization) even if only English ships.
- Self-diagnostics page complete.
- **Exit criteria:** an enterprise admin can silently deploy with a policy file and confirm (via their own tooling) successful installation without ever gaining data visibility; a screen-reader-only run through Journey 1 succeeds.

## M6 — Beta and GA hardening

- Full Phase-4 test suites (below) run to completion with zero critical findings.
- Real-world beta cohort (small, opted-in, ideally including at least one privacy-conscious-professional-type user per [../design/01-prd.md](../design/01-prd.md) personas) validating Zero-to-Value and trust-feature legibility in practice, not just in spec review.
- GA readiness: update channel live, diagnostics validated against real hardware variety, packaging for all listed distribution channels ([../research](../research) is silent on this — packaging specifics belong here, not in research).

## Sequencing rationale

- AI capability (M2) deliberately comes *after* a working, trustworthy, AI-free observation loop (M1) — this directly tests the ADR-0010 bet that the product has real value even with a weak/absent model, and lets trust-UX get validated without the confound of "was the user reacting to the AI or to the observation."
- Security/privacy hardening (M3) is scheduled before the plugin framework (M4) — third-party extensibility should land on top of an already-hardened core, not be the first place hardening gets tested.
- Enterprise/portability/accessibility (M5) is scheduled last among build-out milestones because it's breadth work across an already-functioning core, not a dependency for anything earlier — but it is not "M5 = deprioritized"; per PROMPT.md these are first-class requirements and M6 (GA) is blocked on M5's exit criteria, not optional.

## Beyond GA — the long-term vision (PROMPT.md's "not simply a desktop agent")

PROMPT.md frames HiddenSteps' end state in terms M0–M6 above don't reach: a
**workflow graph** maturing into a **knowledge graph**, a **continuous workflow
optimizer**, a **workflow memory**, and a **decision-support system** — the
framing that distinguishes it from "a desktop agent." This section records how
today's building blocks relate to that vision so it's an explicit decision, not
an accidental silence. None of the below is scheduled work; it's the direction
the shipped architecture is meant to be able to grow toward without a rewrite.

- **Workflow graph → knowledge graph.** The transition graph built in
  `hiddensteps-patterns` (nodes = action keys, edges = observed transitions
  with weights) is the seed. A knowledge graph adds typed nodes (tools,
  documents, people-as-roles-never-identities) and typed edges (produces,
  consumes, blocks) on top of the same store. The privacy constraint is the
  hard part, not the graph: any richer node type must pass the same
  redaction/summarize discipline (ADR-0006) and cloud-eligibility gate
  (ADR-0004) as everything else — a knowledge graph of *summaries and shapes*,
  never of verbatim content. This is why it isn't a near-term milestone: it's a
  privacy-model extension first and a data-structure second.
- **Continuous optimizer / workflow memory.** Today each recommendation is a
  one-shot suggestion. The "continuous optimizer" is the same engine run as a
  standing loop that also remembers what the user acted on, what changed after,
  and adjusts — i.e., recommendations become stateful and longitudinal. The
  `recommendations` table + audit log already capture the raw material
  (accepted/dismissed, when); turning that into feedback the engine learns from
  is the step, and it must stay explainable (ADR-0010's "numbers trace to real
  events" rule) rather than becoming an opaque model.
- **Decision-support system.** The honest framing of the above: the product
  never decides or acts (that's the NG line in [../design/01-prd.md](../design/01-prd.md));
  "decision support" means presenting the graph + longitudinal trends so the
  *user* decides better. Any feature here is additive to, never a replacement
  for, the "observe and explain, never act" boundary.

Each of these would need its own ADR(s), a threat-model addendum, and — for
anything that widens what's observed or retained — a privacy-model revision and
a fresh ethical review before being scheduled, exactly as
[../design/README.md](../design/README.md)'s "not in Phase 2" note requires for
cross-device sync and aggregate views.
