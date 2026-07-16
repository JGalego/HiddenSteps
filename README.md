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
| 5 | Implementation | In progress — milestone M0 (foundation) underway, see [crates/](crates/) |

## Repository layout

```
docs/
  research/   Phase 1 — competitive analysis, market gaps, risk/ethical/privacy analysis, threat model
  design/     Phase 2 — PRD, ADRs, system architecture, data flow, trust/privacy/security models, DB schema, plugin & API specs
  ux/         Phase 3 — user journeys, onboarding wireframes, privacy dashboard, recommendations UX, settings IA, accessibility
  roadmap/    Phase 4 — milestones, technology choices, testing strategy, security/privacy/performance test plans
crates/       Phase 5 — the Rust core, one crate per docs/design/02-system-architecture.md module (built out incrementally per milestone)
```

## Building and testing the core

The core is a Rust workspace; the desktop shell (Tauri + TypeScript/React UI, per [ADR-0001](docs/design/adr/0001-desktop-shell-tauri.md)) is not yet scaffolded — see [crates/README.md](crates/README.md) for exactly what exists so far and what's deliberately deferred.

```sh
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets
```

One test (`hiddensteps-security`'s real-vault round trip) is `#[ignore]`d by default because it requires a real OS credential vault / desktop session, per [docs/roadmap/03-testing-strategy.md](docs/roadmap/03-testing-strategy.md) §2. Run it explicitly with `cargo test -- --ignored` on a machine with one available.
