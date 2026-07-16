# Privacy Analysis

## Challenging the premise, per PROMPT.md

Before accepting "observe continuously, process later" as the architecture, each of PROMPT.md's challenge questions gets a real answer:

**Is there a less invasive way?** Often yes. Much of the signal needed to detect a repeated workflow (same app sequence, same window titles, same rough timing, same clipboard-content *shape* rather than content) doesn't require screen content at all. Minimal/Standard modes should be able to detect the large majority of "quick win" patterns (copy-paste loops, repeated file operations, repetitive browser navigation) from metadata alone — screenshots/OCR/accessibility-tree inspection (Deep mode) should be reserved for cases metadata genuinely can't resolve, and should be a deliberate escalation, not a default.

**Can the same insight be achieved with less data?** Yes, in most cases — see the data pipeline below. A detected pattern like "this clipboard→paste→save sequence recurs" doesn't need the actual clipboard contents retained once the *shape* of the pattern is captured; only during initial classification does more detail help, and that need ends quickly.

**Can more processing happen locally?** This should be the default answer for everything except the parts of the recommendation-explanation step where a user has explicitly chosen a cloud model. Pattern detection, redaction, classification, and embedding are all computationally tractable locally today and should never require network access to run.

**Can information be summarized before storage?** Yes — this is the central architectural commitment (see pipeline below): raw observations are ephemeral inputs, not the retained asset.

**Can raw data be discarded immediately?** For most signal types, yes, once a summary/embedding is derived — there is no legitimate long-term use for the raw event stream once the abstraction is extracted, and retaining it only expands blast radius on compromise (see [06-threat-model.md](06-threat-model.md)) with no offsetting product benefit.

**Can user trust be increased?** Yes, primarily through legibility: visible observation status, a real-time feed of what was just captured, one-click pause/delete, and privacy-dashboard transparency (all already specified in PROMPT.md) turn "trust us" into "verify us."

## The core question, restated as a design constraint

> How can software observe work well enough to provide genuinely useful workflow recommendations without becoming surveillance software?

Operationally, this decomposes into three testable properties the architecture must satisfy:

1. **Proportionality** — the amount and sensitivity of data collected at each privacy level should be the *minimum* sufficient to produce that level's promised insight quality, not the maximum available.
2. **Ephemerality** — raw observational data has a short, bounded lifetime by default; what persists is a derived abstraction (a detected pattern, an embedding, a summary), not the underlying screen/keystroke/clipboard stream.
3. **Exclusivity of benefit** — the only party who benefits from retained data is the observed individual; there is no path (feature, export format, sync mechanism) by which retained data primarily serves an employer, HiddenSteps the company, or a third party.

## Recommended data pipeline (elaborating PROMPT.md's Capture → Classify → Redact → Summarize → Embed → Delete raw → Retain abstractions)

| Stage | What happens | Retention after this stage |
|---|---|---|
| Capture | Raw signal collected per the active observation mode (app/window metadata; or + browser domain/clipboard metadata/file ops; or + OCR/screenshot/accessibility tree, opt-in only) | In-memory / very short-lived on-disk buffer only |
| Classify | Signal is tagged: sensitive-category candidate? part of a known pattern? novel? | Same buffer |
| Redact | Automatic detection and stripping of passwords, API keys, tokens, secrets, PII, PHI, financial identifiers (per PROMPT.md's Sensitive Information Protection) — applied *before* anything touches durable storage | Same buffer, now redacted |
| Summarize | Convert to the minimum durable representation: "app sequence X→Y→Z repeated," not the literal window contents | Durable store (encrypted) |
| Embed | Vector representation of the *summary*, not raw content, for similarity/pattern search across time | Durable store (encrypted) |
| Delete raw | Buffer is purged | — |
| Retain | Only summaries, embeddings, and derived recommendation-relevant abstractions persist | Durable store (encrypted), user-deletable at any time |

The key architectural discipline this implies: **raw capture must never be allowed to reach durable, unencrypted, or long-lived storage as an intermediate convenience** (e.g., "we'll just cache screenshots for a day for debugging") — every exception to immediate ephemerality is a proportionality violation waiting to be found in an audit or a breach.

## Privacy levels — collected vs. retained vs. transmitted

| Level | Collected | Retained (post-pipeline) | Transmitted off-device |
|---|---|---|---|
| 0 — Manual | Nothing | Nothing | Nothing |
| 1 — Application metadata | Active app, window titles, timing, frequency | Pattern summaries only | Nothing, unless cloud AI provider explicitly chosen for recommendation synthesis |
| 2 — Workflow metadata | + application actions, browser domains (not full URLs/content), clipboard metadata (size/type/frequency, not content), file operation metadata | Pattern + workflow-sequence summaries | Same as above |
| 3 — Context-aware | + richer in-app action context, fuller browser/file context | Summaries + limited context snippets, redacted | Same as above; user should be warned that richer context increases what could be sent to a cloud provider, if chosen |
| 4 — Maximum assistance | + explicit opt-in OCR/screenshots/accessibility trees, aggressively redacted at capture time | Summaries + heavily redacted excerpts, never raw screenshots retained by default | Explicit, separate consent required before any Level 4 content is sent to any cloud provider, regardless of the user's general provider choice |

Each level's onboarding screen (per PROMPT.md's First Run Experience) must state, in the collected/retained/transmitted framing above, exactly what's true at that level — not a generic privacy statement.

## Sensitive-information detection as a proportionality mechanism, not just a safety net

PROMPT.md requires automatic detection of passwords/API keys/tokens/secrets/PII/PHI/financial data. This detection has two roles: it's the last line of defense against a Level-3/4 capture accidentally retaining something sensitive, but more importantly, its *false-positive-tolerant* design should bias toward **dropping the observation entirely** rather than attempting partial redaction when the classifier is uncertain — for this product, silently under-collecting is the safe failure mode, not silently under-redacting.

## Local vs. cloud AI and privacy

Local-first AI (Ollama/LM Studio/llama.cpp/LocalAI/vLLM) is privacy-load-bearing, not just an architecture preference: it's the only way to guarantee that Level 1–3 data never leaves the device even during the recommendation-synthesis step. Cloud providers (OpenAI, Anthropic, etc.) must be opt-in per PROMPT.md, and the UI should make unmistakably clear, at the moment a cloud provider is selected, *which* captured data (if any) will be sent to it for a given recommendation — not just a one-time setup-screen disclosure.

## Data at rest

Everything durable — database, embeddings, caches, settings — must be encrypted with keys held in the OS credential vault (Keychain/DPAPI/libsecret), never in a plaintext config file. See [06-threat-model.md](06-threat-model.md) for the corresponding threat analysis.
