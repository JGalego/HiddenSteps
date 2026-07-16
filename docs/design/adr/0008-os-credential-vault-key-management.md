# ADR-0008: OS credential vault as the sole root of key material

Status: Accepted

## Context

PROMPT.md requires secure key storage and OS credential vault integration. ADR-0003's SQLCipher store and any cloud-provider API keys need a master key/secret that must never live in a plaintext config file, must survive app updates, and (per the threat model's Information Disclosure section) must not be recoverable from a disk/backup image alone.

## Decision

Use the `keyring` crate (Keychain on macOS, Credential Manager/DPAPI on Windows, Secret Service/libsecret on Linux) as the **sole** root of trust for: (1) the SQLCipher database master key, (2) any cloud AI provider API keys, (3) any plugin-declared secrets. The database master key is generated randomly at first run, stored only in the OS vault, and never derived from a user-memorable password by default (removing "forgot my password, lost my data" as a routine failure mode) — with an explicit, clearly-labeled **optional** passphrase-recovery mode for users who want portability of the key itself (e.g., moving Portable Mode data between machines without vault access), accepting the weaker guarantees that implies.

## Consequences

- A stolen disk image or unencrypted backup of `hiddensteps.db` alone is not sufficient to decrypt it — the key lives in a separate, OS-protected store, directly addressing the "full-disk or backup compromise" threat.
- Per-OS-user vault scoping gives multi-user-machine isolation "for free" (each OS user's vault is separate), addressing the shared-device threat in the threat model without extra product-level access-control code.
- Portable Mode (PROMPT.md) is the one case where "no OS vault available" is expected by design (running off a USB drive, possibly on an unfamiliar machine) — for that mode, the key is derived from a user-supplied passphrase via a memory-hard KDF (Argon2id) and never persisted outside the portable data directory, with onboarding explicitly warning that losing the passphrase means losing the data (an accepted, disclosed tradeoff, not a hidden one).
- Enterprise/air-gapped deployments can pre-provision the vault entry via policy tooling (PROMPT.md's Enterprise Deployment) rather than requiring an interactive first-run key generation step.

## Alternatives considered

- **App-bundled static key or key derived solely from machine ID**: rejected — trivially extractable by anyone with code or filesystem access, defeating the point of encryption at rest.
- **Mandatory user passphrase for the primary (non-portable) mode**: rejected as the default — adds friction against PROMPT.md's "under five minutes" onboarding goal for a threat (this exact machine's OS vault being compromised) that, if realized, usually also compromises a typed passphrase via keylogging; offered as opt-in hardening instead, not the default.
