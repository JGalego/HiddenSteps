# Core crates — implementation status

This is the honest, current-state complement to [docs/design/02-system-architecture.md](../docs/design/02-system-architecture.md)'s target module map. It says what's actually built, what's verified against a real backend vs. a mock, and what's still genuinely missing — not what's planned (that's [docs/roadmap/01-implementation-roadmap.md](../docs/roadmap/01-implementation-roadmap.md)).

Run `cargo build --workspace && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings` from the repo root. As of this writing: **12 crates, 140 passing tests, zero clippy warnings, `cargo fmt --check` clean.** Two tests are `#[ignore]`d by design (see below) and are not counted as failures or as part of the 140.

## What exists

| Crate | Implements | Verified how |
|---|---|---|
| `hiddensteps-domain` | Core types: `PrivacyLevel`/`PrivacyState`, `EventSummary`/`SignalType`, `Pattern`, `Recommendation`, `AuditEntry`, and `CapturedSignal` — a type that structurally cannot be persisted (no `Serialize`), enforcing ADR-0006's raw-data rule at the type level | Unit tests: level round-tripping/ordering, Deep-mode TTL gating |
| `hiddensteps-security` | `SecretStore` (ADR-0008): real OS-vault (`KeyringSecretStore`) + in-memory (test) implementations; CSPRNG master-key generation; Argon2id passphrase derivation for Portable Mode | Unit tests against the in-memory store and the KDF; the real-vault round trip is `#[ignore]`d (see below) |
| `hiddensteps-event-store` | `SqlCipherEventStore` (ADR-0003): the full schema from [docs/design/07-database-schema.md](../docs/design/07-database-schema.md), CRUD for privacy state, events, audit log, patterns, pattern↔event links, pattern embeddings (see note below), and recommendations, plus `delete_all_data`/`export_data` | 18 tests against a **real** SQLCipher file: wrong key fails to open, same key reopens correctly, delete-all clears every table |
| `hiddensteps-redaction` | The Redaction Engine (`docs/design/05-privacy-model.md` §4): regex+Luhn detectors for API keys/tokens/PEM keys/emails/SSNs/credit cards, an entropy-based ambiguous-secret detector, and the drop-on-uncertainty policy | 23 tests, including deliberately adversarial inputs (secrets embedded in prose, near-miss non-secrets like git SHAs) |
| `hiddensteps-pipeline` | The Event Pipeline (ADR-0006): Classify → Redact → Summarize, privacy-level gating per signal type, Deep-mode TTL assignment | 8 tests covering redaction-triggered drops, level-gating drops, and successful summarization |
| `hiddensteps-observation` | `ObservationSource` (ADR-0005) + **Linux**: `ActiveWindowSource` (X11 `GetInputFocus`), `FileOperationSource` (inotify via `notify`), `ClipboardMetadataSource` (X11 selection, metadata-only), `GlobalShortcutSource` (X11 `XGrabKey`). Plus macOS/Windows source files (see below) | **10 of 11 tests run against real backends in this environment** — a live X11 display (WSLg's `DISPLAY=:0`) and real inotify, not mocks. 1 test (`GlobalShortcutSource`'s real grab) is `#[ignore]`d by design |
| `hiddensteps-llm-provider` | `LlmProvider` (ADR-0004): Ollama client, an OpenAI-wire-compatible client (covers OpenAI/Azure/OpenRouter/Together/Groq/DeepSeek/LocalAI), an Anthropic Messages client, and local-runtime auto-detection | 14 tests against real HTTP request/response assertions via `wiremock` mock servers — exact request shape and response parsing verified, not assumed |
| `hiddensteps-patterns` | Pattern Detection (sliding-window n-gram sequence matching) + Workflow Graph (transition graph with edge weights) — ADR-0010's Layer 1 | 10 tests, including a direct analog of PROMPT.md's own "observed 31 times" example |
| `hiddensteps-recommendations` | The Recommendation Engine's Layer 2 (ADR-0010): LLM synthesis with a structured-JSON prompt contract, a narrative-contradiction validator, and a retry loop — critically, the numeric fields (`estimated_time_saved_minutes`) are **never parsed from the LLM's output at all**, only computed from Layer 1 | 12 tests, including malformed-JSON retry and narrative-contradiction retry, against a scripted test provider |
| `hiddensteps-privacy-engine` | The cloud-dispatch gate (`docs/design/03-data-flow-diagrams.md` §5) and consent versioning (`docs/design/05-privacy-model.md` §5); `PrivacyGatedProvider` wraps any `LlmProvider` so the gate can't be bypassed by the normal call path | 13 tests, including that Level-4 content is blocked even with every consent granted |
| `hiddensteps-plugin-host` | The WASM Plugin Host (ADR-0009): closed capability enumeration, manifest validation, and a `wasmtime`-backed sandbox that links in only granted capabilities' host functions | 12 tests, including **real capability-escape attempts**: hand-written WAT modules compiled at test time, proving an ungranted capability's import is genuinely unresolved (instantiation fails), not merely unused |
| `hiddensteps-enterprise-policy` | Policy schema (`docs/design/05-privacy-model.md` §6) with exactly two knobs (privacy-level floor, provider allowlist) — no field exists for anything else a policy might want to constrain | 6 tests, including parsing a maximally adversarial policy file with five extra excluded-by-design keys and confirming none of them survive parsing |

