# API Specification

HiddenSteps has no public network API by default (local-first, per [../research/05-privacy-analysis.md](../research/05-privacy-analysis.md)) — "API" here means the two internal contracts that make the architecture extensible and inspectable: the **UI ↔ Core IPC** (Tauri commands/events) and the **Plugin Host interface** (already specified in WIT in [08-plugin-architecture.md](08-plugin-architecture.md) §4). This doc covers the former in full and cross-references the latter.

## 1. Design rules

- Every command is a capability the UI can invoke on the Application layer ([02-system-architecture.md](02-system-architecture.md)) — never a direct passthrough to Infrastructure (no command lets the UI touch the filesystem, network, or OS APIs directly).
- Every command that reads or changes privacy-relevant state is itself subject to the same audit-logging discipline as its underlying use-case — the IPC layer doesn't add a bypass.
- Commands are versioned as a group (`v1`); a breaking change to any command's shape bumps the whole IPC contract version, since UI and core ship together in one release.

## 2. Commands (UI → Core)

### Onboarding & setup

| Command | Request | Response |
|---|---|---|
| `get_onboarding_state` | — | `{ step, completed: bool }` |
| `get_provider_detection` | — | `{ detected_local: [{ runtime, endpoint, models: [...] }], hardware: { ram_gb, gpu, suitability } }` |
| `test_provider_connectivity` | `{ provider_type, endpoint?, api_key? }` | `{ ok: bool, error?: string }` |
| `set_privacy_level` | `{ level: 0-4, acknowledged_permissions: [string] }` | `{ effective_level, newly_active_sources: [string] }` |
| `set_ai_provider` | `{ provider_id, api_key? }` | `{ active: bool }` |
| `complete_onboarding` | `{ consent: true }` | `{ observation_active: bool }` |

### Observation & privacy dashboard

| Command | Request | Response |
|---|---|---|
| `get_observation_status` | — | `{ active: bool, privacy_level, active_sources: [{id, display_name, signal_types}] }` |
| `pause_observation` / `resume_observation` | — | `{ active: bool }` |
| `get_recent_events` | `{ limit, since? }` | `[{ id, occurred_at, source_id, signal_type, summary_json }]` (post-redaction only — same content that's stored, per FR-5) |
| `change_privacy_level` | `{ new_level, acknowledged_permissions: [string] }` | `{ effective_level }` (see [03-data-flow-diagrams.md](03-data-flow-diagrams.md) §3) |
| `set_exclusion_rule` | `{ scope: "app"|"domain"|"window_title", pattern }` | `{ rule_id }` |
| `delete_events` | `{ event_ids: [int] } | { all: true }` | `{ deleted_count }` |
| `export_data` | `{ destination_path }` | `{ archive_path }` |
| `delete_all_data` | `{ confirm: true }` | `{ ok: bool }` (see [03-data-flow-diagrams.md](03-data-flow-diagrams.md) §4) |

### Patterns & recommendations

| Command | Request | Response |
|---|---|---|
| `list_patterns` | `{ status_filter? }` | `[{ id, occurrence_count, estimated_minutes_per_occurrence, first_seen_at, last_seen_at }]` |
| `list_recommendations` | `{ status_filter? }` | `[Recommendation]` (full shape mirrors [07-database-schema.md](07-database-schema.md) `recommendations` table) |
| `get_recommendation_detail` | `{ id }` | `Recommendation & { contributing_events: [EventSummary] }` (FR-13 traceability) |
| `set_recommendation_status` | `{ id, status: "implemented"|"dismissed", dismissal_reason? }` | `{ ok: bool }` |

### Providers, plugins, settings

| Command | Request | Response |
|---|---|---|
| `list_llm_providers` | — | `[{ id, provider_type, is_local, active }]` |
| `list_plugins` | — | `[{ id, name, version, trust_tier, granted_capabilities, enabled }]` |
| `install_plugin` | `{ manifest_path }` | `{ plugin_id, requested_capabilities: [...] }` (disclosure step, per [08-plugin-architecture.md](08-plugin-architecture.md) §3) |
| `grant_plugin_capabilities` | `{ plugin_id, capabilities: [...] }` | `{ granted: [...] }` |
| `revoke_plugin_capability` | `{ plugin_id, capability }` | `{ ok: bool }` |
| `uninstall_plugin` | `{ plugin_id }` | `{ ok: bool }` |
| `get_settings` / `update_settings` | `{ key }` / `{ key, value }` | `{ value }` / `{ ok: bool }` |
| `get_audit_log` | `{ limit, since? }` | `[{ id, occurred_at, actor, action_type, details_json }]` |

### Diagnostics

| Command | Request | Response |
|---|---|---|
| `get_diagnostics` | — | `{ provider_status, model_status, gpu, cpu_usage, memory_usage, storage_usage, observation_permissions, security_status, encryption_status, update_status }` (per PROMPT.md Self-Diagnostics) |
| `get_network_activity_log` | — | `[{ timestamp, destination, allowed_by_capability }]` (air-gapped-mode verifiability, [06-security-architecture.md](06-security-architecture.md) §5) |

## 3. Events (Core → UI)

Pushed asynchronously over Tauri's event system; the UI subscribes rather than polls for anything latency-sensitive.

| Event | Payload | Purpose |
|---|---|---|
| `observation::event_captured` | `EventSummary` (post-redaction) | Drives the live recent-events feed in the privacy dashboard |
| `observation::status_changed` | `{ active, privacy_level }` | Keeps the persistent status indicator ([04-trust-model.md](04-trust-model.md) §2) live |
| `recommendation::new` | `Recommendation` | Surfaces a freshly generated recommendation, e.g. for the Zero-to-Value first-24-hours moment |
| `plugin::capability_used` | `{ plugin_id, capability, timestamp }` | Real-time capability-usage visibility, beyond the static audit log |
| `update::status_changed` | `{ state: "checking"|"available"|"downloading"|"ready"|"error" }` | Drives the update UI |
| `security::key_or_vault_error` | `{ error_kind }` | Surfaces key-loss/vault-access failures immediately rather than on next action |

## 4. Non-goals for this API surface

- No authenticated remote API — there is no server component in this phase ([01-prd.md](01-prd.md) §8 excludes cross-device sync); adding one later requires its own API spec and threat-model addendum, not an extension of this local IPC contract.
- No aggregate/multi-user query surface — every command above is scoped to the single local user profile, consistent with NG2.
