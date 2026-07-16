# Database Schema

Single SQLCipher-encrypted SQLite file (`hiddensteps.db`, ADR-0003) with the `sqlite-vec` extension loaded for embeddings (ADR-0007). Every table below stores only post-pipeline abstractions — there is deliberately no table capable of holding raw, pre-redaction captured content (ADR-0006); this is a design property to preserve, not just a current fact, when evolving the schema.

## 1. Schema

```sql
-- Versioning, so migrations are explicit and Portable Mode files remain self-describing.
CREATE TABLE schema_version (
    version         INTEGER PRIMARY KEY,
    applied_at      TEXT NOT NULL  -- ISO-8601
);

-- Current privacy level and which manifest version the user consented to (05-privacy-model.md §5).
CREATE TABLE privacy_state (
    id                      INTEGER PRIMARY KEY CHECK (id = 1),  -- singleton
    current_level           INTEGER NOT NULL CHECK (current_level BETWEEN 0 AND 4),
    consented_manifest_version INTEGER NOT NULL,
    observation_active      INTEGER NOT NULL DEFAULT 0,  -- boolean
    updated_at              TEXT NOT NULL
);

-- Registered observation sources (in-tree and plugin), and what they're currently allowed to do.
CREATE TABLE observation_sources (
    id                  TEXT PRIMARY KEY,           -- e.g. "macos.active_window", "plugin.acme_jira_tracker"
    display_name        TEXT NOT NULL,
    trust_tier          TEXT NOT NULL CHECK (trust_tier IN ('in_tree', 'third_party')),
    manifest_json        TEXT NOT NULL,              -- declared capabilities, signal types, required OS permission tier
    granted_capabilities_json TEXT NOT NULL DEFAULT '[]',
    min_privacy_level   INTEGER NOT NULL,            -- lowest level at which this source may run
    enabled             INTEGER NOT NULL DEFAULT 0,
    installed_at        TEXT NOT NULL
);

-- The durable abstraction of a captured signal, AFTER classify/redact/summarize.
-- There is intentionally no column here that could hold raw pre-redaction content.
CREATE TABLE event_summaries (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    occurred_at         TEXT NOT NULL,
    source_id           TEXT NOT NULL REFERENCES observation_sources(id),
    signal_type         TEXT NOT NULL,               -- e.g. 'app_focus_change', 'clipboard_metadata', 'file_op'
    privacy_level_at_capture INTEGER NOT NULL,
    summary_json        TEXT NOT NULL,               -- structured, redacted summary (schema per signal_type)
    is_deep_mode        INTEGER NOT NULL DEFAULT 0,   -- true if derived from Level 4 capture
    ttl_expires_at      TEXT                          -- non-null only for is_deep_mode rows (05-privacy-model.md §2)
);
CREATE INDEX idx_event_summaries_time ON event_summaries(occurred_at);
CREATE INDEX idx_event_summaries_ttl ON event_summaries(ttl_expires_at) WHERE ttl_expires_at IS NOT NULL;

-- Vector index over event/pattern summary embeddings (sqlite-vec virtual table).
-- Embeds the SUMMARY, never raw content (ADR-0007).
CREATE VIRTUAL TABLE summary_embeddings USING vec0(
    summary_id          INTEGER PRIMARY KEY,   -- FK-by-convention to event_summaries.id or patterns.id
    embedding            FLOAT[384]             -- dimension depends on configured embedding model
);

-- A detected repeated workflow — the deterministic Layer 1 output (ADR-0010).
CREATE TABLE patterns (
    id                      INTEGER PRIMARY KEY AUTOINCREMENT,
    first_seen_at           TEXT NOT NULL,
    last_seen_at            TEXT NOT NULL,
    occurrence_count        INTEGER NOT NULL,
    estimated_minutes_per_occurrence REAL,
    sequence_signature_json TEXT NOT NULL,        -- canonicalized action-sequence shape used for matching
    status                  TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'stale', 'dismissed'))
);

-- Which event summaries contributed to which pattern (traceability — "what observations contributed?", FR-13).
CREATE TABLE pattern_events (
    pattern_id      INTEGER NOT NULL REFERENCES patterns(id) ON DELETE CASCADE,
    event_id        INTEGER NOT NULL REFERENCES event_summaries(id) ON DELETE CASCADE,
    PRIMARY KEY (pattern_id, event_id)
);

-- Workflow graph: nodes are distinct app/action states, edges are observed transitions.
CREATE TABLE workflow_nodes (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    label           TEXT NOT NULL,
    node_type       TEXT NOT NULL            -- e.g. 'application', 'action', 'domain_category'
);

CREATE TABLE workflow_edges (
    from_node_id    INTEGER NOT NULL REFERENCES workflow_nodes(id) ON DELETE CASCADE,
    to_node_id      INTEGER NOT NULL REFERENCES workflow_nodes(id) ON DELETE CASCADE,
    pattern_id      INTEGER REFERENCES patterns(id) ON DELETE SET NULL,
    weight          INTEGER NOT NULL DEFAULT 1,
    PRIMARY KEY (from_node_id, to_node_id, pattern_id)
);

-- Recommendation Engine output. Layer-1 numeric fields are copied verbatim from `patterns`
-- at generation time and validated against drift from the LLM synthesis step (ADR-0010).
CREATE TABLE recommendations (
    id                          INTEGER PRIMARY KEY AUTOINCREMENT,
    pattern_id                  INTEGER NOT NULL REFERENCES patterns(id),
    created_at                  TEXT NOT NULL,
    title                       TEXT NOT NULL,
    category                    TEXT NOT NULL,     -- shortcut|template|script|browser_automation|rpa|workflow_platform|ai_agent|hybrid
    why                         TEXT NOT NULL,
    confidence                  REAL NOT NULL CHECK (confidence BETWEEN 0 AND 1),
    estimated_time_saved_minutes REAL NOT NULL,     -- copied from patterns.estimated_minutes_per_occurrence * occurrence rate
    difficulty                  TEXT NOT NULL CHECK (difficulty IN ('low', 'medium', 'high')),
    maintenance_burden           TEXT NOT NULL CHECK (maintenance_burden IN ('low', 'medium', 'high')),
    privacy_implications         TEXT NOT NULL,
    implementation_effort        TEXT NOT NULL,
    alternatives_json            TEXT NOT NULL,     -- array of {approach, tradeoff}
    assumptions_json             TEXT NOT NULL,
    ignored_information_json     TEXT NOT NULL,     -- explainability: "what was intentionally ignored"
    generating_provider          TEXT NOT NULL,     -- which LlmProvider produced the synthesis
    status                       TEXT NOT NULL DEFAULT 'suggested' CHECK (status IN ('suggested', 'implemented', 'dismissed')),
    dismissal_reason             TEXT
);
CREATE INDEX idx_recommendations_pattern ON recommendations(pattern_id);

-- AI provider configuration. API keys/secrets are NEVER stored here — only a reference
-- to the OS credential vault entry (06-security-architecture.md §1).
CREATE TABLE llm_providers (
    id              TEXT PRIMARY KEY,          -- e.g. "ollama-local", "openai-cloud"
    provider_type   TEXT NOT NULL,
    is_local        INTEGER NOT NULL,
    model_name      TEXT,
    endpoint         TEXT,
    vault_key_ref    TEXT,                      -- opaque reference into the OS credential vault; NULL for local
    active          INTEGER NOT NULL DEFAULT 0
);

-- Append-only audit log. Metadata only, never captured content (06-security-architecture.md §4).
CREATE TABLE audit_log (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    occurred_at     TEXT NOT NULL,
    actor           TEXT NOT NULL CHECK (actor IN ('user', 'system')),
    action_type     TEXT NOT NULL,       -- e.g. 'privacy_level_changed', 'plugin_capability_granted', 'data_exported'
    details_json    TEXT NOT NULL        -- structured metadata about the action, no content payloads
);
CREATE INDEX idx_audit_log_time ON audit_log(occurred_at);

-- General settings (UI complexity tier, retention overrides, exclusion rules, etc.)
CREATE TABLE settings (
    key         TEXT PRIMARY KEY,
    value_json  TEXT NOT NULL
);

-- Enterprise policy snapshot currently in effect (05-privacy-model.md §6), for diagnostics/audit visibility.
CREATE TABLE enterprise_policy (
    id                  INTEGER PRIMARY KEY CHECK (id = 1),  -- singleton
    policy_source        TEXT,             -- file path or distribution mechanism identifier
    privacy_level_floor  INTEGER,
    provider_allowlist_json TEXT,
    applied_at           TEXT
);
```

