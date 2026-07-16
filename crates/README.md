# Core crates — implementation status

This is the honest, current-state complement to [docs/design/02-system-architecture.md](../docs/design/02-system-architecture.md)'s target module map. It says what's actually built, what's deliberately deferred, and why — not what's planned (that's [docs/roadmap/01-implementation-roadmap.md](../docs/roadmap/01-implementation-roadmap.md)).

## What exists (milestone M0 — foundation)

| Crate | Implements | Key guarantees tested |
|---|---|---|
| `hiddensteps-domain` | Core types: `PrivacyLevel`, `PrivacyState`, `EventSummary`, `SignalType`, `CapturedSignal`, `AuditEntry`/`AuditActor` — no I/O | `PrivacyLevel` round-trips through its numeric form and orders correctly; `EventSummary::new` never attaches a Deep-mode TTL to a non-Deep-mode event even if a caller tries |
| `hiddensteps-security` | `SecretStore` trait (ADR-0008) + `KeyringSecretStore` (real OS vault, via the `keyring` crate) + `InMemorySecretStore` (for unit-testing application logic without a real vault) + `generate_master_key` (CSPRNG) + `derive_key_from_passphrase` (Argon2id, for Portable Mode) | CSPRNG output doesn't repeat across calls; same passphrase+salt always re-derives the same key, different passphrases/salts never collide; the real-vault round trip is `#[ignore]`d (see below) |
| `hiddensteps-event-store` | `SqlCipherEventStore`: the single encrypted SQLite file (ADR-0003), full schema from [docs/design/07-database-schema.md](../docs/design/07-database-schema.md), CRUD for privacy state / event summaries / audit log, plus `delete_all_data` and `export_data` | Wrong key fails to open an existing encrypted file; same key reopens it correctly; the recent-events read path returns exactly what was inserted (no paraphrasing); delete-all clears every table and resets privacy state to Level 0/inactive |

Run `cargo test --workspace` from the repo root to exercise all of the above (23 tests, one `#[ignore]`d).

## What's deliberately not here yet

Nothing below is missing by oversight — each is scoped to a later milestone in [docs/roadmap/01-implementation-roadmap.md](../docs/roadmap/01-implementation-roadmap.md), and building it now would mean building it against invented requirements instead of the real ones the earlier milestones will surface:

- **Observation plugins** (per-platform app/window/clipboard/browser capture) — M1. No `ObservationSource` implementation exists yet; there's nothing to feed `EventStore` in a running app yet, only the store itself.
- **Event Pipeline** (Classify → Redact → Summarize, ADR-0006) — M1. `EventStore` accepts already-summarized `EventSummary` values; the pipeline that produces them from a `CapturedSignal` doesn't exist yet.
- **`sqlite-vec` embedding layer, Pattern Detection, Workflow Graph, Recommendation Engine** — M2. The `summary_embeddings` virtual table is intentionally *not* in `schema.sql` yet (see the comment at the top of that file) — there's nothing to embed before Pattern Detection exists to produce pattern-level summaries.
- **`LlmProvider` trait and any provider implementation** (Ollama, cloud) — M2.
- **WASM plugin host** (ADR-0009) — M4.
- **Tauri shell / UI** (ADR-0001) — not started. Building the GUI shell requires system WebView libraries (`webkit2gtk-4.1` + friends on Linux) that aren't present in every dev environment; the Rust core above is deliberately structured so it's fully buildable and testable via `cargo test` with zero GUI dependency, per the Clean Architecture separation in ADR-0002 — the UI is a client of this core, not a prerequisite for it.

## A note on the one `#[ignore]`d test

`hiddensteps-security::keyring_store::tests::set_get_delete_round_trip_against_the_real_vault` exercises the real OS credential vault (Keychain/DPAPI/Secret Service) and is skipped by default because a headless/sandboxed dev environment may not have one available — this is the exact distinction [docs/roadmap/03-testing-strategy.md](../docs/roadmap/03-testing-strategy.md) §2 draws between logic that should be unit-tested against a mock (`InMemorySecretStore` covers that) and OS integration that needs a real backend and belongs in manual/CI-on-real-platforms testing. Run it explicitly with `cargo test -- --ignored` on a machine with a real desktop session.
