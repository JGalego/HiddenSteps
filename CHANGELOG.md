# Changelog

## v0.1.1 — fixes found by actually running v0.1.0

Every item here was found by hands-on use of the real v0.1.0 installers, not by review — the fastest possible feedback loop once a build exists.

### Fixed

- **Onboarding crashed at the validation step** ("provider returned an error: 404 Not Found: model 'default' not found") — the wizard let you reach validation having never chosen a model, and the backend silently substituted a literal, nonexistent model named `"default"`. `get_provider_detection` now reports each runtime's *actual* available models (parsed from Ollama's `/api/tags` and the OpenAI-compatible `/v1/models`), the wizard auto-selects a sensible one, and a genuinely missing model now surfaces as its own clear error instead of a fake model name.
- **Cloud providers (OpenAI, Anthropic) had no API key field at all** — onboarding was guaranteed to fail with 401 for either, unconditionally. Step 5 now has a real password-masked API key input for cloud providers, stored only in the OS credential vault.
- **The provider you picked during onboarding was silently never saved** — finishing onboarding never called `set_ai_provider`, so Settings showed "No provider configured yet." immediately afterward. `startObserving` now persists it before completing.
- **The entire UI had zero CSS** — every screen rendered in the browser/webview's raw default styling. Added a real design system (colors from the app icon, typography, buttons, forms, focus states, a genuine light-mode override) wired through every component.

### Also in this release

- `hiddensteps-llm-provider`: 2 new tests for the model-list parsing (16 tests total in that crate).
- `apps/desktop/ui`: 5 new `OnboardingWizard` tests covering auto-selection, the no-models-detected fallback, the cloud API-key field, and that the real selected model (never a hardcoded default) is what gets sent (31 UI tests total).
- Root workspace: 150 passing tests, zero clippy warnings, clean `cargo fmt --check`.

## v0.1.0 — first tagged build

Early, unsigned, prerelease. The Rust core is real and tested; the desktop app around it is functional but young. See [`crates/README.md`](crates/README.md) and [`apps/desktop/README.md`](apps/desktop/README.md) for the honest, current-state breakdown of what's verified how.

### Core

- Observation (Linux: active window, file operations, clipboard metadata, global shortcuts — all against real backends; macOS/Windows: written, CI-compiled).
- The Classify → Redact → Summarize event pipeline, with a drop-on-uncertainty redaction policy (secrets, API keys, PII, financial identifiers).
- Deterministic pattern detection + workflow graph construction.
- A two-layer recommendation engine: deterministic facts (frequency, time span) plus LLM-synthesized judgment, with the LLM structurally unable to override the numeric fields.
- LLM provider clients for Ollama, OpenAI-compatible APIs (OpenAI/Azure/OpenRouter/Together/Groq/DeepSeek/LocalAI), and Anthropic.
- A privacy-dispatch gate enforcing cloud-eligibility rules per privacy level, with Level 4 (Deep-mode) content never eligible for cloud dispatch under any configuration.
- A WASM plugin sandbox (`wasmtime`) with structural capability enforcement — verified against real capability-escape attempts, not just declared manifests.
- An enterprise policy schema with exactly two knobs (privacy-level floor, provider allowlist) and no field for anything else a policy might try to constrain.
- A single SQLCipher-encrypted local store for everything durable.

### Desktop app

- Onboarding wizard (all 8 screens from the UX spec), privacy dashboard, recommendations view, settings, and diagnostics — built and tested in the UI layer.
- Tauri shell wiring ~21 IPC commands to the core, including a real capture → pipeline → store → UI-event background loop on Linux.

### Known limitations in this release

- Builds are unsigned (no code-signing certificates yet) — expect a Gatekeeper/SmartScreen prompt on first run.
- macOS builds target the GitHub Actions runner's native architecture only (not yet a universal Intel+Apple Silicon binary).
- No exclusion-rule or plugin-management UI yet.
- Browser-domain observation isn't implemented (needs a separate browser-extension component).
- No package-manager listings yet (Winget/Homebrew/Flatpak) — download directly from Releases.