## 2. Design notes

- **No table stores raw pre-redaction content.** `event_summaries.summary_json` is populated only after the Summarize stage; `ttl_expires_at` is non-null exclusively for Level-4-derived rows, implementing the retention rule in [05-privacy-model.md](05-privacy-model.md) §2 directly in schema rather than relying solely on application-code discipline.
- **`pattern_events` gives full traceability** from a recommendation back to `patterns` back to the specific `event_summaries` rows that produced it — satisfying FR-13's "what observations contributed?" requirement as a straightforward join, not a reconstruction.
- **`recommendations`' numeric fields are denormalized (copied) from `patterns` at generation time**, not computed live at read time — this is deliberate: it lets the ADR-0010 validator compare "what Layer 1 computed" against "what the LLM's synthesis said" as a stored, auditable pair, and it means a recommendation's displayed numbers don't silently drift if the underlying pattern is later updated by new observations (a new `recommendations` row is generated instead).
- **`llm_providers.vault_key_ref` is opaque** — the actual secret never enters this database, keeping the encrypted-DB and OS-vault trust boundaries cleanly separated (ADR-0008): compromising the DB file alone yields no provider credentials.
- **A full delete-all operation** (per [03-data-flow-diagrams.md](03-data-flow-diagrams.md) §4) drops and recreates every table above except `schema_version`, then triggers `VACUUM`, then removes the vault key entry — ensuring no SQLite free-list remnants of deleted rows survive the encrypted file itself being effectively useless without the (now-deleted) key.
- **Migrations** are forward-only, versioned scripts keyed against `schema_version`; Portable Mode files carry their own version so a newer app build can detect and migrate an older portable data directory without assuming it matches the currently installed version.
