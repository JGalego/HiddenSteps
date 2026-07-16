# Changelog

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
