# HiddenSteps

A local-first desktop application that observes how you work — at a privacy level you choose — and explains, over time, where your effort is going and what your real options are for getting it back. It never automates or acts on your behalf by default; see [docs/research/02-market-gaps-and-differentiation.md](docs/research/02-market-gaps-and-differentiation.md) for what that means and why it matters.

Built per the phased process in [PROMPT.md](PROMPT.md).

## Status

| Phase | Deliverable | Status |
|---|---|---|
| 1 | Research (competitive landscape, market gaps, risk/ethical/privacy analysis, threat model) | Done — [docs/research/](docs/research/) |
| 2 | Design (PRD, ADRs, architecture, schema, plugin/API specs) | Done — [docs/design/](docs/design/) |
| 3 | UX (journeys, wireframes, onboarding, accessibility) | Done — [docs/ux/](docs/ux/) |
| 4 | Implementation roadmap and test plans | Done — [docs/roadmap/](docs/roadmap/) |
| 5 | Implementation | In progress — core (M1-M4) built and tested; Tauri shell + UI (M5) written, UI tested, shell unverified; CI/packaging (M6) drafted — see [crates/](crates/) and [apps/desktop/](apps/desktop/) |

## Repository layout

```
docs/
  research/   Phase 1 — competitive analysis, market gaps, risk/ethical/privacy analysis, threat model
  design/     Phase 2 — PRD, ADRs, system architecture, data flow, trust/privacy/security models, DB schema, plugin & API specs
  ux/         Phase 3 — user journeys, onboarding wireframes, privacy dashboard, recommendations UX, settings IA, accessibility
  roadmap/    Phase 4 — milestones, technology choices, testing strategy, security/privacy/performance test plans
crates/       Phase 5 — the Rust core: domain, security, event-store, redaction, pipeline, observation, llm-provider,
              patterns, recommendations, privacy-engine, plugin-host, enterprise-policy (see crates/README.md for detail)
apps/desktop/ Phase 5 — the Tauri shell + React/TypeScript UI (see apps/desktop/README.md — built outside the root
              workspace on purpose; see why there)
.github/workflows/ci.yml — cross-platform build/test, including the first real compilation of code this dev
              environment couldn't verify (see crates/README.md)
```

## Building and testing the core

The core is a Rust workspace, fully buildable and testable in a plain Linux/macOS/Windows dev environment with no system dependencies beyond what `cargo` fetches. See [crates/README.md](crates/README.md) for exactly what's built, what's verified against a real backend vs. mocked, and what's a disclosed simplification.

```sh
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --check
```

Two tests are `#[ignore]`d by default — one needs a real OS credential vault/desktop session, one performs a real session-wide global-keyboard-shortcut grab that would be disruptive to run automatically (see [crates/README.md](crates/README.md)'s closing section for both, per [docs/roadmap/03-testing-strategy.md](docs/roadmap/03-testing-strategy.md) §2). Run either explicitly with `cargo test -- --ignored <name>` on a machine where doing so is appropriate.

## The desktop app

```sh
cd apps/desktop/ui && npm install && npm test   # UI: builds and tests cleanly here
cd apps/desktop/src-tauri && cargo build         # Shell: needs webkit2gtk (Linux) — see apps/desktop/README.md
```
