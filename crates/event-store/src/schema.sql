-- Mirrors docs/design/07-database-schema.md exactly, with one deliberate
-- deviation: the `summary_embeddings` sqlite-vec virtual table is deferred to the
-- migration that ships the Embedding Layer (roadmap milestone M2) — loading the
-- sqlite-vec extension is Embedding Layer's concern, not the base EventStore's, and
-- there is nothing to embed yet before Pattern Detection exists to produce
-- pattern-level summaries. Every other table below is created now so the schema a
-- future migration extends is the real one, not a placeholder.

CREATE TABLE IF NOT EXISTS schema_version (
    version         INTEGER PRIMARY KEY,
    applied_at      TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS privacy_state (
    id                      INTEGER PRIMARY KEY CHECK (id = 1),
    current_level           INTEGER NOT NULL CHECK (current_level BETWEEN 0 AND 4),
    consented_manifest_version INTEGER NOT NULL,
    observation_active      INTEGER NOT NULL DEFAULT 0,
    updated_at              TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS observation_sources (
    id                  TEXT PRIMARY KEY,
    display_name        TEXT NOT NULL,
    trust_tier          TEXT NOT NULL CHECK (trust_tier IN ('in_tree', 'third_party')),
    manifest_json        TEXT NOT NULL,
    granted_capabilities_json TEXT NOT NULL DEFAULT '[]',
    min_privacy_level   INTEGER NOT NULL,
    enabled             INTEGER NOT NULL DEFAULT 0,
    installed_at        TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS event_summaries (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    occurred_at         TEXT NOT NULL,
    source_id           TEXT NOT NULL,
    signal_type         TEXT NOT NULL,
    privacy_level_at_capture INTEGER NOT NULL,
    summary_json        TEXT NOT NULL,
    is_deep_mode        INTEGER NOT NULL DEFAULT 0,
    ttl_expires_at      TEXT
);
CREATE INDEX IF NOT EXISTS idx_event_summaries_time ON event_summaries(occurred_at);
CREATE INDEX IF NOT EXISTS idx_event_summaries_ttl ON event_summaries(ttl_expires_at) WHERE ttl_expires_at IS NOT NULL;

CREATE TABLE IF NOT EXISTS patterns (
    id                      INTEGER PRIMARY KEY AUTOINCREMENT,
    first_seen_at           TEXT NOT NULL,
    last_seen_at            TEXT NOT NULL,
    occurrence_count        INTEGER NOT NULL,
    estimated_minutes_per_occurrence REAL,
    sequence_signature_json TEXT NOT NULL,
    status                  TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'stale', 'dismissed'))
);

CREATE TABLE IF NOT EXISTS pattern_events (
    pattern_id      INTEGER NOT NULL REFERENCES patterns(id) ON DELETE CASCADE,
    event_id        INTEGER NOT NULL REFERENCES event_summaries(id) ON DELETE CASCADE,
    PRIMARY KEY (pattern_id, event_id)
);

CREATE TABLE IF NOT EXISTS workflow_nodes (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    label           TEXT NOT NULL,
    node_type       TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS workflow_edges (
    from_node_id    INTEGER NOT NULL REFERENCES workflow_nodes(id) ON DELETE CASCADE,
    to_node_id      INTEGER NOT NULL REFERENCES workflow_nodes(id) ON DELETE CASCADE,
    pattern_id      INTEGER REFERENCES patterns(id) ON DELETE SET NULL,
    weight          INTEGER NOT NULL DEFAULT 1,
    PRIMARY KEY (from_node_id, to_node_id, pattern_id)
);

CREATE TABLE IF NOT EXISTS recommendations (
    id                          INTEGER PRIMARY KEY AUTOINCREMENT,
    pattern_id                  INTEGER NOT NULL REFERENCES patterns(id),
    created_at                  TEXT NOT NULL,
    title                       TEXT NOT NULL,
    category                    TEXT NOT NULL,
    why                         TEXT NOT NULL,
    confidence                  REAL NOT NULL CHECK (confidence BETWEEN 0 AND 1),
    estimated_time_saved_minutes REAL NOT NULL,
    difficulty                  TEXT NOT NULL CHECK (difficulty IN ('low', 'medium', 'high')),
    maintenance_burden           TEXT NOT NULL CHECK (maintenance_burden IN ('low', 'medium', 'high')),
    privacy_implications         TEXT NOT NULL,
    implementation_effort        TEXT NOT NULL,
    alternatives_json            TEXT NOT NULL,
    assumptions_json             TEXT NOT NULL,
    ignored_information_json     TEXT NOT NULL,
    generating_provider          TEXT NOT NULL,
    status                       TEXT NOT NULL DEFAULT 'suggested' CHECK (status IN ('suggested', 'implemented', 'dismissed')),
    dismissal_reason             TEXT
);
CREATE INDEX IF NOT EXISTS idx_recommendations_pattern ON recommendations(pattern_id);

CREATE TABLE IF NOT EXISTS llm_providers (
    id              TEXT PRIMARY KEY,
    provider_type   TEXT NOT NULL,
    is_local        INTEGER NOT NULL,
    model_name      TEXT,
    endpoint         TEXT,
    vault_key_ref    TEXT,
    active          INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS audit_log (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    occurred_at     TEXT NOT NULL,
    actor           TEXT NOT NULL CHECK (actor IN ('user', 'system')),
    action_type     TEXT NOT NULL,
    details_json    TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_audit_log_time ON audit_log(occurred_at);

CREATE TABLE IF NOT EXISTS settings (
    key         TEXT PRIMARY KEY,
    value_json  TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS enterprise_policy (
    id                  INTEGER PRIMARY KEY CHECK (id = 1),
    policy_source        TEXT,
    privacy_level_floor  INTEGER,
    provider_allowlist_json TEXT,
    applied_at           TEXT
);
