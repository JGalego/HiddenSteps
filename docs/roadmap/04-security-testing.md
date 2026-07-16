# Security Testing Plan

Operationalizes the test hooks named in [../design/06-security-architecture.md](../design/06-security-architecture.md) §7 into a concrete, repeatable plan, scoped against the STRIDE threat model in [../research/06-threat-model.md](../research/06-threat-model.md).

## 1. Test suites

| Suite | Target | Method | Pass criterion |
|---|---|---|---|
| Redaction adversarial suite | Redaction Engine | Crafted inputs designed to evade secret/PII/PHI detection (split tokens, encoded secrets, homoglyphs, context-dependent PII) | Every evasion attempt either is redacted or triggers drop-on-uncertainty — never stored unredacted |
| Plugin capability-escape suite | WASM Plugin Host | A permanent "hostile plugin" fixture attempts every capability the plugin was *not* granted | Every attempt fails at the sandbox boundary (import not linked), not at an application-level check that could be bypassed |
| Key lifecycle failure-mode suite | Security Layer, SecretStore | Simulate: missing vault entry, corrupted DB header, wrong Portable-Mode passphrase, mid-rotation crash | Every failure mode fails closed (refuses to open/decrypt) with a clear user-facing error — never a silent plaintext fallback |
| Update signature suite | Updater | Corrupted signature, wrong signing key, replayed old signed payload, downgrade attempt | Updater refuses to apply in every case, with no partial-apply state left behind |
| Air-gapped egress suite | Every code path that can open a network connection | Attempt every such path with air-gapped mode enabled | Zero successful egress; the network-activity audit log shows zero attempts (or explicitly-blocked attempts, per the chosen implementation) |
| Encryption-at-rest verification | EventStore file | Attempt to open `hiddensteps.db` with a generic SQLite tool; attempt to read a backup copy with the vault key deleted | Both attempts fail; no plaintext table/column is ever readable without the key |
| Audit log integrity | Audit Log | Attempt to modify or selectively delete audit entries without triggering full delete-all | No partial-edit/partial-delete path exists |

## 2. External review

- **Independent security review** (third-party pentest or audit) is a release gate before any GA, not an optional nice-to-have — given the "small-vendor credibility gap" risk ([../research/03-risk-analysis.md](../research/03-risk-analysis.md)), a self-reported "we tested it" claim is weak evidence for exactly the audience most likely to need convincing (the privacy-conscious-professional persona).
- Scope for the external review: the plugin sandbox boundary (highest-leverage per the threat model), the encryption/key-management lifecycle, the update mechanism, and the redaction engine's real-world evasion resistance.
- If the core is open-sourced (recommended in [../design/04-trust-model.md](../design/04-trust-model.md) §4), a public responsible-disclosure process (security contact, disclosure timeline policy) should exist before GA.

## 3. Cadence

- Adversarial/capability-escape/key-lifecycle/update-signature/air-gapped suites: run in CI on every PR touching their respective module, plus fully on every release candidate.
- External review: before first GA, then annually or on any major architectural change to the plugin sandbox, encryption, or update mechanism.
- Redaction adversarial suite specifically should grow over time — every new evasion technique discovered (internally or externally reported) becomes a permanent new test case, never a one-off fix without a regression guard.

## 4. Explicit non-goals for this plan

- Does not cover a companion cloud/sync service's security, since none exists in the current architecture ([../design/01-prd.md](../design/01-prd.md) §8) — a future sync feature requires its own security test plan addendum.
- Does not cover physical/coercive-access scenarios (device seizure, compelled unlock) as a testable control — per [../research/06-threat-model.md](../research/06-threat-model.md)'s own scoping, this is a policy/data-minimization question, not something a test suite can pass/fail.
