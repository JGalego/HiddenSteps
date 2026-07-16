# Privacy Testing Plan

Verifies the enforceable guarantees in [../design/05-privacy-model.md](../design/05-privacy-model.md) actually hold in a real build — distinct from [04-security-testing.md](04-security-testing.md), which asks "can an attacker get in"; this asks "does the product collect, retain, and transmit exactly what it claims to, no more."

## 1. Per-level signal-boundary tests

For each privacy level 0-4, run a scripted session generating a known set of activities (opening specific apps, visiting specific domains, copying specific clipboard content, editing specific files) and assert:

- Only the exact signals enumerated for that level in [../design/05-privacy-model.md](../design/05-privacy-model.md) §1 appear in `event_summaries`.
- No signal from a *higher* level ever appears (e.g., at Level 2, confirm no OCR/screenshot/accessibility-tree content exists anywhere in the database — not just "isn't shown in the UI").
- The recent-events feed shown in the UI ([../ux/03-privacy-dashboard.md](../ux/03-privacy-dashboard.md)) matches what's actually stored, field for field — this specifically tests the trust-model claim that the feed shows real stored data, not a sanitized preview.

## 2. Retention and deletion tests

- Deep-mode (Level 4) excerpts past their TTL are confirmed gone from the database after the sweep job runs — not just excluded from queries, but actually removed (verified by direct file inspection where feasible, or by confirming no recovery is possible via any exposed API).
- `delete_events`/`delete_all_data` are tested for completeness: after a full delete-all, confirm zero rows in every table (per [../design/07-database-schema.md](../design/07-database-schema.md)), zero vault key entries, and — for Portable Mode — zero recoverable bytes in the deleted directory via basic forensic inspection (e.g., `strings` against the freed disk region isn't expected to be perfectly clean given filesystem behavior, but the encrypted file itself must already be unreadable once the key is gone, which is the actual guarantee being made, not "forensically pristine deletion").
- Export completeness: confirm an exported archive actually contains everything the user is entitled to (cross-check against a full DB dump in a test environment) — an incomplete export would itself be a privacy-model violation (the "take your data and leave" trust feature).

## 3. Redaction correctness (privacy angle, distinct from the adversarial security angle in 04)

- Positive tests: known secret/PII-shaped test strings are reliably redacted or cause a drop, across all signal types that could carry them (window titles, clipboard metadata edge cases, Deep-mode OCR text).
- False-positive-tolerance tests: confirm the system is *biased toward dropping* on uncertainty rather than storing partially redacted, per the explicit policy in [../design/05-privacy-model.md](../design/05-privacy-model.md) §4 — a test that induces classifier uncertainty should observe a dropped event, not a half-redacted one.

## 4. Cloud-dispatch gating tests

- For each defined cloud-eligibility tier ([../design/05-privacy-model.md](../design/05-privacy-model.md) §3): confirm content below the threshold reaches a configured cloud provider only after the appropriate consent step, and confirm Level-4-derived content is **never** dispatched to a cloud provider under any configuration — including attempting to misconfigure the system into doing so, to confirm the rule is structurally enforced ([../design/adr/0004-llm-provider-trait-local-first.md](../design/adr/0004-llm-provider-trait-local-first.md)) and not just a default that a bug could bypass.
- Confirm the "what will be sent" disclosure ([../design/05-privacy-model.md](../design/05-privacy-model.md) end, and [../design/03-data-flow-diagrams.md](../design/03-data-flow-diagrams.md) §5) accurately previews the actual outbound payload — a mismatch here (disclosure says X, payload contains Y) is itself a critical finding regardless of whether Y was otherwise permissible.

## 5. Consent-versioning tests

- Simulate a privacy-level manifest version bump (per [../design/05-privacy-model.md](../design/05-privacy-model.md) §5) and confirm affected users see a re-consent prompt describing the actual delta before the new manifest takes effect — and confirm observation does *not* silently expand in the interim.

## 6. Enterprise policy boundary tests

- Confirm an enterprise policy file cannot, through any combination of settings, achieve any of the excluded actions listed in [../design/05-privacy-model.md](../design/05-privacy-model.md) §6 (lower redaction threshold, disable the Level-4 cloud rule, hide a trust feature, extend retention/disable deletion) — attempt each directly via a maximally adversarial policy file, not just confirm the documented schema lacks the field.

## 7. Cadence and ownership

- Per-level signal-boundary and cloud-dispatch tests run in CI on every PR touching the pipeline, privacy engine, or provider layer.
- Retention/deletion and enterprise-policy-boundary tests run pre-release as part of the full regression pass, since they exercise slower, more end-to-end paths.
- Any confirmed over-collection or under-redaction finding is treated as a release blocker, not a follow-up ticket — this class of bug is the single fastest way to invalidate every trust claim in [../design/04-trust-model.md](../design/04-trust-model.md).