Plus, outside `crates/` (not part of the root workspace — see why below):

| Location | Implements | Verified how |
|---|---|---|
| `../apps/desktop/ui` | React/TypeScript UI: `PrivacyDashboard` and `RecommendationCard` (docs/ux/03 and 04), talking to the core only through a typed `tauriBridge.ts` | 9 tests via `vitest` + `@testing-library/react` against real jsdom rendering; `tsc -b` typechecks clean |
| `../apps/desktop/src-tauri` | The Tauri shell: ~15 IPC commands (docs/design/09-api-specification.md) wiring every crate above together, plus a real capture→pipeline→store→UI-event background loop for Linux | **Not compiled** — see `../apps/desktop/README.md` |

## What's a disclosed simplification, not a gap

- **Pattern embeddings** are stored as plain BLOBs in `hiddensteps-event-store` with cosine similarity computed in Rust, standing in for ADR-0007's `sqlite-vec` virtual table (see the comment at the top of `event-store/src/schema.sql`) — loading a native SQLite extension wasn't verifiable in this environment, and ADR-0007 itself notes that at realistic single-user volumes, `sqlite-vec`'s own behavior *is* brute-force exact search. Same semantics, no native-extension risk.
- **`hiddensteps-observation`'s macOS and Windows modules** (`src/macos/`, `src/windows/`) are real, complete source against long-stable platform APIs (`CGWindowListCopyWindowInfo`; `GetForegroundWindow`/`GetWindowTextW`/`QueryFullProcessImageNameW`) but **have never been compiled** — no macOS/Windows toolchain was available. Each module's doc comment says so explicitly. `.github/workflows/ci.yml`'s `core` job matrix is where these get their first real compilation.
- **Browser-domain observation and global-shortcut auto-start** are named gaps in `hiddensteps-observation/src/lib.rs`'s doc comment — the former needs a separate browser-extension artifact this repo doesn't contain; the latter (`GlobalShortcutSource`) is implemented but never auto-started, because grabbing a key combo session-wide in a shared dev sandbox would be actively disruptive.
- **The Recommendation Engine's Layer 2** synthesizes qualitative judgment via whichever `LlmProvider` is configured; it has been tested against a scripted stand-in provider (real assertions on retry/validation logic) and against `wiremock`-mocked HTTP (in `hiddensteps-llm-provider`), but not yet against a real running Ollama instance — that integration is mechanically identical to the mocked test, just pointed at `http://localhost:11434` instead of a mock server.

## Why the Tauri shell and UI live outside `crates/`

`crates/*` is the root `Cargo.toml` workspace and is fully buildable/testable in this Linux dev environment with zero system dependencies beyond what `cargo` fetches. `apps/desktop/src-tauri` needs `webkit2gtk-4.1` (Linux) to even compile, which this environment cannot install (no passwordless `sudo`, no working `nix`/package-manager path — confirmed by direct attempt). Keeping it out of the workspace means `cargo build --workspace` stays 100% green here rather than permanently red on one crate nobody can fix in this sandbox. `apps/desktop/ui` has no such constraint and is verified the same way the Rust core is.

## A note on the two `#[ignore]`d tests

- `hiddensteps-security::keyring_store::tests::set_get_delete_round_trip_against_the_real_vault` — needs a real OS credential vault/desktop session.
- `hiddensteps-observation::linux::shortcuts::tests::grabs_and_ungrabs_a_real_shortcut` — performs a real session-wide `XGrabKey`, which would be disruptive to run automatically in a shared environment.

Both are real tests, not vestigial — [docs/roadmap/03-testing-strategy.md](../docs/roadmap/03-testing-strategy.md) §2 draws exactly this distinction between logic that belongs behind a mock in CI and OS/session integration that belongs in deliberate, manual verification. Run either with `cargo test -- --ignored <test name>` on a machine where running them is appropriate.
