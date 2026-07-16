# Product Requirements Document — HiddenSteps

## 1. Summary

HiddenSteps is a local-first desktop application that observes how an individual knowledge worker actually works — at a privacy level they choose — and, over time, surfaces specific, explained recommendations for eliminating, simplifying, delegating, or automating parts of that work. It never executes changes on the user's behalf by default; it earns trust by explaining itself and by collecting the least data necessary to be useful. See [../research/02-market-gaps-and-differentiation.md](../research/02-market-gaps-and-differentiation.md) for why this positioning is defensible against adjacent categories.

## 2. Goals

- **G1**: Give a new user at least one genuinely useful, low-risk, easy-to-understand workflow insight within 24 hours of first launch (Zero-to-Value).
- **G2**: Make observation legible and controllable at every moment — the user always knows what is being watched, can see recent captures, and can pause/delete/downgrade instantly.
- **G3**: Span the full remediation spectrum in recommendations — shortcut → template → script → RPA → workflow platform → AI agent → hybrid — not just "add automation."
- **G4**: Run fully offline with a local AI provider for users who never want cloud dependency, with cloud providers as an explicit, revocable opt-in.
- **G5**: Be installable and configured by a non-technical user in under five minutes, and be extensible by a technical/enterprise user without editing config files by hand.

## 3. Non-goals

