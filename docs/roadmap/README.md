# HiddenSteps — Phase 4: Implementation Roadmap

Phase 4 deliverable set, per `PROMPT.md`. Turns the Phase 2 architecture and Phase 3 UX into a sequenced build plan and the test suites that gate each release.

## Reading order

1. [01-implementation-roadmap.md](01-implementation-roadmap.md) — milestones M0-M6, exit criteria per milestone, and the rationale for sequencing (AI-free observation before AI, security hardening before third-party plugins, breadth work last but not deprioritized).
2. [02-technology-choices.md](02-technology-choices.md) — the concrete crate/tool list realizing every ADR, per-platform observation integration specifics, packaging/distribution mechanisms, and CI tooling.
3. [03-testing-strategy.md](03-testing-strategy.md) — the general test pyramid and the rule for which guarantees must be tested against real components, never mocks.
4. [04-security-testing.md](04-security-testing.md) — adversarial suites against the threat model, external review requirements, cadence.
5. [05-privacy-testing.md](05-privacy-testing.md) — per-level signal-boundary tests, retention/deletion completeness, redaction correctness, cloud-dispatch gating, consent versioning, enterprise-policy boundaries.
6. [06-performance-testing.md](06-performance-testing.md) — resource budgets, test scenarios, hardware-coverage matrix (including the equity angle from the ethical analysis), cadence.

## How this phase relates to the earlier ones

Nothing here introduces a new product decision — Phase 4 is entirely in service of shipping what Phases 1-3 already specified, correctly and verifiably. Where a test suite's pass criterion reads like a restatement of a Phase 2 guarantee, that's intentional: the point of this phase is to make every claim in `../design/` and `../research/` checkable against a real build, not just true on paper.

## Next: Phase 5

Implementation — write the production-quality code itself, maintaining the Clean Architecture layering, with tests and documentation throughout, per `PROMPT.md`'s Phase 5.
