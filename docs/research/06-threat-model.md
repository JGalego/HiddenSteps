# Threat Model

Scope: the observation pipeline (capture → classify → redact → summarize → embed → delete raw → retain), the local data store (encrypted DB, embeddings, caches, settings), the AI provider layer (local and cloud), and the plugin framework. Assumes zero trust per PROMPT.md's Security section: no component is trusted merely because it's local, and no user is assumed to be safe from a compromised machine, a malicious plugin, or a coerced/curious co-user of a shared device.

Primary asset: **the accumulated record of how a specific person works** — which is, by construction, one of the richest single-target caches of behavioral, contextual, and potentially sensitive (PII/PHI/credential/trade-secret) data an attacker could compromise on that machine.

## STRIDE analysis

### Spoofing

| Threat | Detail | Mitigation |
|---|---|---|
| Malicious/spoofed observation plugin | The plugin framework is a large attack surface — a third-party observation plugin could masquerade as legitimate and exfiltrate far more than it claims to capture. | Signed plugins; explicit, narrow, declared-capability manifests per plugin (what signal types it may access); runtime sandboxing/permission enforcement, not just manifest trust. |
| Spoofed AI provider endpoint | A cloud-provider integration pointed at an attacker-controlled endpoint (via a modified config or MITM) could receive sensitive summaries/embeddings believing it's the real provider. | Certificate pinning or strict TLS validation for cloud provider calls; endpoint configuration protected by the same encryption/vault mechanism as other secrets, not a plaintext settings file. |
| Fake update package | Unsigned/spoofed update could plant a malicious build that captures everything. | Mandatory signed updates (already a PROMPT.md requirement) with signature verification enforced, not optional. |

### Tampering

| Threat | Detail | Mitigation |
|---|---|---|
| Local database/embedding tampering | An attacker with disk access modifies retained summaries/recommendations (e.g., to inject a malicious "recommended script" that looks like it came from HiddenSteps' own engine). | Encrypt at rest with authenticated encryption (integrity, not just confidentiality); recommendation content that suggests executable code should be treated as untrusted-until-user-reviewed regardless of storage integrity. |
| Redaction-engine bypass via crafted input | Content deliberately or accidentally formatted to evade the secret/PII detector (e.g., an API key split across two clipboard events) reaches durable storage unredacted. | Redaction should run on reassembled/contextualized content where feasible, not just isolated events; treat detector as probabilistic and bias toward drop-on-uncertainty (see [05](05-privacy-analysis.md)); periodic red-team testing of the redaction engine as part of the security testing plan (Phase 4). |
| Tampered plugin altering captured data before classification | A compromised observation plugin could feed manipulated data into the pipeline to hide sensitive-data leakage from the redaction stage. | Plugin sandboxing with a well-defined, minimal data-passing interface; redaction/classification should not blindly trust plugin-provided sensitivity tags. |

### Repudiation

| Threat | Detail | Mitigation |
|---|---|---|
| No record of what was observed, when, or what was deleted | Without an audit log, a user (or an investigator, in a dispute) can't verify what HiddenSteps actually did — undermining the trust model itself. | Local, tamper-evident audit log of privacy-level changes, pause/resume events, deletions, and exports (already a PROMPT.md requirement) — this log itself must not contain sensitive captured content, only metadata about actions taken. |

### Information Disclosure — the dominant category for this product

| Threat | Detail | Mitigation |
|---|---|---|
| Full-disk or backup compromise exposing the entire behavioral history | The encrypted local DB, if backed up unencrypted (cloud backup tools, disk images) or if the encryption key is weak/recoverable, becomes a single high-value breach target. | OS credential-vault-backed key storage (never a static app-bundled key); encryption at rest for embeddings/caches/settings, not just the primary DB; explicit guidance/detection for whether OS-level backup tools are capturing the encrypted store's key material alongside the data. |
| Shared-device / multi-user exposure | On a shared or family/work-shared machine, another OS user account or a co-worker with physical access could attempt to read another user's HiddenSteps data. | Per-OS-user data isolation using existing OS user-separation and credential-vault scoping (data keyed to the OS user, not just "the machine"); no cross-user aggregate view, ever. |
| Cloud AI provider leakage | Any data sent to a cloud provider for recommendation synthesis is, from that point on, outside HiddenSteps' security boundary and subject to that provider's own retention/training-use policies. | Minimize what's sent (summaries/patterns, not raw context, wherever the model task allows); surface exactly what will be sent before sending it (per [05](05-privacy-analysis.md)); default to local providers. |
| Screenshot/OCR content leaking through crash reports, temp files, or memory dumps | Deep-mode captures are the most sensitive data type in the system; if they touch a crash-reporting pipeline, swap file, or temp directory unencrypted, redaction/ephemerality guarantees are silently broken. | Deep-mode buffers should be memory-only where feasible, explicitly excluded from crash/telemetry reporting, and securely wiped (not just unlinked) after use. |
| Enterprise policy / air-gapped mode misconfiguration | An enterprise deployment intended to be local-only could be misconfigured to phone home (telemetry, update checks, license validation) — a especially damaging failure for a product whose entire pitch is local-first. | Air-gapped mode should be enforceable and *verifiable* (e.g., a network-activity audit view), not just a settings toggle the app can silently ignore. |

### Denial of Service

| Threat | Detail | Mitigation |
|---|---|---|
| Observation pipeline resource exhaustion | A malicious or buggy plugin, or an unbounded capture loop, consumes CPU/disk/battery to the point the product becomes unusable (or the user disables observation out of frustration, which is itself a security-relevant outcome — an unused security control). | Resource budgets/throttling per plugin; self-diagnostics page (already a PROMPT.md requirement) surfacing exactly what's consuming resources. |
| Local LLM inference denial via resource starvation | Heavy local inference could make the machine unusable during active work — ironic for a product meant to reduce friction. | Idle-time/batched inference scheduling by default; user-configurable inference intensity caps. |

### Elevation of Privilege

| Threat | Detail | Mitigation |
|---|---|---|
| Accessibility-API/UI-tree access is inherently broad | On most platforms, the permissions needed to observe window/UI state (Accessibility API on macOS, UI Automation on Windows) are broad, all-or-nothing OS grants — HiddenSteps necessarily requests more OS-level access than it needs for any single observation mode. | Request only the OS permission tier needed for the *currently selected* privacy level, not the maximum tier upfront; re-prompt/re-justify when the user raises their privacy level rather than requesting everything at install time. |
| Plugin escaping its declared capability scope | A plugin manifest claims "window-title-only" access but the plugin framework doesn't actually enforce that boundary at runtime, allowing a compromised or malicious plugin to read arbitrary screen content. | Runtime capability enforcement (not just manifest-based trust) is a hard requirement for the plugin framework — this is the highest-leverage single control in the whole threat model, since the plugin surface is the widest. |

## Assumptions and out-of-scope

- **Assumes zero trust in the sense specified by PROMPT.md**: no component (local storage, plugin, OS-level permission grant) is trusted by default; every boundary above gets an explicit control.
- **Does not cover** application-layer web/API vulnerabilities of a companion cloud service, because Phase 1 assumes local-first-by-default; if a companion sync/cloud service is added in a later phase, it needs its own threat model addendum (auth, multi-tenant isolation, data-in-transit).
- **Physical/coercive access** (e.g., a device seized under legal process, or a user compelled to unlock it) is a real threat for a tool that may contain a rich behavioral record, but is treated here as a policy/legal question (data minimization and short retention windows are the main mitigation available to the product) rather than a solvable technical control.
