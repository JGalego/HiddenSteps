# Plugin Architecture

Implements PROMPT.md's "everything should be pluggable" requirement across observation, LLM/embedding providers, automation providers, enterprise policies, recommendation engines, pattern detectors, and integrations, on the WASM sandbox committed in ADR-0009.

## 1. Plugin types

| Plugin type | Extends | Example |
|---|---|---|
| Observation source | `ObservationSource` | A Jira/Linear integration that captures ticket-transition events as workflow signal, at Standard level |
| LLM provider | `LlmProvider` | A provider not shipped in-tree (e.g., a niche local runtime, or a future cloud API) |
| Embedding provider | `LlmProvider` (embedding method) | An alternative embedding model source |
| Pattern detector | `PatternDetector` | A domain-specific detector (e.g., specialized for spreadsheet-formula-repetition patterns) supplementing the in-tree deterministic detector |
| Automation target integration | `AutomationTarget` | A connector that can, on explicit user approval, materialize a recommendation into a real n8n flow, Playwright script, etc. (the §9 "future consideration" feature in [01-prd.md](01-prd.md)) |
| Enterprise policy source | `PolicyLoader` | A connector to an enterprise config-management system beyond a flat policy file |
| Recommendation engine variant | `RecommendationSynthesizer` | An alternative synthesis strategy a technical user substitutes for the in-tree Layer 2 |

All plugin types share one manifest schema and one capability-grant model; the plugin *type* just determines which host trait(s) it implements.

## 2. Manifest schema

```json
{
  "id": "com.example.jira-observer",
  "name": "Jira Workflow Observer",
  "version": "1.2.0",
  "plugin_type": "observation_source",
  "min_privacy_level": 2,
  "capabilities": [
    { "kind": "observe", "scope": "app_action_events", "detail": "jira-desktop-only" },
    { "kind": "network", "scope": "outbound", "allowlist": ["*.atlassian.net"] }
  ],
  "signature": "base64...",
  "publisher_key_id": "..."
}
```

- `capabilities` is drawn from a **closed enumeration** the host recognizes: `observe:{app_focus, window_title, app_action_events, browser_domain, clipboard_metadata, file_op_metadata, ocr_text, screenshot, accessibility_tree}`, `network:outbound(allowlist)`, `filesystem:read(path_scope)`, `provider:llm`, `provider:embedding`, `policy:read`. A manifest requesting a capability outside this enumeration is rejected at install time — there is no "unknown capability, grant anyway" path.
- `min_privacy_level` ties an observation-source plugin to the privacy-level gating in [05-privacy-model.md](05-privacy-model.md) — the host will not activate it below that level regardless of what the user's settings otherwise allow.
- `signature`/`publisher_key_id` are verified the same way update packages are (§3 of [06-security-architecture.md](06-security-architecture.md)) before the plugin is even loaded into the WASM host, not just before it's granted capabilities.

## 3. Capability grant model

1. **Install-time**: manifest is parsed, signature verified, capabilities checked against the closed enumeration. Any `network:outbound` or `filesystem:read` capability request is surfaced to the user in plain language ("This plugin can send data to `*.atlassian.net`") before install completes — mirroring the OS-permission-explanation discipline from onboarding ([04-trust-model.md](04-trust-model.md) §3).
2. **Grant**: on user approval, the host records the granted capability set in `observation_sources.granted_capabilities_json` ([07-database-schema.md](07-database-schema.md)) and instantiates the plugin's WASM component with **only** the corresponding host-function imports linked — an ungranted capability isn't merely policy-blocked, its host function is absent from the plugin's import table, so there is nothing for the plugin's code to call even if it tries.
3. **Runtime**: every host-function call the plugin does make is additionally logged (capability usage, not content) to the audit log if it's a sensitive-tier capability (`network:outbound`, `filesystem:read`, `observe:ocr_text`/`observe:screenshot`), giving a runtime-verifiable trail beyond the static grant.
4. **Revoke**: the user can revoke any granted capability at any time from Settings; revocation takes effect immediately (the plugin instance is reloaded without that import, or unloaded entirely if the capability was load-bearing for its function).
5. **Update**: a version update that requests a **new** capability beyond the previously granted set re-triggers the install-time disclosure/approval step for the delta only — it does not silently inherit broader trust from the prior version.

## 4. Host interface (WIT sketch)

```wit
package hiddensteps:plugin-host@1.0.0;

interface observation-source {
  record signal {
    signal-type: string,
    occurred-at: string,
    payload-json: string,   // structured, plugin-produced; still passes through host Redact/Summarize
  }
  emit-signal: func(signal: signal);
}

interface llm-provider {
  record completion-request { prompt: string, context-json: string }
  record completion-response { text: string }
  complete: func(req: completion-request) -> result<completion-response, string>;
  embed: func(text: string) -> result<list<float32>, string>;
  is-local: func() -> bool;
}

interface host-capabilities {
  // Only linked into a plugin instance if the corresponding manifest capability was granted.
  network-fetch: func(url: string) -> result<list<u8>, string>;
  read-file-metadata: func(path: string) -> result<string, string>;
}
```

Every signal a plugin emits via `emit-signal` still passes through the host's Classify → Redact → Summarize pipeline (ADR-0006) — a plugin cannot bypass redaction by claiming its payload is already safe; the host treats plugin-sourced signals identically to in-tree ones for pipeline purposes.

## 5. Distribution and trust tiers

- **In-tree plugins** (default Minimal/Standard observation sources, Ollama/OpenAI/Anthropic providers) ship signed with the main application, reviewed as part of the core codebase, but declared through the same manifest schema — so the privacy dashboard and capability-audit views are uniform regardless of tier (ADR-0009).
- **Third-party plugins**: no central marketplace is in scope for this phase ([01-prd.md](01-prd.md) §8) — distribution is by direct file (signed `.wasm` + manifest bundle) the user explicitly installs, or an enterprise-policy-pushed set for managed deployments. This keeps the trust chain simple (publisher signature → user approval) without requiring a hosted review pipeline to exist before third-party extensibility is usable at all.

## 6. Enterprise policy as a constrained plugin type

The Enterprise Policy Engine ([02-system-architecture.md](02-system-architecture.md)) loads policy through the same `PolicyLoader` plugin interface, but its capability surface is deliberately narrow and asymmetric: it may only *read* configuration and *write* constraints (privacy-level floor, provider allowlist) into `enterprise_policy` ([07-database-schema.md](07-database-schema.md)); it has no `observe:*` or data-read capability of any kind, structurally preventing an enterprise policy plugin from ever becoming a surveillance channel — enforced the same way any other capability boundary is (§3 above), not by convention.
