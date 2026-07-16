# Changelog

## v0.1.3 — Recommendations were never actually generated; now they are

Found the same hands-on way as the last two releases: hundreds of events were captured, "Detected patterns: 0" and "Recommendations: 0" never moved. The cause wasn't "needs more data" — nothing in the running app ever invoked pattern detection or recommendation synthesis at all, on either platform.

### Added

- **A periodic background sweep** (`recommendation_loop.rs`, every 5 minutes) that actually runs `hiddensteps-patterns`' deterministic detector and, for newly-discovered patterns, `hiddensteps-recommendations`' LLM synthesis over stored events — both crates were fully implemented and tested since v0.1.0 but nothing ever called either one.
- **Cloud-dispatch consent, for real.** Recommendation synthesis calls an LLM; for a cloud provider, that's gated through the existing `DispatchGate`/privacy-dispatch architecture, which until now was constructed and never read anywhere. A new Settings toggle ("Allow sending pattern summaries to your cloud AI provider") grants/revokes general cloud consent, persisted and re-applied to the gate on every launch. Local providers are unaffected — they were never gated.

### Fixed

- **Pattern detection couldn't tell apps apart.** The action-key identity pattern detection matched on was `{capture_module_id}:{signal_type}` — e.g. every `windows.active_window` app switch produced the identical key regardless of which app you switched to, since the capture module's own name doesn't vary per app. Detection now derives a real per-signal-type subject from the event summary (the app identifier, the clipboard content type, the shortcut label, an operation+extension for file events, a domain for browser events) where one exists, so a detected pattern is actually about *which* apps/formats recur — "you keep switching Slack → Excel" — not just which capture modules fired near each other.
- File paths and window titles are deliberately excluded from this — too identifying (paths) or too high-cardinality to ever repeat (titles) to belong in a pattern signature that may reach an LLM prompt. Browser domains are included but flagged as containing a verbatim string, gating them the same as any other cloud-dispatch-sensitive content.

## v0.1.2 — Windows observation wired up, and a running-app capture gap closed on both platforms

### Added

- **Windows capture now actually runs.** `apps/desktop/src-tauri/src/observation_loop.rs` previously did nothing at all on non-Linux platforms. It now builds and polls real sources on Windows: active-window (app focus + window title), clipboard metadata (content type + byte size, never content), and file-operation metadata (path + operation type, never file content) for the user's Desktop/Documents/Downloads folders.
- **A global-shortcut source for Windows** (`hiddensteps-observation::windows::GlobalShortcutSource`, via `RegisterHotKey`) joins the existing Linux one — written but, like its Linux counterpart, deliberately never auto-started, since grabbing a key combo is session-wide and invasive.
- **Clipboard and file-operation capture now also run on Linux**, not just active-window — the same three sources above were implemented and tested as standalone modules since v0.1.0 but nothing in the app ever constructed or polled them until now.

### Fixed

- **The Windows data directory never checked `%APPDATA%`.** The database and its containing directory silently fell back to `%TEMP%\hiddensteps` on any machine where `HOME` isn't set — the normal case on Windows. Now resolves `%APPDATA%` on Windows and `~/Library/Application Support` on macOS, matching each platform's real convention.

### Known limitations in this release

- The three new Windows sources (active-window, clipboard, shortcuts) contain hand-written Win32 FFI written without access to a Windows toolchain — genuinely untested until CI's `windows-latest` runner compiles them for the first time. `windows::FileOperationSource` carries lower risk: it has no Win32 FFI of its own, built on the same cross-platform `notify` crate Linux already uses.
- File-operation watching is scoped to Desktop/Documents/Downloads, not the whole home directory, to avoid exhausting OS file-watch limits on a typical developer machine.

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
