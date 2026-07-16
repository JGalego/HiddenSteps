# Trust Model

This is distinct from the [privacy model](05-privacy-model.md) (what data is collected/retained) and the [threat model](../research/06-threat-model.md) (what an attacker could do). The trust model answers a narrower question: **how does a user verify, on an ongoing basis, that HiddenSteps is doing what it claims** — since per [../research/04-ethical-analysis.md](../research/04-ethical-analysis.md), this product is technologically identical to surveillance software except in who it serves, and "trust us" is not an adequate answer to that fact.

## 1. Trust actors and boundaries

| Actor | What they can see/do | What they explicitly cannot do |
|---|---|---|
| The observed user | Everything: all captured (post-redaction) data, all patterns, all recommendations, full audit log, full export | — |
| HiddenSteps (the software vendor) | Nothing, by architecture, once the app is running local-first with no telemetry — no default phone-home of captured content, patterns, or recommendations | See a specific user's data unless the user explicitly sends it (e.g., a bug report the user chooses to attach data to) |
| A cloud AI provider (if opted into) | Only the specific content sent for a specific completion/embedding call, per the dispatch-gating flow ([03](03-data-flow-diagrams.md) §5) | See any data outside what was explicitly sent for that call; see historical data retroactively |
| A third-party plugin author | Only what their plugin's granted capabilities allow it to touch, enforced by the WASM sandbox (ADR-0009) | Exceed declared/granted capabilities; see data from other plugins |
| An enterprise IT/policy administrator | Policy configuration (privacy-level floors, approved-provider allowlists, deployment settings) | See any individual's captured data, patterns, or recommendations, under any deployment mode (per NG2 in [01-prd.md](01-prd.md)) |
| A co-user of a shared machine (different OS account) | Nothing — data is OS-user-scoped via the credential vault (ADR-0008) | Access another OS user's HiddenSteps data without that user's OS credentials |

## 2. Trust features (verifiability, not just disclosure)

Per PROMPT.md's Trust Features requirement, each of these must be **continuously and immediately** checkable, not a one-time onboarding disclosure:

| Feature | What it proves | Where it lives |
|---|---|---|
| Observation status indicator | Whether anything is being captured right now | Persistent UI element (not buried in settings) |
| Current privacy level | Exactly which signal types are in scope right now | Privacy dashboard |
| Current AI provider + model | Whether any data could leave the device right now, and to whom | Privacy dashboard |
| Recent captured events feed | What was *actually* captured in the last N minutes/hours, post-redaction | Privacy dashboard, real-time |
| Pause observation | Immediate, one action, no confirmation maze | Persistent UI element |
| Delete observations (selective or all) | That a specific capture (or everything) is gone, verifiably (§4 of [03](03-data-flow-diagrams.md)) | Privacy dashboard |
| Export data | That the user can take their data and leave, in full, at any time | Privacy dashboard |
| Audit log | A durable record of every consent-relevant action (level changes, pause/resume, deletions, exports) the app itself has taken | Diagnostics/Settings |
| "Why was this suggested?" on every recommendation | That a recommendation traces to real, inspectable observations, not an opaque model guess | Recommendation detail view |

The **recent captured events feed** is the single highest-leverage trust feature: it turns "the privacy level says it only collects metadata" from a claim into something the user can watch happen in real time and catch a violation of immediately. It must show what was captured *after* redaction, i.e., exactly what will be retained — not a sanitized marketing summary of the privacy level.

## 3. Trust is asymmetric and front-loaded — design accordingly

A new user has no basis yet to trust HiddenSteps beyond its onboarding claims (§1 of [03](03-data-flow-diagrams.md)). Trust-model consequences:

- Default to the **least invasive privacy level compatible with any value at all** at first run, and let demonstrated value (a good first recommendation) earn the case for the user to *choose* to raise their privacy level — never the other way around (nudging toward more invasive defaults to produce a flashier demo, which [../research/04-ethical-analysis.md](../research/04-ethical-analysis.md) explicitly flags as a dark pattern to avoid).
- Every OS permission prompt must be preceded by an in-app explanation of why, shown before the OS dialog appears — an unexplained OS permission dialog is a trust-destroying surprise regardless of how legitimate the underlying need is.
- The gap between "install" and "first value" (Zero-to-Value, PROMPT.md) is also the gap in which a wary user decides whether to keep observation on at all — this is a trust deadline, not just a UX metric.

## 4. Independent verifiability as a trust multiplier

Because HiddenSteps is a new, unaudited vendor asking for sensitive access (per the "small-vendor credibility gap" risk in [../research/03-risk-analysis.md](../research/03-risk-analysis.md)), self-reported trust claims are weaker evidence than independently checkable ones. Architecture choices that materially strengthen trust beyond "we say so":

- **Open, inspectable local storage** (a documented schema, [07-database-schema.md](07-database-schema.md)) means a technical user (or a third-party auditor) can independently confirm what's actually stored, not just what the UI claims is stored.
- **Open-sourcing the core** (recommended, though a business decision outside this architecture's scope) would let privacy claims be verified by inspection rather than taken on faith — the single strongest lever available for a small vendor's credibility gap.
- **The WASM plugin capability model** (ADR-0009) is itself independently testable: a security researcher can verify a plugin's granted capability set matches its manifest without trusting vendor claims about "sandboxing."
- **No default telemetry** — if any anonymous usage telemetry is ever added, it must be opt-in, itself shown in the privacy dashboard as a distinct data category, and documented with the same collected/retained/transmitted rigor as observation data ([05-privacy-model.md](05-privacy-model.md)).

## 5. Trust failure modes to design against

- **Silent scope creep**: a feature update that expands what's captured at an existing privacy level without the user re-consenting. Mitigation: privacy-level manifests are versioned; any update that changes what a given level captures requires a re-consent prompt on upgrade, not a changelog footnote.
- **Recommendation-as-surveillance-artifact**: a recommendation that, if it left the app (screenshotted, exported, shown to a manager), would function as a productivity-monitoring report. Mitigation: recommendations are framed and worded as personal leverage ("here's time you could reclaim"), never as an audit ("here's how this employee spends their time") — a wording and framing discipline that the Recommendation Engine's prompt templates must enforce, not just a style guideline.
- **Enterprise policy quietly weakening user-facing trust guarantees**: a policy pack that, say, disables the recent-captured-events feed "for performance." Mitigation: the trust features in §2 are treated as non-configurable by enterprise policy — policy can restrict *what's collected*, never *what's shown to the observed user about what's collected*.
