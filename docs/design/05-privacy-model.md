# Privacy Model (Specification)

This formalizes [../research/05-privacy-analysis.md](../research/05-privacy-analysis.md) into an enforceable spec: exact signal lists per level, retention rules, and the enforcement points responsible for each guarantee. Where this doc and the Phase 1 analysis differ, this doc is authoritative for implementation.

## 1. Privacy levels — exact signal enumeration

| Level | Name | Signals captured (exact) | OS permission tier required |
|---|---|---|---|
| 0 | Manual | None. No `ObservationSource` plugin is active. | None |
| 1 | Application metadata | Active application identifier; window title (title text only, not content); window focus-change timestamps; keyboard-shortcut invocation (which shortcut, not what it produced) | Minimal (process/window enumeration — no Accessibility/Automation grant) |
| 2 | Workflow metadata | Level 1 + application-exposed action events (e.g., menu item invoked, if the app exposes this) + browser **domain** only (not path, query, or page content) + clipboard **metadata** (content type, size, timestamp — never content) + file operation metadata (path, operation type, timestamp — never file content) | Moderate (browser-extension-level domain access; filesystem-event API access) |
| 3 | Context-aware | Level 2 + fuller in-app action context (e.g., which field/control within a supported application) + browser page **title** (still not full URL/query/content) + limited file-operation context (e.g., file type/extension in addition to path) | Elevated (Accessibility/UI Automation read access, scoped to structure, not content rendering) |
| 4 | Maximum assistance | Level 3 + explicit, separately-opted-in OCR / periodic screenshot / accessibility-tree content capture, each independently toggleable, each redacted at capture time before any other pipeline stage | Full (Accessibility API + Screen Recording permission) |

Escalating a level requires the onboarding-style permission explanation (per [04-trust-model.md](04-trust-model.md) §3) for any newly required OS permission tier before the corresponding plugins activate — this is enforced in the Privacy Engine (`change_privacy_level` refuses to activate a plugin whose required permission tier hasn't been separately acknowledged).

## 2. Retention rules

| Data class | Retention | Enforcement point |
|---|---|---|
| Raw `CapturedSignal` (pre-redaction) | In-memory only; never serialized; discarded immediately after Redact stage completes or the event is dropped | Type system (ADR-0006: `CapturedSignal` has no `Serialize` impl, no `EventStore` API accepts it) |
| Redacted-but-not-yet-summarized intermediate content | In-memory only, bounded ring buffer, discarded after Summarize completes | Pipeline worker lifecycle |
| Event summaries, pattern records, embeddings, recommendations, settings, audit log | Persistent until user deletes (no automatic expiry by default; see below for the one exception) | `EventStore` (SQLCipher) |
| Deep-mode (Level 4) redacted excerpts | Persistent but subject to a **default 90-day rolling TTL** (configurable, including "keep forever" or "keep 0 days / never persist excerpts"), because Level 4 content carries materially higher re-identification/sensitivity risk even after redaction | `EventStore` TTL sweep job, run at idle time |
| Audit log | Persistent, append-only, never auto-deleted except by full delete-all | `Security Layer` |

There is no automatic expiry for Levels 1–3 summaries by default because they are, by construction, low-sensitivity abstractions (app names, domains, timing) whose long-term value (multi-month pattern detection) outweighs their minimal residual risk; the Level 4 TTL exists because that tier's content, even redacted, warrants a materially more conservative default. Users may configure shorter or longer retention windows for any tier from Settings.

## 3. Cloud-eligibility threshold

"Cloud-eligible" (referenced in [03-data-flow-diagrams.md](03-data-flow-diagrams.md) §5) is defined per content type, not per privacy level as a whole:

- **Cloud-eligible by default** (still requires general cloud-provider opt-in, but no further per-call consent): Level 1–2 derived pattern summaries (app sequences, timing, frequency) with no domain/file-path/window-title strings included verbatim — i.e., the *shape* of a pattern.
- **Requires separate, explicit per-class consent**: Level 2–3 content that includes verbatim strings (a domain name, a window title, a file path), because these can be identifying or sensitive even after redaction of secrets/PII.
- **Never cloud-eligible, regardless of consent**: any Level 4 (Deep-mode) raw-derived excerpt (OCR text, screenshot-derived content, accessibility-tree content). Level 4 content may only be processed by a local (`is_local() == true`) provider. This is a hard architectural rule, not a configurable setting — removing it would require a deliberate future ADR superseding this one, not a settings toggle.

## 4. Redaction guarantees

- Redaction runs before Summarize, on every event, regardless of privacy level (even Level 1 metadata is checked — a window title can itself leak a secret, e.g. a terminal title containing a token).
- Detection categories (minimum): passwords, API keys/tokens, secrets (high-entropy strings in known secret-shaped contexts), credit-card/financial-account-number patterns, government ID patterns, medical-terminology patterns in specific contexts (e.g., EHR application detection combined with content, at Level 4 only since lower levels don't capture content).
- **Drop-on-uncertainty**: if the redaction classifier's confidence is below a configured threshold, the event is dropped entirely rather than stored partially redacted (ADR-0006). This threshold is tunable but defaults conservative (biased toward dropping); lowering it (accepting more risk for more retained data) requires an explicit, separately-surfaced Advanced-tier setting, never a default.
- Redaction quality is a Phase 4 testing requirement (adversarial red-team test suite against the redaction engine, per [../research/06-threat-model.md](../research/06-threat-model.md)) — this spec does not certify a specific detection accuracy, only the drop-on-uncertainty policy and the requirement that it be tested.

## 5. Consent versioning

Each privacy level's exact signal manifest (§1) is versioned. If a future release changes what a level captures (adds a signal type, changes a detail level), the affected level is bumped to a new manifest version, and any user currently on that level sees a re-consent prompt describing exactly what changed before the new manifest takes effect — the update does not silently apply. This directly implements the "silent scope creep" mitigation in [04-trust-model.md](04-trust-model.md) §5.

## 6. Enterprise policy interaction

An enterprise policy pack may set a **privacy-level floor** (the minimum level a device must run, if any observation is mandated at all — though per [01-prd.md](01-prd.md) NG2/[../research/04-ethical-analysis.md](../research/04-ethical-analysis.md), HiddenSteps' architecture assumes individual consent regardless of employer deployment) and an **approved-provider allowlist**. Policy cannot: lower the redaction confidence threshold, disable the Level-4-never-cloud-eligible rule, disable any trust feature in [04-trust-model.md](04-trust-model.md) §2, or extend retention/disable deletion. These are hard-coded exclusions from the policy schema (see [08-plugin-architecture.md](08-plugin-architecture.md) for the policy-loading mechanism), not conventions the policy loader happens to respect.
