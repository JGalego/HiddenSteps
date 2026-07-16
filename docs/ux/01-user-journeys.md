# User Journeys

Five journeys covering the personas in [../design/01-prd.md](../design/01-prd.md) §4, from first launch through ongoing use. Each maps to specific FRs and trust/privacy guarantees established in Phase 2.

## Journey 1 — First 24 hours (Zero-to-Value)

**Persona:** Overloaded individual contributor. **Goal:** install, understand what's happening, get one genuinely useful insight before they'd otherwise have given up on a new tool.

1. Downloads and installs (under 5 minutes, FR-20) → launches app.
2. First-run flow (detailed in [02-onboarding-flow.md](02-onboarding-flow.md)): what it does/doesn't do → permissions explained → privacy level chosen (defaults to the least-invasive option that still produces value, per [../design/04-trust-model.md](../design/04-trust-model.md) §3) → AI provider chosen (local auto-detected or cloud) → explicit consent → observation starts.
3. Works normally for the rest of the day. The persistent status indicator is visible but unobtrusive.
4. Within 24 hours, the deterministic Pattern Detection layer ([../design/adr/0010-hybrid-deterministic-plus-llm-recommendation-engine.md](../design/adr/0010-hybrid-deterministic-plus-llm-recommendation-engine.md)) finds a qualifying repeated pattern (e.g., a copy-paste-between-two-apps loop) and a recommendation is generated.
5. A non-intrusive notification surfaces the first recommendation (see [04-recommendations-ux.md](04-recommendations-ux.md)) — framed as a discovery, not an alert: *"You've done this 6 times today — here's a 2-minute fix."*
6. User reviews the recommendation, sees the why/confidence/alternatives, tries the suggested keyboard shortcut or template, marks it "useful."
7. **Success condition (G1):** the user's very first substantive interaction with the product's *judgment* (not just its onboarding) is a correct, low-risk, immediately actionable insight — the moment retention is decided.

**Failure mode to design against:** if no qualifying pattern exists yet by hour 24, the dashboard says so honestly ("still learning your patterns — nothing repeated enough yet to suggest") rather than manufacturing a low-value suggestion just to hit the 24-hour mark (FR-12).

## Journey 2 — Daily ambient use and trust verification

**Persona:** Privacy-conscious professional. **Goal:** confirm, on their own terms, that the tool is doing only what it claims — this journey is really about the privacy dashboard, not about recommendations.

1. Opens the privacy dashboard mid-day (accessible in one click/shortcut from anywhere, per [03-privacy-dashboard.md](03-privacy-dashboard.md)).
2. Sees: observation active, current level (e.g., Level 2), current AI provider (e.g., "Ollama — local, offline"), and the live recent-events feed showing exactly what's been captured in the last hour.
3. Scans the feed, confirms it matches their mental model of what Level 2 should capture (domains, not full URLs; metadata, not clipboard content) — this is the moment trust is either reinforced or broken.
4. Notices they're about to work in an EHR-adjacent tool; the app proactively suggests an exclusion rule or a temporary drop to Level 0 for that application (per the ethical-analysis mitigation, [../research/04-ethical-analysis.md](../research/04-ethical-analysis.md)).
5. Accepts the suggestion; the exclusion takes effect immediately, confirmed in the feed (no more events from that app).

## Journey 3 — Recommendation review, implementation, and feedback

**Persona:** Technical power user. **Goal:** get real leverage from a detected pattern, not just an insight — and give feedback that improves future ranking.

1. Opens the Recommendations view, sees a card: *"Observed 31 times over 2 weeks. Estimated 11 hours/month."* with a recommended approach (e.g., "Hybrid: Playwright + local LLM") and five alternatives ranked by difficulty/maintenance tradeoff.
2. Expands "why was this suggested?" → sees the exact contributing observations (FR-13), the assumptions made, and what was intentionally ignored (e.g., "ignored occurrences on your personal laptop — cross-device correlation isn't supported yet").
3. Decides to implement it themselves outside the app (writes the Playwright script), returns and marks the recommendation "implemented."
4. The pattern's future occurrences are tracked separately (post-implementation) so the user can later see whether the fix actually reduced the pattern's frequency — closing the loop from insight to measured outcome.

## Journey 4 — Enterprise-managed deployment

**Persona:** IT/security admin. **Goal:** deploy HiddenSteps org-wide with policy guardrails, without gaining any visibility into individual employees' data (a hard architectural constraint, not just a policy promise — [../design/05-privacy-model.md](../design/05-privacy-model.md) §6).

1. Admin authors a policy file: privacy-level floor (e.g., "no lower than Level 1 if the tool is used at all"), approved-provider allowlist (e.g., "only the org's self-hosted Ollama endpoint"), silent-install flags.
2. Distributes via existing enterprise software-deployment tooling (SCCM, Jamf, etc.) alongside the signed installer.
3. Each employee, on first launch, still goes through the full first-run consent flow (Journey 1) — the policy constrains *choices available*, not consent itself (per [../design/01-prd.md](../design/01-prd.md) NG2).
4. Admin can confirm the policy is in effect via their own tooling's install-success reporting, but has no path — by design — into any employee's captured data, patterns, or recommendations.

## Journey 5 — Portable Mode on a USB drive

**Persona:** Freelancer/consultant working across multiple client machines, or anyone wanting zero footprint. **Goal:** use HiddenSteps without installing anything or leaving traces on a machine they don't own.

1. Runs the portable executable directly from a USB drive; no installer, no admin rights required.
2. First-run flow is identical to Journey 1, except the key-management step (per [../design/adr/0008-os-credential-vault-key-management.md](../design/adr/0008-os-credential-vault-key-management.md)) asks for a passphrase instead of using the (unfamiliar, not-theirs) machine's OS vault, with an explicit, plain-language warning that losing the passphrase means losing the data.
3. All data — including the SQLCipher file — lives under the portable directory on the USB drive; nothing is written to the host machine.
4. Ejects the drive at end of day; deleting the portable directory later (from any machine) leaves nothing recoverable, satisfying "leaves no traces after deletion" (FR-20).
