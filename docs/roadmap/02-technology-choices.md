# Technology Choices

Consolidates the ADRs ([../design/adr/](../design/adr/)) into a single concrete dependency/tooling list for implementation. Where an ADR already justified a choice, this doc only names the specific crate/tool version family — the "why" lives in the ADR.

## Core stack

| Layer | Choice | ADR |
|---|---|---|
| Desktop shell | Tauri 2.x | [0001](../design/adr/0001-desktop-shell-tauri.md) |
| Core language | Rust (stable channel) | [0001](../design/adr/0001-desktop-shell-tauri.md), [0002](../design/adr/0002-clean-architecture-modular-core.md) |
| UI language/framework | TypeScript + React | [0001](../design/adr/0001-desktop-shell-tauri.md) |
| Primary datastore | SQLite + SQLCipher (`rusqlite`, `bundled-sqlcipher` feature) | [0003](../design/adr/0003-encrypted-sqlite-single-file-store.md) |
| Vector search | `sqlite-vec` extension | [0007](../design/adr/0007-sqlite-vec-embedding-store.md) |
| Key management | `keyring` crate (Keychain/DPAPI/Secret Service) | [0008](../design/adr/0008-os-credential-vault-key-management.md) |
| Passphrase KDF (Portable Mode) | Argon2id (`argon2` crate) | [0008](../design/adr/0008-os-credential-vault-key-management.md) |
| Plugin sandbox | `wasmtime` + WASM Component Model / WIT | [0009](../design/adr/0009-wasm-plugin-sandbox.md) |
| Default local AI runtime | Ollama (HTTP API client) | [0004](../design/adr/0004-llm-provider-trait-local-first.md) |
| Cryptography primitives | `ring` or `RustCrypto` crates (AEAD, CSPRNG) | [06-security-architecture.md](../design/06-security-architecture.md) |

## Per-platform observation integration

| Platform | Mechanism | Notes |
|---|---|---|
| macOS | `NSWorkspace` (active app), Accessibility API (`AXUIElement`) for Level 3+, `CGWindowListCopyWindowInfo` for window metadata | Requires Accessibility + (Level 4) Screen Recording TCC grants — requested only when needed, per [0005](../design/adr/0005-observation-plugin-per-platform.md) |
| Windows | Win32 (`GetForegroundWindow`, `SetWinEventHook`), UI Automation (`IUIAutomation`) for Level 3+ | UI Automation COM interop via `windows-rs` |
| Linux | AT-SPI (`atspi` crate or D-Bus direct) where available; X11 (`x11rb`) global hooks on X11 sessions; reduced capability set on Wayland compositors lacking equivalent global hooks, surfaced honestly per [0005](../design/adr/0005-observation-plugin-per-platform.md) | Wayland gap is a known, disclosed platform limitation, not a bug to silently work around via compositor-specific hacks |

## Packaging and distribution

| Platform | Mechanism | PROMPT.md requirement |
|---|---|---|
| Windows | Signed `.msi` (via `tauri-bundler` / WiX), Winget manifest | Signed installer, Winget |
| macOS | Signed + notarized `.dmg` (universal or separate Intel/Apple-Silicon builds), Homebrew Cask formula | Notarized, Apple Silicon native |
| Linux | AppImage, Flatpak manifest; Snap optional | AppImage/Flatpak required, Snap optional |
| All | Portable build: same binary, `--portable` flag or dedicated build target that only ever reads/writes a local data directory | Portable Mode |
| Updates | `tauri-plugin-updater` with signature verification against the embedded vendor public key | Signed updates |

## Testing and CI tooling

| Concern | Tool |
|---|---|
| Rust unit/integration tests | `cargo test`, workspace-wide, per-crate |
| Dependency-direction lint (ADR-0002) | Custom `cargo` xtask or `cargo-deny`-style rule checking crate dependency graph against the layering rule |
| UI component tests | Vitest/React Testing Library |
| E2E (full app) | `tauri-driver` (WebDriver-based) driving real onboarding/dashboard/recommendation journeys ([../ux/01-user-journeys.md](../ux/01-user-journeys.md)) |
| Accessibility | axe-core integrated into UI CI ([../ux/06-accessibility.md](../ux/06-accessibility.md) §5) |
| Security fuzzing | `cargo-fuzz` against the redaction engine's classifiers and the WASM manifest parser |
| Cross-platform build matrix | GitHub Actions (or equivalent) matrix: windows-latest, macos-latest (+ an Apple Silicon runner), ubuntu-latest |

## Explicit non-choices (rejected alternatives, for reference)

See each ADR's "Alternatives considered" section for the reasoning; summarized here for a single-glance view: Electron (rejected, [0001](../design/adr/0001-desktop-shell-tauri.md)), microservices/separate daemon (rejected, [0002](../design/adr/0002-clean-architecture-modular-core.md)), Postgres/pgvector or a standalone vector DB (rejected, [0003](../design/adr/0003-encrypted-sqlite-single-file-store.md)/[0007](../design/adr/0007-sqlite-vec-embedding-store.md)), LangChain-as-provider-abstraction (rejected, [0004](../design/adr/0004-llm-provider-trait-local-first.md)), OS-process-per-plugin isolation (rejected for the default case, [0009](../design/adr/0009-wasm-plugin-sandbox.md)), pure-LLM or pure-rule-based recommendation generation (rejected, [0010](../design/adr/0010-hybrid-deterministic-plus-llm-recommendation-engine.md)).