- **NG1**: HiddenSteps is not an execution agent by default. It does not open browsers, fill forms, or run scripts on its own initiative. (A future, explicitly-scoped "one-click implement" feature may execute a *user-approved* recommendation — see §9 — but this is opt-in per action, not the product's default mode.)
- **NG2**: HiddenSteps is not an employee-monitoring or team-analytics product. There is no aggregate, multi-user, or manager-facing view in scope for any phase covered by this PRD.
- **NG3**: HiddenSteps is not a screen recorder. Continuous video/screenshot capture is not a default data source at any privacy level below explicit Deep-mode opt-in.
- **NG4**: HiddenSteps does not require a cloud account, subscription, or network connectivity to install, configure, or produce its first recommendation.

## 4. Target users and personas

| Persona | Description | Primary need |
|---|---|---|
| Overloaded individual contributor | Knowledge worker doing repetitive cross-app work (data entry, report assembly, ticket triage) with no IT support to fix it | "Show me I'm wasting time and tell me exactly how to stop, in language I understand" |
| Privacy-conscious professional | Handles sensitive material (legal, healthcare, security-cleared, or simply security-aware) and would never install a screen recorder | "Prove to me, continuously, that you're not doing more than you say" |
| Technical power user / tinkerer | Comfortable with scripts, RPA, local LLMs; wants maximum insight and control | "Give me the raw pattern data and let me wire up my own automation, or at least point me at the right tool" |
| Small-team lead / freelancer consultant | Wants to improve their own workflow and possibly recommend HiddenSteps to clients/teammates individually | "Help me individually — I have no interest in monitoring anyone else, and I need that to be provably true" |
| IT/security admin (enterprise deployment) | Needs to approve, deploy, and constrain the tool org-wide without being able to see any individual's captured data | "Let me set floors and policies, not surveil people" |

## 5. Functional requirements

### 5.1 Observation

- FR-1: Support four observation modes — Minimal, Standard, Deep — each with a precisely documented signal list (see [../research/05-privacy-analysis.md](../research/05-privacy-analysis.md) §Privacy levels), configurable independently of privacy level presets but defaulting together.
- FR-2: Observation is off (Level 0/Manual) until the user completes onboarding and explicitly starts it.
- FR-3: Per-application/per-domain/per-window exclusion rules, settable by the user at any time, take effect immediately (no restart required).
- FR-4: The product must proactively suggest Level 0/Manual or an exclusion rule when it detects use of an application category associated with regulated data (EHR/medical, legal case management, password managers) — per [../research/04-ethical-analysis.md](../research/04-ethical-analysis.md).

### 5.2 Privacy and trust

- FR-5: A persistent, always-reachable privacy dashboard shows: current observation status, current privacy level, current AI provider/model, a feed of recently captured (post-redaction) events, and one-click pause/resume, delete-all, and export.
- FR-6: Deleting data is complete and verifiable within the single-file store (ADR-0003) — no residual copies in caches, logs, or crash reports.
- FR-7: Automatic detection and redaction of passwords, API keys, tokens, secrets, PII, financial identifiers, and medical-information patterns runs before any data reaches durable storage (ADR-0006); on classifier uncertainty, the event is dropped, not stored under-redacted.
- FR-8: Every privacy-level change, pause/resume, deletion, and export is written to a local, tamper-evident audit log containing action metadata only — never captured content.

### 5.3 Pattern detection and recommendations

- FR-9: Detect repeated workflows (same or near-identical action sequences) across sessions spanning days to months, using both exact sequence matching and embedding similarity (ADR-0007).
- FR-10: Every recommendation must state: why it was suggested, confidence, estimated time saved, difficulty, maintenance burden, privacy implications, implementation effort, and at least one alternative approach (per PROMPT.md's Recommendation Engine requirements, enforced structurally per ADR-0010).
- FR-11: Recommendations must span categories: productivity (shortcuts/features/templates/snippets), traditional scripting, browser automation, RPA, workflow platforms, AI/prompts/copilots, agentic frameworks, and hybrid combinations.
- FR-12: The system must produce at least one recommendation within the user's first 24 hours of active observation if any qualifying repeated pattern exists (Zero-to-Value, G1); if none exists yet, the dashboard must say so honestly rather than manufacture a low-value suggestion.
- FR-13: The underlying quantitative claims in a recommendation (frequency, time span, estimated cost) must be traceable back to the specific detected pattern and observation events (post-redaction) that produced them, viewable on demand ("what observations contributed?").

### 5.4 AI providers

- FR-14: Auto-detect locally running Ollama, LM Studio, LocalAI, and vLLM endpoints, and installed llama.cpp binaries, at onboarding and on demand from Settings; recommend a default based on detected hardware (RAM, GPU).
- FR-15: Support configuring any of OpenAI, Anthropic, Google, Azure OpenAI, OpenRouter, Together, Groq, Mistral, Cohere, DeepSeek via API key + optional endpoint, with automatic connectivity testing before the provider is marked active.
- FR-16: No data above the active privacy level's cloud-eligible threshold (per [../research/05-privacy-analysis.md](../research/05-privacy-analysis.md)) may be sent to a cloud provider without a separate, explicit consent step beyond general provider selection.

### 5.5 Onboarding and progressive complexity

- FR-17: First-run flow: explain what HiddenSteps does / does not do → explain every requested OS permission and why → choose privacy level → choose AI provider → validate configuration → begin observing only after explicit consent (PROMPT.md First Run Experience, verbatim ordering required).
- FR-18: Three complexity tiers (Beginner/Intermediate/Advanced) gate UI surface area, not capability — an Advanced user can reach every setting a Beginner's simplified UI hides, without a separate build or hidden flag.
- FR-19: A Self-Diagnostics page reports AI provider status, model status, GPU/CPU/memory/storage usage, observation permission status, security/encryption status, and update status, in language a non-technical user can act on.

### 5.6 Installation, updates, portability

- FR-20: Ship signed installers for Windows (.exe/.msi), macOS (.dmg, notarized, Apple Silicon native), and Linux (AppImage, Flatpak); a Portable Mode requiring no installation, storing all data under one directory, leaving no residual files after that directory is deleted.
- FR-21: Support automatic, manual, offline, and enterprise-managed update channels, all preserving user settings and encrypted data across versions; updates are signature-verified before install (ADR-0008/threat model).
- FR-22: Support enterprise silent installation, policy-file-driven configuration, and air-gapped deployment, with policy able to constrain privacy-level floors and approved AI providers, but never able to grant remote visibility into an individual's captured data or recommendations (per [../research/04-ethical-analysis.md](../research/04-ethical-analysis.md)).

### 5.7 Accessibility and localization

- FR-23: Full screen-reader support, keyboard navigation, high-contrast theming, and scalable fonts across onboarding and the main app, meeting WCAG 2.1 AA as the Phase 4 testing bar.
- FR-24: UI string externalization from day one, even if only English ships initially, so localization is additive rather than a retrofit.

## 6. Non-functional requirements

| Category | Requirement |
|---|---|
| Performance | Idle CPU/RAM footprint must not be perceptible to the user on a mid-range 2020+ laptop; observation must never visibly block foreground application input |
| Security | Every requirement in [06-security-architecture.md](06-security-architecture.md) and [../research/06-threat-model.md](../research/06-threat-model.md) |
| Privacy | Every requirement in [05-privacy-model.md](05-privacy-model.md) |
| Reliability | No data loss across app update or crash; pipeline failures (e.g., a plugin crash) must degrade to reduced observation, never to silent over-collection |
| Portability | Windows 10+, macOS 12+ (Intel + Apple Silicon), major Linux distributions (Ubuntu, Fedora, Debian-family at minimum) |
| Maintainability | Clean Architecture layering (ADR-0002) enforced in CI; no module may bypass its declared ports |

## 7. Success metrics

- % of new users who receive at least one recommendation within 24 hours (target tied to G1).
- 30-day retention, segmented by whether the user received a recommendation they marked "useful" in the first week.
- % of users who ever change their privacy level after first-run (a proxy for whether the trust model is actually legible — both upgrades and downgrades are healthy signals; silence is not).
- % of recommendations acted on (implemented, in full or via the alternative suggested) vs. dismissed, with dismissal reason captured to improve future ranking.
- Zero critical findings in each Phase-4 security/privacy test pass (see [../research/06-threat-model.md](../research/06-threat-model.md)) prior to any release.

## 8. Explicit exclusions from this PRD (deferred, not rejected)

- A companion mobile or web app / cross-device sync — would require its own threat model and consent model (noted as out of scope in [../research/06-threat-model.md](../research/06-threat-model.md)); no sync feature ships until that work exists.
- Any team/aggregate view of any kind, at any privacy level, for any deployment mode (enterprise included) — see NG2. Revisiting this would require a distinct product decision and a fresh ethical review, not an incremental feature add.
- Marketplace/monetized third-party plugin distribution — the plugin *architecture* (ADR-0009, [08-plugin-architecture.md](08-plugin-architecture.md)) is in scope; a public plugin marketplace with review/payment processes is not.

## 9. Future consideration flagged, not committed

An opt-in "implement this for me" action that executes a specific, already-approved recommendation (e.g., actually creating the n8n flow it suggested) is plausible as a later feature and does not violate NG1, because it is per-action, user-initiated, and scoped to a recommendation the user already saw fully explained — as opposed to a standing execution agent. Any such feature requires its own capability-scoped permission grant and its own entry in the audit log, and should not be assumed into scope for the initial architecture without a dedicated design pass.
