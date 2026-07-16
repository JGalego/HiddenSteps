# Security Architecture

Implements PROMPT.md's Security requirements (encrypted DB/embeddings/caches/settings, secure key storage, OS vault integration, signed updates, audit logs, local-only/air-gapped/enterprise modes) under the zero-trust assumption from [../research/06-threat-model.md](../research/06-threat-model.md): no component is trusted merely because it's local.

## 1. Encryption at rest

| Asset | Mechanism | Key source |
|---|---|---|
| Primary store (`hiddensteps.db`: summaries, patterns, workflow graph, recommendations, embeddings, settings, audit log) | SQLCipher (AES-256, authenticated) — ADR-0003 | Master key from OS credential vault (ADR-0008) |
| Plugin-declared secrets (e.g., a cloud-provider API key entered by the user) | Stored directly in the OS credential vault, never in the SQLCipher DB or any config file | OS vault-native encryption |
| In-memory pipeline buffers (raw/redacted-pending content) | Not persisted; zeroize-on-drop buffer type; excluded from crash dumps/core dumps where the OS allows opting out (e.g., `madvise(MADV_DONTDUMP)`-equivalent on Linux, minidump exclusion on Windows) | N/A (never written) |
| Portable Mode data directory | Same SQLCipher mechanism, key derived via Argon2id from a user passphrase instead of the OS vault (ADR-0008) | User passphrase (disclosed tradeoff: loss of passphrase = loss of data) |

Authenticated encryption (not just confidentiality) is required for the primary store specifically to address the Tampering threats in [../research/06-threat-model.md](../research/06-threat-model.md) — a modified ciphertext must fail to decrypt/verify, not silently decrypt to corrupted plaintext.

## 2. Key management lifecycle

1. **First run**: a 256-bit master key is generated via a CSPRNG and written to the OS credential vault, scoped to the current OS user account. The key is never displayed, logged, or included in diagnostics output.
2. **Runtime**: the Security Layer retrieves the key from the vault once per app launch and holds it only in memory for the SQLCipher connection's lifetime; it is not cached to disk anywhere else.
3. **Rotation**: supported on demand from Settings (Advanced tier) — re-encrypts the store under a freshly generated key, then updates the vault entry; the old key is zeroized after successful re-encryption confirmation.
4. **Loss/recovery**: if the vault entry is lost (e.g., OS profile corruption) with no passphrase-recovery mode configured, the data is unrecoverable by design — this is disclosed at first run for the non-portable default, matching the "no memorable-password requirement by default" decision in ADR-0008.
5. **Portable Mode**: key derivation uses Argon2id with a per-installation random salt stored alongside (not instead of) the encrypted data directory; the passphrase itself is never stored anywhere.

## 3. Signed updates

- Every release artifact (installer, AppImage, portable bundle, and the update-delta payload for the auto-updater) is signed with a vendor key whose public counterpart ships embedded in the application binary.
- The updater verifies the signature **before** any update payload is applied, and refuses to proceed on verification failure — with a clear, non-dismissible error rather than a silent fallback to the unverified payload.
- Enterprise/offline update channels use the same signature scheme; an air-gapped deployment can verify a manually transferred update package using the same embedded public key, with no network call required for verification itself.

## 4. Audit log

- Append-only, stored in the same SQLCipher database (its own table — see [07-database-schema.md](07-database-schema.md)), covering: privacy-level changes, pause/resume, plugin capability grants/revocations, data exports, data deletions, provider changes, and update installs.
- Contains **action metadata only** — actor (always "user" or "system," since there is no multi-user concept), action type, timestamp, and relevant IDs (e.g., which plugin) — never captured content, per the Repudiation mitigation in the threat model.
- Readable from the Diagnostics view; exportable as part of the general data export; not independently deletable (deleting it requires the same delete-all operation that removes everything else, so a user can't selectively erase their own audit trail while keeping other data — preventing the audit log from being a tool for concealing a change from oneself later, which has limited value here but keeps the log's integrity story simple: it's whole or it's gone, never selectively edited).

## 5. Local-only / air-gapped / enterprise modes

| Mode | Behavior |
|---|---|
| Local-only | Cloud AI providers disabled entirely at the settings layer (not just unconfigured); update checks still permitted (user's choice) |
| Air-gapped | All network access blocked at the application layer (not relying on OS firewall alone) — the app itself refuses to open a socket for anything but loopback/IPC; updates and provider connectivity tests are no-ops; a **network-activity audit view** (per the threat model's air-gapped-mode-verifiability mitigation) shows zero outbound attempts, so the mode is verifiable, not just configured |
| Enterprise-managed | Policy file (signed, loaded at startup) sets privacy-level floors and provider allowlists per [05-privacy-model.md](05-privacy-model.md) §6; silent install and centrally distributed config supported per PROMPT.md's Enterprise Deployment requirements |

## 6. Plugin security

- WASM sandbox with structural capability enforcement (ADR-0009) is the primary control; capability grants are logged to the audit log.
- Plugin manifests are validated against a closed schema (see [08-plugin-architecture.md](08-plugin-architecture.md)) before installation; unrecognized capability requests are rejected outright rather than granted-by-default.
- First-party/in-tree plugins are held to the same manifest-declaration discipline as third-party ones (ADR-0009), so the security model doesn't have a silent "trusted by default" tier invisible to the user.

## 7. Security testing hooks (feeding Phase 4)

- Redaction-engine adversarial test suite (crafted inputs designed to evade secret/PII detection, per [05-privacy-model.md](05-privacy-model.md) §4).
- Plugin capability-escape test suite (attempt to call host functions outside a plugin's granted capability set; must fail at the sandbox boundary, not at an application-level check).
- Key-loss/recovery scenario tests (vault entry missing, corrupted DB, portable-mode passphrase mismatch) to confirm every failure mode fails safely (no silent plaintext fallback) rather than just failing.
- Update-signature-tampering tests (corrupted signature, wrong key, replayed old signed payload) to confirm the updater's refusal path.
- Air-gapped-mode network-egress tests (attempt every code path that could open a socket) to confirm the audit-visible zero-egress guarantee actually holds.
