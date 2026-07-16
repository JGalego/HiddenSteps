use std::path::Path;
use std::sync::Mutex;

use hiddensteps_domain::{
    Alternative, AuditActor, AuditEntry, EventSummary, Level, LlmProviderConfig, Pattern,
    PatternStatus, PrivacyLevel, PrivacyState, Recommendation, RecommendationCategory,
    RecommendationStatus, SignalType,
};
use rusqlite::{params, Connection, OptionalExtension};
use time::OffsetDateTime;

use crate::time_fmt::{from_rfc3339, to_rfc3339};
use crate::EventStoreError;

const SCHEMA: &str = include_str!("schema.sql");
const CURRENT_SCHEMA_VERSION: i64 = 1;

/// The single encrypted SQLite file described in ADR-0003 and
/// `docs/design/07-database-schema.md`. One `SqlCipherEventStore` per user profile.
pub struct SqlCipherEventStore {
    conn: Mutex<Connection>,
}

impl SqlCipherEventStore {
    /// Opens (creating if necessary) the encrypted store at `path`, keyed by
    /// `key` — the 256-bit master key that `hiddensteps-security` retrieves from
    /// the OS credential vault (or derives from a Portable Mode passphrase).
    pub fn open(path: &Path, key: &[u8; 32]) -> Result<Self, EventStoreError> {
        let conn = Connection::open(path)?;
        Self::from_connection(conn, key)
    }

    /// Opens a fully in-memory store — used by tests that don't need a real file on
    /// disk. Still goes through the same key-application and migration path as the
    /// file-backed constructor, so tests exercise real behavior, not a stub.
    pub fn open_in_memory(key: &[u8; 32]) -> Result<Self, EventStoreError> {
        let conn = Connection::open_in_memory()?;
        Self::from_connection(conn, key)
    }

    fn from_connection(conn: Connection, key: &[u8; 32]) -> Result<Self, EventStoreError> {
        apply_key(&conn, key)?;
        verify_key(&conn)?;
        let store = Self {
            conn: Mutex::new(conn),
        };
        store.migrate()?;
        Ok(store)
    }

    fn migrate(&self) -> Result<(), EventStoreError> {
        let conn = self.conn.lock().expect("event store mutex poisoned");
        conn.execute_batch(SCHEMA)?;

        let existing: Option<i64> = conn
            .query_row("SELECT MAX(version) FROM schema_version", [], |row| {
                row.get(0)
            })
            .optional()?
            .flatten();

        if existing.is_none() {
            let now = to_rfc3339(OffsetDateTime::now_utc());
            conn.execute(
                "INSERT INTO schema_version (version, applied_at) VALUES (?1, ?2)",
                params![CURRENT_SCHEMA_VERSION, now],
            )?;
            conn.execute(
                "INSERT INTO privacy_state
                    (id, current_level, consented_manifest_version, observation_active, updated_at)
                 VALUES (1, 0, 0, 0, ?1)",
                params![now],
            )?;
        }
        Ok(())
    }

    // --- privacy state (FR-2, FR-5) ---

    pub fn get_privacy_state(&self) -> Result<PrivacyState, EventStoreError> {
        let conn = self.conn.lock().expect("event store mutex poisoned");
        let (level, manifest_version, active, updated_at): (i64, i64, i64, String) = conn
            .query_row(
                "SELECT current_level, consented_manifest_version, observation_active, updated_at
                 FROM privacy_state WHERE id = 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )?;

        Ok(PrivacyState {
            current_level: PrivacyLevel::from_u8(level as u8)
                .map_err(|e| EventStoreError::InvalidStoredValue(e.to_string()))?,
            consented_manifest_version: manifest_version,
            observation_active: active != 0,
            updated_at: from_rfc3339(&updated_at)?,
        })
    }

    /// Changes the privacy level and/or observation-active flag, and records
    /// which manifest version the user consented to — per
    /// `docs/design/03-data-flow-diagrams.md` §3, this is always paired with an
    /// audit-log entry by the caller (the Privacy Engine), not by this store.
    pub fn set_privacy_state(&self, state: &PrivacyState) -> Result<(), EventStoreError> {
        let conn = self.conn.lock().expect("event store mutex poisoned");
        conn.execute(
            "UPDATE privacy_state
             SET current_level = ?1, consented_manifest_version = ?2,
                 observation_active = ?3, updated_at = ?4
             WHERE id = 1",
            params![
                state.current_level.as_u8(),
                state.consented_manifest_version,
                state.observation_active as i64,
                to_rfc3339(state.updated_at),
            ],
        )?;
        Ok(())
    }

    // --- event summaries (FR-5, FR-9) ---

    /// Persists an already-classified, already-redacted, already-summarized event.
    /// There is no overload accepting a `CapturedSignal` — see that type's doc
    /// comment in the domain crate.
    pub fn insert_event_summary(&self, event: &EventSummary) -> Result<i64, EventStoreError> {
        let conn = self.conn.lock().expect("event store mutex poisoned");
        conn.execute(
            "INSERT INTO event_summaries
                (occurred_at, source_id, signal_type, privacy_level_at_capture,
                 summary_json, is_deep_mode, ttl_expires_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                to_rfc3339(event.occurred_at),
                event.source_id,
                signal_type_to_str(event.signal_type),
                event.privacy_level_at_capture.as_u8(),
                serde_json::to_string(&event.summary)?,
                event.is_deep_mode as i64,
                event.ttl_expires_at.map(to_rfc3339),
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    /// Returns the most recent events, newest first — the data source for the
    /// privacy dashboard's live recent-events feed
    /// (`docs/design/09-api-specification.md` `get_recent_events`). This returns
    /// exactly what's stored; the dashboard must not show anything this method
    /// wouldn't also return, per the trust-model claim in
    /// `docs/design/04-trust-model.md` §2.
    pub fn list_recent_events(&self, limit: i64) -> Result<Vec<EventSummary>, EventStoreError> {
        let conn = self.conn.lock().expect("event store mutex poisoned");
        let mut stmt = conn.prepare(
            "SELECT id, occurred_at, source_id, signal_type, privacy_level_at_capture,
                    summary_json, is_deep_mode, ttl_expires_at
             FROM event_summaries
             ORDER BY occurred_at DESC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit], row_to_event_summary)?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(EventStoreError::from)
    }

    /// Deletes specific events by id (FR-6's selective-delete path).
    pub fn delete_events(&self, ids: &[i64]) -> Result<usize, EventStoreError> {
        if ids.is_empty() {
            return Ok(0);
        }
        let conn = self.conn.lock().expect("event store mutex poisoned");
        let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let sql = format!("DELETE FROM event_summaries WHERE id IN ({placeholders})");
        let mut stmt = conn.prepare(&sql)?;
        let params: Vec<&dyn rusqlite::ToSql> =
            ids.iter().map(|id| id as &dyn rusqlite::ToSql).collect();
        Ok(stmt.execute(params.as_slice())?)
    }

    // --- audit log (FR-8, `docs/design/06-security-architecture.md` §4) ---

    pub fn append_audit_entry(&self, entry: &AuditEntry) -> Result<i64, EventStoreError> {
        let conn = self.conn.lock().expect("event store mutex poisoned");
        conn.execute(
            "INSERT INTO audit_log (occurred_at, actor, action_type, details_json)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                to_rfc3339(entry.occurred_at),
                actor_to_str(entry.actor),
                entry.action_type,
                serde_json::to_string(&entry.details)?,
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn list_audit_log(&self, limit: i64) -> Result<Vec<AuditEntry>, EventStoreError> {
        let conn = self.conn.lock().expect("event store mutex poisoned");
        let mut stmt = conn.prepare(
            "SELECT id, occurred_at, actor, action_type, details_json
             FROM audit_log
             ORDER BY occurred_at DESC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit], row_to_audit_entry)?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(EventStoreError::from)
    }

    // --- delete-all / export (FR-6, `docs/design/03-data-flow-diagrams.md` §4) ---

    /// Removes every row from every table except `schema_version`, then reclaims
    /// the freed space. Deleting the vault key entry (making the file itself
    /// unreadable even if a copy survives) is the caller's responsibility — this
    /// method only clears the store's own contents, per the Security Layer /
    /// EventStore split in `docs/design/02-system-architecture.md`.
    pub fn delete_all_data(&self) -> Result<(), EventStoreError> {
        let conn = self.conn.lock().expect("event store mutex poisoned");
        const TABLES: &[&str] = &[
            "pattern_events",
            "pattern_embeddings",
            "workflow_edges",
            "workflow_nodes",
            "recommendations",
            "patterns",
            "event_summaries",
            "observation_sources",
            "llm_providers",
            "audit_log",
            "settings",
            "enterprise_policy",
        ];
        for table in TABLES {
            conn.execute(&format!("DELETE FROM {table}"), [])?;
        }
        conn.execute(
            "UPDATE privacy_state
             SET current_level = 0, consented_manifest_version = 0,
                 observation_active = 0, updated_at = ?1
             WHERE id = 1",
            params![to_rfc3339(OffsetDateTime::now_utc())],
        )?;
        conn.execute_batch("VACUUM;")?;
        Ok(())
    }

    /// Serializes every table's contents to JSON, for the Export Data trust
    /// feature. Decryption already happened implicitly on read — this is plain
    /// serialization of already-decrypted rows, per
    /// `docs/design/03-data-flow-diagrams.md` §4.
    pub fn export_data(&self) -> Result<serde_json::Value, EventStoreError> {
        let events = self.list_recent_events(i64::MAX)?;
        let audit_log = self.list_audit_log(i64::MAX)?;
        let privacy_state = self.get_privacy_state()?;
        let patterns = self.list_patterns(None)?;
        let recommendations = self.list_recommendations(None)?;
        Ok(serde_json::json!({
            "privacy_state": privacy_state,
            "event_summaries": events,
            "audit_log": audit_log,
            "patterns": patterns,
            "recommendations": recommendations,
        }))
    }

    // --- patterns (Pattern Detection's Layer 1 output, ADR-0010) ---

    pub fn insert_pattern(&self, pattern: &Pattern) -> Result<i64, EventStoreError> {
        let conn = self.conn.lock().expect("event store mutex poisoned");
        conn.execute(
            "INSERT INTO patterns
                (first_seen_at, last_seen_at, occurrence_count,
                 estimated_minutes_per_occurrence, sequence_signature_json, status)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                to_rfc3339(pattern.first_seen_at),
                to_rfc3339(pattern.last_seen_at),
                pattern.occurrence_count,
                pattern.estimated_minutes_per_occurrence,
                serde_json::to_string(&pattern.sequence_signature)?,
                pattern_status_to_str(pattern.status),
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    /// Updates an existing pattern's rolling stats (occurrence count, last-seen
    /// time, estimate) as new matching events accumulate — the pattern's
    /// identity/signature does not change, only these fields.
    pub fn update_pattern_stats(
        &self,
        pattern_id: i64,
        last_seen_at: OffsetDateTime,
        occurrence_count: u32,
        estimated_minutes_per_occurrence: Option<f64>,
    ) -> Result<(), EventStoreError> {
        let conn = self.conn.lock().expect("event store mutex poisoned");
        conn.execute(
            "UPDATE patterns
             SET last_seen_at = ?1, occurrence_count = ?2, estimated_minutes_per_occurrence = ?3
             WHERE id = ?4",
            params![
                to_rfc3339(last_seen_at),
                occurrence_count,
                estimated_minutes_per_occurrence,
                pattern_id,
            ],
        )?;
        Ok(())
    }

    pub fn set_pattern_status(
        &self,
        pattern_id: i64,
        status: PatternStatus,
    ) -> Result<(), EventStoreError> {
        let conn = self.conn.lock().expect("event store mutex poisoned");
        conn.execute(
            "UPDATE patterns SET status = ?1 WHERE id = ?2",
            params![pattern_status_to_str(status), pattern_id],
        )?;
        Ok(())
    }

    pub fn list_patterns(
        &self,
        status_filter: Option<PatternStatus>,
    ) -> Result<Vec<Pattern>, EventStoreError> {
        let conn = self.conn.lock().expect("event store mutex poisoned");
        let base = "SELECT id, first_seen_at, last_seen_at, occurrence_count,
                    estimated_minutes_per_occurrence, sequence_signature_json, status
             FROM patterns";
        let (sql, filter): (String, Option<&'static str>) = match status_filter {
            Some(status) => (
                format!("{base} WHERE status = ?1"),
                Some(pattern_status_to_str(status)),
            ),
            None => (base.to_string(), None),
        };
        let mut stmt = conn.prepare(&sql)?;
        let rows = match filter {
            Some(status) => stmt.query_map(params![status], row_to_pattern)?,
            None => stmt.query_map([], row_to_pattern)?,
        };
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(EventStoreError::from)
    }

    /// Links a pattern to the specific event summaries that contributed to
    /// detecting it — the storage layer behind FR-13's "what observations
    /// contributed?" traceability requirement.
    pub fn link_pattern_events(
        &self,
        pattern_id: i64,
        event_ids: &[i64],
    ) -> Result<(), EventStoreError> {
        let conn = self.conn.lock().expect("event store mutex poisoned");
        for event_id in event_ids {
            conn.execute(
                "INSERT OR IGNORE INTO pattern_events (pattern_id, event_id) VALUES (?1, ?2)",
                params![pattern_id, event_id],
            )?;
        }
        Ok(())
    }

    pub fn list_pattern_events(
        &self,
        pattern_id: i64,
    ) -> Result<Vec<EventSummary>, EventStoreError> {
        let conn = self.conn.lock().expect("event store mutex poisoned");
        let mut stmt = conn.prepare(
            "SELECT e.id, e.occurred_at, e.source_id, e.signal_type, e.privacy_level_at_capture,
                    e.summary_json, e.is_deep_mode, e.ttl_expires_at
             FROM event_summaries e
             JOIN pattern_events pe ON pe.event_id = e.id
             WHERE pe.pattern_id = ?1
             ORDER BY e.occurred_at ASC",
        )?;
        let rows = stmt.query_map(params![pattern_id], row_to_event_summary)?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(EventStoreError::from)
    }

    // --- pattern embeddings (stands in for ADR-0007's sqlite-vec table — see
    // the comment at the top of schema.sql) ---

    pub fn upsert_pattern_embedding(
        &self,
        pattern_id: i64,
        embedding: &[f32],
    ) -> Result<(), EventStoreError> {
        let conn = self.conn.lock().expect("event store mutex poisoned");
        conn.execute(
            "INSERT INTO pattern_embeddings (pattern_id, embedding, dimensions)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(pattern_id) DO UPDATE SET embedding = excluded.embedding, dimensions = excluded.dimensions",
            params![pattern_id, encode_embedding(embedding), embedding.len() as i64],
        )?;
        Ok(())
    }

    /// Brute-force cosine-similarity search over every stored pattern embedding —
    /// see the file-level comment in `schema.sql` for why this is Rust-side
    /// rather than a native `vec0` virtual table, and why that's an honest
    /// implementation of ADR-0007's semantics rather than a shortcut around it.
    /// Returns `(pattern_id, similarity)` pairs, highest similarity first.
    pub fn find_similar_patterns(
        &self,
        query: &[f32],
        top_k: usize,
    ) -> Result<Vec<(i64, f32)>, EventStoreError> {
        let conn = self.conn.lock().expect("event store mutex poisoned");
        let mut stmt = conn.prepare("SELECT pattern_id, embedding FROM pattern_embeddings")?;
        let mut scored: Vec<(i64, f32)> = stmt
            .query_map([], |row| {
                let pattern_id: i64 = row.get(0)?;
                let blob: Vec<u8> = row.get(1)?;
                Ok((pattern_id, decode_embedding(&blob)))
            })?
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .map(|(id, vector)| (id, cosine_similarity(query, &vector)))
            .collect();
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(top_k);
        Ok(scored)
    }

    // --- recommendations (ADR-0010) ---

    pub fn insert_recommendation(&self, rec: &Recommendation) -> Result<i64, EventStoreError> {
        let conn = self.conn.lock().expect("event store mutex poisoned");
        conn.execute(
            "INSERT INTO recommendations
                (pattern_id, created_at, title, category, why, confidence,
                 estimated_time_saved_minutes, difficulty, maintenance_burden,
                 privacy_implications, implementation_effort, alternatives_json,
                 assumptions_json, ignored_information_json, generating_provider,
                 status, dismissal_reason)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
            params![
                rec.pattern_id,
                to_rfc3339(rec.created_at),
                rec.title,
                recommendation_category_to_str(rec.category),
                rec.why,
                rec.confidence,
                rec.estimated_time_saved_minutes,
                level_to_str(rec.difficulty),
                level_to_str(rec.maintenance_burden),
                rec.privacy_implications,
                rec.implementation_effort,
                serde_json::to_string(&rec.alternatives)?,
                serde_json::to_string(&rec.assumptions)?,
                serde_json::to_string(&rec.ignored_information)?,
                rec.generating_provider,
                recommendation_status_to_str(rec.status),
                rec.dismissal_reason,
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn set_recommendation_status(
        &self,
        id: i64,
        status: RecommendationStatus,
        dismissal_reason: Option<&str>,
    ) -> Result<(), EventStoreError> {
        let conn = self.conn.lock().expect("event store mutex poisoned");
        conn.execute(
            "UPDATE recommendations SET status = ?1, dismissal_reason = ?2 WHERE id = ?3",
            params![recommendation_status_to_str(status), dismissal_reason, id],
        )?;
        Ok(())
    }

    pub fn list_recommendations(
        &self,
        status_filter: Option<RecommendationStatus>,
    ) -> Result<Vec<Recommendation>, EventStoreError> {
        let conn = self.conn.lock().expect("event store mutex poisoned");
        let base = "SELECT id, pattern_id, created_at, title, category, why, confidence,
                    estimated_time_saved_minutes, difficulty, maintenance_burden,
                    privacy_implications, implementation_effort, alternatives_json,
                    assumptions_json, ignored_information_json, generating_provider,
                    status, dismissal_reason
             FROM recommendations";
        let (sql, filter): (String, Option<&'static str>) = match status_filter {
            Some(status) => (
                format!("{base} WHERE status = ?1"),
                Some(recommendation_status_to_str(status)),
            ),
            None => (base.to_string(), None),
        };
        let mut stmt = conn.prepare(&sql)?;
        let rows = match filter {
            Some(status) => stmt.query_map(params![status], row_to_recommendation)?,
            None => stmt.query_map([], row_to_recommendation)?,
        };
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(EventStoreError::from)
    }

    // --- LLM provider configuration (never the secret itself — see
    // `LlmProviderConfig::vault_key_ref`'s doc comment) ---

    pub fn upsert_llm_provider(&self, provider: &LlmProviderConfig) -> Result<(), EventStoreError> {
        let conn = self.conn.lock().expect("event store mutex poisoned");
        conn.execute(
            "INSERT INTO llm_providers (id, provider_type, is_local, model_name, endpoint, vault_key_ref, active)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(id) DO UPDATE SET
                provider_type = excluded.provider_type,
                is_local = excluded.is_local,
                model_name = excluded.model_name,
                endpoint = excluded.endpoint,
                vault_key_ref = excluded.vault_key_ref,
                active = excluded.active",
            params![
                provider.id,
                provider.provider_type,
                provider.is_local,
                provider.model_name,
                provider.endpoint,
                provider.vault_key_ref,
                provider.active,
            ],
        )?;
        Ok(())
    }

    pub fn list_llm_providers(&self) -> Result<Vec<LlmProviderConfig>, EventStoreError> {
        let conn = self.conn.lock().expect("event store mutex poisoned");
        let mut stmt = conn.prepare(
            "SELECT id, provider_type, is_local, model_name, endpoint, vault_key_ref, active FROM llm_providers",
        )?;
        let rows = stmt.query_map([], row_to_llm_provider)?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(EventStoreError::from)
    }

    /// Marks exactly one provider active, deactivating every other row —
    /// there is never more than one active provider at a time (the
    /// Recommendation Engine and Embedding Layer each read a single active
    /// provider, per ADR-0004).
    pub fn set_active_llm_provider(&self, provider_id: &str) -> Result<(), EventStoreError> {
        let conn = self.conn.lock().expect("event store mutex poisoned");
        let updated = conn.execute(
            "UPDATE llm_providers SET active = (id = ?1)",
            params![provider_id],
        )?;
        if updated == 0 {
            return Err(EventStoreError::InvalidStoredValue(format!(
                "no llm_providers row to update (provider '{provider_id}' not registered?)"
            )));
        }
        Ok(())
    }

    pub fn get_active_llm_provider(&self) -> Result<Option<LlmProviderConfig>, EventStoreError> {
        let conn = self.conn.lock().expect("event store mutex poisoned");
        conn.query_row(
            "SELECT id, provider_type, is_local, model_name, endpoint, vault_key_ref, active
             FROM llm_providers WHERE active = 1",
            [],
            row_to_llm_provider,
        )
        .optional()
        .map_err(EventStoreError::from)
    }

    // --- generic settings (UI complexity tier, notification frequency, ...) ---

    pub fn get_setting(&self, key: &str) -> Result<Option<serde_json::Value>, EventStoreError> {
        let conn = self.conn.lock().expect("event store mutex poisoned");
        let value: Option<String> = conn
            .query_row(
                "SELECT value_json FROM settings WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .optional()?;
        value
            .map(|v| serde_json::from_str(&v).map_err(EventStoreError::Serialization))
            .transpose()
    }

    pub fn set_setting(&self, key: &str, value: &serde_json::Value) -> Result<(), EventStoreError> {
        let conn = self.conn.lock().expect("event store mutex poisoned");
        conn.execute(
            "INSERT INTO settings (key, value_json) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json",
            params![key, serde_json::to_string(value)?],
        )?;
        Ok(())
    }

    // --- diagnostics support ---

    pub fn count_rows(&self, table: &str) -> Result<i64, EventStoreError> {
        // `table` is always one of a small set of hardcoded literals passed by
        // this crate's own callers (see `hiddensteps-desktop`'s diagnostics
        // command) — never user/network input — so building the identifier
        // into the SQL text directly is safe here; `rusqlite` has no
        // parameter-binding form for identifiers.
        const ALLOWED: &[&str] = &[
            "event_summaries",
            "patterns",
            "recommendations",
            "audit_log",
        ];
        if !ALLOWED.contains(&table) {
            return Err(EventStoreError::InvalidStoredValue(format!(
                "count_rows: '{table}' is not in the allowed table list"
            )));
        }
        let conn = self.conn.lock().expect("event store mutex poisoned");
        conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .map_err(EventStoreError::from)
    }
}

fn apply_key(conn: &Connection, key: &[u8; 32]) -> Result<(), EventStoreError> {
    conn.execute_batch(&format!("PRAGMA key = \"x'{}'\";", to_hex(key)))?;
    Ok(())
}

/// SQLCipher only rejects a wrong key once you actually try to read the database —
/// opening the connection itself always "succeeds." This runs a real query against
/// SQLite's own bookkeeping table to force that check immediately, rather than
/// deferring a confusing failure to the first caller-issued query.
fn verify_key(conn: &Connection) -> Result<(), EventStoreError> {
    conn.query_row("SELECT count(*) FROM sqlite_master", [], |row| {
        row.get::<_, i64>(0)
    })
    .map(|_| ())
    .map_err(|_| EventStoreError::InvalidKeyOrCorruptFile)
}

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn signal_type_to_str(signal_type: SignalType) -> &'static str {
    match signal_type {
        SignalType::AppFocusChange => "app_focus_change",
        SignalType::WindowTitle => "window_title",
        SignalType::ShortcutUsed => "shortcut_used",
        SignalType::AppActionEvent => "app_action_event",
        SignalType::BrowserDomainVisited => "browser_domain_visited",
        SignalType::ClipboardMetadata => "clipboard_metadata",
        SignalType::FileOperationMetadata => "file_operation_metadata",
        SignalType::OcrText => "ocr_text",
        SignalType::Screenshot => "screenshot",
        SignalType::AccessibilityTree => "accessibility_tree",
    }
}

fn signal_type_from_str(value: &str) -> Result<SignalType, EventStoreError> {
    Ok(match value {
        "app_focus_change" => SignalType::AppFocusChange,
        "window_title" => SignalType::WindowTitle,
        "shortcut_used" => SignalType::ShortcutUsed,
        "app_action_event" => SignalType::AppActionEvent,
        "browser_domain_visited" => SignalType::BrowserDomainVisited,
        "clipboard_metadata" => SignalType::ClipboardMetadata,
        "file_operation_metadata" => SignalType::FileOperationMetadata,
        "ocr_text" => SignalType::OcrText,
        "screenshot" => SignalType::Screenshot,
        "accessibility_tree" => SignalType::AccessibilityTree,
        other => {
            return Err(EventStoreError::InvalidStoredValue(format!(
                "unknown signal_type '{other}'"
            )))
        }
    })
}

fn actor_to_str(actor: AuditActor) -> &'static str {
    match actor {
        AuditActor::User => "user",
        AuditActor::System => "system",
    }
}

fn actor_from_str(value: &str) -> Result<AuditActor, EventStoreError> {
    match value {
        "user" => Ok(AuditActor::User),
        "system" => Ok(AuditActor::System),
        other => Err(EventStoreError::InvalidStoredValue(format!(
            "unknown actor '{other}'"
        ))),
    }
}

fn row_to_event_summary(row: &rusqlite::Row<'_>) -> rusqlite::Result<EventSummary> {
    let id: i64 = row.get(0)?;
    let occurred_at: String = row.get(1)?;
    let source_id: String = row.get(2)?;
    let signal_type: String = row.get(3)?;
    let privacy_level: i64 = row.get(4)?;
    let summary_json: String = row.get(5)?;
    let is_deep_mode: i64 = row.get(6)?;
    let ttl_expires_at: Option<String> = row.get(7)?;

    let map_err = |e: EventStoreError| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
    };

    Ok(EventSummary {
        id: Some(id),
        occurred_at: from_rfc3339(&occurred_at).map_err(map_err)?,
        source_id,
        signal_type: signal_type_from_str(&signal_type).map_err(map_err)?,
        privacy_level_at_capture: PrivacyLevel::from_u8(privacy_level as u8)
            .map_err(|e| map_err(EventStoreError::InvalidStoredValue(e.to_string())))?,
        summary: serde_json::from_str(&summary_json)
            .map_err(|e| map_err(EventStoreError::Serialization(e)))?,
        is_deep_mode: is_deep_mode != 0,
        ttl_expires_at: ttl_expires_at
            .map(|s| from_rfc3339(&s))
            .transpose()
            .map_err(map_err)?,
    })
}

fn row_to_audit_entry(row: &rusqlite::Row<'_>) -> rusqlite::Result<AuditEntry> {
    let id: i64 = row.get(0)?;
    let occurred_at: String = row.get(1)?;
    let actor: String = row.get(2)?;
    let action_type: String = row.get(3)?;
    let details_json: String = row.get(4)?;

    let map_err = |e: EventStoreError| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
    };

    Ok(AuditEntry {
        id: Some(id),
        occurred_at: from_rfc3339(&occurred_at).map_err(map_err)?,
        actor: actor_from_str(&actor).map_err(map_err)?,
        action_type,
        details: serde_json::from_str(&details_json)
            .map_err(|e| map_err(EventStoreError::Serialization(e)))?,
    })
}

fn pattern_status_to_str(status: PatternStatus) -> &'static str {
    match status {
        PatternStatus::Active => "active",
        PatternStatus::Stale => "stale",
        PatternStatus::Dismissed => "dismissed",
    }
}

fn pattern_status_from_str(value: &str) -> Result<PatternStatus, EventStoreError> {
    match value {
        "active" => Ok(PatternStatus::Active),
        "stale" => Ok(PatternStatus::Stale),
        "dismissed" => Ok(PatternStatus::Dismissed),
        other => Err(EventStoreError::InvalidStoredValue(format!(
            "unknown pattern status '{other}'"
        ))),
    }
}

fn level_to_str(level: Level) -> &'static str {
    match level {
        Level::Low => "low",
        Level::Medium => "medium",
        Level::High => "high",
    }
}

fn level_from_str(value: &str) -> Result<Level, EventStoreError> {
    match value {
        "low" => Ok(Level::Low),
        "medium" => Ok(Level::Medium),
        "high" => Ok(Level::High),
        other => Err(EventStoreError::InvalidStoredValue(format!(
            "unknown level '{other}'"
        ))),
    }
}

fn recommendation_category_to_str(category: RecommendationCategory) -> &'static str {
    match category {
        RecommendationCategory::Shortcut => "shortcut",
        RecommendationCategory::Template => "template",
        RecommendationCategory::Script => "script",
        RecommendationCategory::BrowserAutomation => "browser_automation",
        RecommendationCategory::Rpa => "rpa",
        RecommendationCategory::WorkflowPlatform => "workflow_platform",
        RecommendationCategory::AiAgent => "ai_agent",
        RecommendationCategory::Hybrid => "hybrid",
    }
}

fn recommendation_category_from_str(
    value: &str,
) -> Result<RecommendationCategory, EventStoreError> {
    match value {
        "shortcut" => Ok(RecommendationCategory::Shortcut),
        "template" => Ok(RecommendationCategory::Template),
        "script" => Ok(RecommendationCategory::Script),
        "browser_automation" => Ok(RecommendationCategory::BrowserAutomation),
        "rpa" => Ok(RecommendationCategory::Rpa),
        "workflow_platform" => Ok(RecommendationCategory::WorkflowPlatform),
        "ai_agent" => Ok(RecommendationCategory::AiAgent),
        "hybrid" => Ok(RecommendationCategory::Hybrid),
        other => Err(EventStoreError::InvalidStoredValue(format!(
            "unknown recommendation category '{other}'"
        ))),
    }
}

fn recommendation_status_to_str(status: RecommendationStatus) -> &'static str {
    match status {
        RecommendationStatus::Suggested => "suggested",
        RecommendationStatus::Implemented => "implemented",
        RecommendationStatus::Dismissed => "dismissed",
    }
}

fn recommendation_status_from_str(value: &str) -> Result<RecommendationStatus, EventStoreError> {
    match value {
        "suggested" => Ok(RecommendationStatus::Suggested),
        "implemented" => Ok(RecommendationStatus::Implemented),
        "dismissed" => Ok(RecommendationStatus::Dismissed),
        other => Err(EventStoreError::InvalidStoredValue(format!(
            "unknown recommendation status '{other}'"
        ))),
    }
}

fn encode_embedding(vector: &[f32]) -> Vec<u8> {
    vector.iter().flat_map(|f| f.to_le_bytes()).collect()
}

fn decode_embedding(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

/// Cosine similarity, in `[-1.0, 1.0]`; `0.0` for a zero-length or zero-norm
/// vector rather than dividing by zero — an embedding provider should never
/// actually produce an all-zero vector, but this must not panic if one somehow
/// appears.
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    dot / (norm_a * norm_b)
}

fn row_to_pattern(row: &rusqlite::Row<'_>) -> rusqlite::Result<Pattern> {
    let id: i64 = row.get(0)?;
    let first_seen_at: String = row.get(1)?;
    let last_seen_at: String = row.get(2)?;
    let occurrence_count: i64 = row.get(3)?;
    let estimated_minutes_per_occurrence: Option<f64> = row.get(4)?;
    let sequence_signature_json: String = row.get(5)?;
    let status: String = row.get(6)?;

    let map_err = |e: EventStoreError| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
    };

    Ok(Pattern {
        id: Some(id),
        first_seen_at: from_rfc3339(&first_seen_at).map_err(map_err)?,
        last_seen_at: from_rfc3339(&last_seen_at).map_err(map_err)?,
        occurrence_count: occurrence_count as u32,
        estimated_minutes_per_occurrence,
        sequence_signature: serde_json::from_str(&sequence_signature_json)
            .map_err(|e| map_err(EventStoreError::Serialization(e)))?,
        status: pattern_status_from_str(&status).map_err(map_err)?,
    })
}

fn row_to_recommendation(row: &rusqlite::Row<'_>) -> rusqlite::Result<Recommendation> {
    let map_err = |e: EventStoreError| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
    };
    let parse_json = |s: &str| -> rusqlite::Result<serde_json::Value> {
        serde_json::from_str(s).map_err(|e| map_err(EventStoreError::Serialization(e)))
    };

    let id: i64 = row.get(0)?;
    let pattern_id: i64 = row.get(1)?;
    let created_at: String = row.get(2)?;
    let title: String = row.get(3)?;
    let category: String = row.get(4)?;
    let why: String = row.get(5)?;
    let confidence: f64 = row.get(6)?;
    let estimated_time_saved_minutes: f64 = row.get(7)?;
    let difficulty: String = row.get(8)?;
    let maintenance_burden: String = row.get(9)?;
    let privacy_implications: String = row.get(10)?;
    let implementation_effort: String = row.get(11)?;
    let alternatives_json: String = row.get(12)?;
    let assumptions_json: String = row.get(13)?;
    let ignored_information_json: String = row.get(14)?;
    let generating_provider: String = row.get(15)?;
    let status: String = row.get(16)?;
    let dismissal_reason: Option<String> = row.get(17)?;

    let alternatives: Vec<Alternative> = serde_json::from_value(parse_json(&alternatives_json)?)
        .map_err(|e| map_err(EventStoreError::Serialization(e)))?;
    let assumptions: Vec<String> = serde_json::from_value(parse_json(&assumptions_json)?)
        .map_err(|e| map_err(EventStoreError::Serialization(e)))?;
    let ignored_information: Vec<String> =
        serde_json::from_value(parse_json(&ignored_information_json)?)
            .map_err(|e| map_err(EventStoreError::Serialization(e)))?;

    Ok(Recommendation {
        id: Some(id),
        pattern_id,
        created_at: from_rfc3339(&created_at).map_err(map_err)?,
        title,
        category: recommendation_category_from_str(&category).map_err(map_err)?,
        why,
        confidence: confidence as f32,
        estimated_time_saved_minutes,
        difficulty: level_from_str(&difficulty).map_err(map_err)?,
        maintenance_burden: level_from_str(&maintenance_burden).map_err(map_err)?,
        privacy_implications,
        implementation_effort,
        alternatives,
        assumptions,
        ignored_information,
        generating_provider,
        status: recommendation_status_from_str(&status).map_err(map_err)?,
        dismissal_reason,
    })
}

fn row_to_llm_provider(row: &rusqlite::Row<'_>) -> rusqlite::Result<LlmProviderConfig> {
    Ok(LlmProviderConfig {
        id: row.get(0)?,
        provider_type: row.get(1)?,
        is_local: row.get(2)?,
        model_name: row.get(3)?,
        endpoint: row.get(4)?,
        vault_key_ref: row.get(5)?,
        active: row.get(6)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use hiddensteps_domain::PrivacyState;

    fn test_key() -> [u8; 32] {
        [0x42; 32]
    }

    #[test]
    fn fresh_store_starts_at_level_zero_with_observation_off() {
        let store = SqlCipherEventStore::open_in_memory(&test_key()).unwrap();
        let state = store.get_privacy_state().unwrap();
        assert_eq!(state.current_level, PrivacyLevel::Manual);
        assert!(!state.observation_active);
    }

    #[test]
    fn wrong_key_fails_to_open_an_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("profile.db");

        SqlCipherEventStore::open(&path, &[0x11; 32]).unwrap();
        // Drop happens implicitly; re-open the same file with a different key.
        let reopened = SqlCipherEventStore::open(&path, &[0x22; 32]);
        assert!(matches!(
            reopened,
            Err(EventStoreError::InvalidKeyOrCorruptFile)
        ));
    }

    #[test]
    fn same_key_reopens_the_same_file_successfully() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("profile.db");
        let key = test_key();

        {
            let store = SqlCipherEventStore::open(&path, &key).unwrap();
            store
                .set_privacy_state(&PrivacyState {
                    current_level: PrivacyLevel::WorkflowMetadata,
                    consented_manifest_version: 3,
                    observation_active: true,
                    updated_at: OffsetDateTime::now_utc(),
                })
                .unwrap();
        }

        let reopened = SqlCipherEventStore::open(&path, &key).unwrap();
        let state = reopened.get_privacy_state().unwrap();
        assert_eq!(state.current_level, PrivacyLevel::WorkflowMetadata);
        assert_eq!(state.consented_manifest_version, 3);
        assert!(state.observation_active);
    }

    #[test]
    fn inserted_events_come_back_newest_first() {
        let store = SqlCipherEventStore::open_in_memory(&test_key()).unwrap();
        let base = OffsetDateTime::now_utc();

        for (i, app) in ["Terminal", "Browser", "Editor"].iter().enumerate() {
            let event = EventSummary::new(
                base + time::Duration::minutes(i as i64),
                "linux.app_focus",
                SignalType::AppFocusChange,
                PrivacyLevel::ApplicationMetadata,
                serde_json::json!({ "app": app }),
                None,
            );
            store.insert_event_summary(&event).unwrap();
        }

        let recent = store.list_recent_events(10).unwrap();
        assert_eq!(recent.len(), 3);
        assert_eq!(recent[0].summary["app"], "Editor");
        assert_eq!(recent[2].summary["app"], "Terminal");
    }

    #[test]
    fn recent_events_feed_never_shows_more_than_was_stored() {
        // Directly exercises the docs/design/04-trust-model.md §2 claim: the
        // dashboard's feed must show exactly what's in the store, nothing more,
        // nothing paraphrased.
        let store = SqlCipherEventStore::open_in_memory(&test_key()).unwrap();
        let event = EventSummary::new(
            OffsetDateTime::now_utc(),
            "macos.browser",
            SignalType::BrowserDomainVisited,
            PrivacyLevel::WorkflowMetadata,
            serde_json::json!({ "domain": "github.com" }),
            None,
        );
        let id = store.insert_event_summary(&event).unwrap();

        let recent = store.list_recent_events(10).unwrap();
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].id, Some(id));
        assert_eq!(recent[0].summary, event.summary);
        assert_eq!(recent[0].signal_type, event.signal_type);
        assert_eq!(
            recent[0].privacy_level_at_capture,
            event.privacy_level_at_capture
        );
    }

    #[test]
    fn delete_events_removes_only_the_requested_ids() {
        let store = SqlCipherEventStore::open_in_memory(&test_key()).unwrap();
        let mut ids = Vec::new();
        for i in 0..3 {
            let event = EventSummary::new(
                OffsetDateTime::now_utc() + time::Duration::seconds(i),
                "src",
                SignalType::AppFocusChange,
                PrivacyLevel::ApplicationMetadata,
                serde_json::json!({ "n": i }),
                None,
            );
            ids.push(store.insert_event_summary(&event).unwrap());
        }

        let deleted = store.delete_events(&ids[0..1]).unwrap();
        assert_eq!(deleted, 1);

        let remaining = store.list_recent_events(10).unwrap();
        assert_eq!(remaining.len(), 2);
        assert!(remaining.iter().all(|e| e.id != Some(ids[0])));
    }

    #[test]
    fn audit_log_round_trips_actor_and_action_type() {
        let store = SqlCipherEventStore::open_in_memory(&test_key()).unwrap();
        let entry = AuditEntry::new(
            AuditActor::User,
            "privacy_level_changed",
            serde_json::json!({ "from": 0, "to": 1 }),
        );
        store.append_audit_entry(&entry).unwrap();

        let log = store.list_audit_log(10).unwrap();
        assert_eq!(log.len(), 1);
        assert_eq!(log[0].actor, AuditActor::User);
        assert_eq!(log[0].action_type, "privacy_level_changed");
        assert_eq!(log[0].details, entry.details);
    }

    #[test]
    fn delete_all_data_clears_events_audit_log_and_resets_privacy_state() {
        let store = SqlCipherEventStore::open_in_memory(&test_key()).unwrap();

        store
            .insert_event_summary(&EventSummary::new(
                OffsetDateTime::now_utc(),
                "src",
                SignalType::AppFocusChange,
                PrivacyLevel::ApplicationMetadata,
                serde_json::json!({}),
                None,
            ))
            .unwrap();
        store
            .append_audit_entry(&AuditEntry::new(
                AuditActor::User,
                "observation_started",
                serde_json::json!({}),
            ))
            .unwrap();
        store
            .set_privacy_state(&PrivacyState {
                current_level: PrivacyLevel::ContextAware,
                consented_manifest_version: 2,
                observation_active: true,
                updated_at: OffsetDateTime::now_utc(),
            })
            .unwrap();

        store.delete_all_data().unwrap();

        assert!(store.list_recent_events(10).unwrap().is_empty());
        assert!(store.list_audit_log(10).unwrap().is_empty());
        let state = store.get_privacy_state().unwrap();
        assert_eq!(state.current_level, PrivacyLevel::Manual);
        assert!(!state.observation_active);
    }

    #[test]
    fn export_data_includes_events_audit_log_and_privacy_state() {
        let store = SqlCipherEventStore::open_in_memory(&test_key()).unwrap();
        store
            .insert_event_summary(&EventSummary::new(
                OffsetDateTime::now_utc(),
                "src",
                SignalType::AppFocusChange,
                PrivacyLevel::ApplicationMetadata,
                serde_json::json!({ "app": "Terminal" }),
                None,
            ))
            .unwrap();

        let export = store.export_data().unwrap();
        assert_eq!(export["event_summaries"].as_array().unwrap().len(), 1);
        assert!(export["privacy_state"].is_object());
        assert!(export["audit_log"].is_array());
    }

    fn sample_pattern() -> Pattern {
        Pattern {
            id: None,
            first_seen_at: OffsetDateTime::now_utc(),
            last_seen_at: OffsetDateTime::now_utc(),
            occurrence_count: 31,
            estimated_minutes_per_occurrence: Some(21.3),
            sequence_signature: serde_json::json!(["jira", "clipboard", "excel", "save"]),
            status: PatternStatus::Active,
        }
    }

    #[test]
    fn pattern_round_trips_through_insert_and_list() {
        let store = SqlCipherEventStore::open_in_memory(&test_key()).unwrap();
        let id = store.insert_pattern(&sample_pattern()).unwrap();

        let patterns = store.list_patterns(None).unwrap();
        assert_eq!(patterns.len(), 1);
        assert_eq!(patterns[0].id, Some(id));
        assert_eq!(patterns[0].occurrence_count, 31);
        assert_eq!(patterns[0].status, PatternStatus::Active);
    }

    #[test]
    fn update_pattern_stats_changes_only_the_rolling_fields() {
        let store = SqlCipherEventStore::open_in_memory(&test_key()).unwrap();
        let id = store.insert_pattern(&sample_pattern()).unwrap();
        let new_last_seen = OffsetDateTime::now_utc() + time::Duration::days(1);

        store
            .update_pattern_stats(id, new_last_seen, 32, Some(21.5))
            .unwrap();

        let patterns = store.list_patterns(None).unwrap();
        assert_eq!(patterns[0].occurrence_count, 32);
        assert_eq!(patterns[0].estimated_minutes_per_occurrence, Some(21.5));
    }

    #[test]
    fn set_pattern_status_filters_list_patterns() {
        let store = SqlCipherEventStore::open_in_memory(&test_key()).unwrap();
        let id = store.insert_pattern(&sample_pattern()).unwrap();
        store
            .set_pattern_status(id, PatternStatus::Dismissed)
            .unwrap();

        assert!(store
            .list_patterns(Some(PatternStatus::Active))
            .unwrap()
            .is_empty());
        assert_eq!(
            store
                .list_patterns(Some(PatternStatus::Dismissed))
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn pattern_events_traceability_returns_the_contributing_observations() {
        let store = SqlCipherEventStore::open_in_memory(&test_key()).unwrap();
        let pattern_id = store.insert_pattern(&sample_pattern()).unwrap();

        let event_id = store
            .insert_event_summary(&EventSummary::new(
                OffsetDateTime::now_utc(),
                "src",
                SignalType::AppFocusChange,
                PrivacyLevel::ApplicationMetadata,
                serde_json::json!({ "app": "Jira" }),
                None,
            ))
            .unwrap();
        store.link_pattern_events(pattern_id, &[event_id]).unwrap();

        let contributing = store.list_pattern_events(pattern_id).unwrap();
        assert_eq!(contributing.len(), 1);
        assert_eq!(contributing[0].id, Some(event_id));
    }

    #[test]
    fn find_similar_patterns_ranks_by_cosine_similarity() {
        let store = SqlCipherEventStore::open_in_memory(&test_key()).unwrap();
        let close_id = store.insert_pattern(&sample_pattern()).unwrap();
        let far_id = store.insert_pattern(&sample_pattern()).unwrap();

        store
            .upsert_pattern_embedding(close_id, &[1.0, 0.0, 0.0])
            .unwrap();
        store
            .upsert_pattern_embedding(far_id, &[0.0, 1.0, 0.0])
            .unwrap();

        let results = store.find_similar_patterns(&[0.9, 0.1, 0.0], 2).unwrap();
        assert_eq!(results[0].0, close_id);
        assert!(results[0].1 > results[1].1);
    }

    #[test]
    fn upserting_an_embedding_twice_replaces_it_rather_than_duplicating() {
        let store = SqlCipherEventStore::open_in_memory(&test_key()).unwrap();
        let id = store.insert_pattern(&sample_pattern()).unwrap();
        store.upsert_pattern_embedding(id, &[1.0, 0.0]).unwrap();
        store.upsert_pattern_embedding(id, &[0.0, 1.0]).unwrap();

        let results = store.find_similar_patterns(&[0.0, 1.0], 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].1, 1.0);
    }

    fn sample_recommendation(pattern_id: i64) -> Recommendation {
        Recommendation {
            id: None,
            pattern_id,
            created_at: OffsetDateTime::now_utc(),
            title: "Automate the weekly ticket export".to_string(),
            category: RecommendationCategory::Hybrid,
            why: "This exact sequence recurs with high regularity.".to_string(),
            confidence: 0.9,
            estimated_time_saved_minutes: 660.0,
            difficulty: Level::Medium,
            maintenance_burden: Level::Low,
            privacy_implications: "Fully local, no cloud dispatch required.".to_string(),
            implementation_effort: "~2-3 hours one-time setup".to_string(),
            alternatives: vec![Alternative {
                approach: "Python script".to_string(),
                tradeoff: "Lower setup effort, higher long-term maintenance.".to_string(),
            }],
            assumptions: vec!["API access to the source system is available.".to_string()],
            ignored_information: vec![
                "Occurrences on a second device were not correlated.".to_string()
            ],
            generating_provider: "ollama".to_string(),
            status: RecommendationStatus::Suggested,
            dismissal_reason: None,
        }
    }

    #[test]
    fn recommendation_round_trips_every_explainability_field() {
        let store = SqlCipherEventStore::open_in_memory(&test_key()).unwrap();
        let pattern_id = store.insert_pattern(&sample_pattern()).unwrap();
        let rec = sample_recommendation(pattern_id);
        store.insert_recommendation(&rec).unwrap();

        let recommendations = store.list_recommendations(None).unwrap();
        assert_eq!(recommendations.len(), 1);
        let stored = &recommendations[0];
        assert_eq!(stored.title, rec.title);
        assert_eq!(stored.category, rec.category);
        assert_eq!(stored.why, rec.why);
        assert_eq!(stored.confidence, rec.confidence);
        assert_eq!(stored.alternatives, rec.alternatives);
        assert_eq!(stored.assumptions, rec.assumptions);
        assert_eq!(stored.ignored_information, rec.ignored_information);
    }

    #[test]
    fn set_recommendation_status_records_dismissal_reason() {
        let store = SqlCipherEventStore::open_in_memory(&test_key()).unwrap();
        let pattern_id = store.insert_pattern(&sample_pattern()).unwrap();
        let id = store
            .insert_recommendation(&sample_recommendation(pattern_id))
            .unwrap();

        store
            .set_recommendation_status(
                id,
                RecommendationStatus::Dismissed,
                Some("not worth the effort"),
            )
            .unwrap();

        let recommendations = store.list_recommendations(None).unwrap();
        assert_eq!(recommendations[0].status, RecommendationStatus::Dismissed);
        assert_eq!(
            recommendations[0].dismissal_reason.as_deref(),
            Some("not worth the effort")
        );
    }

    #[test]
    fn delete_all_data_also_clears_patterns_embeddings_and_recommendations() {
        let store = SqlCipherEventStore::open_in_memory(&test_key()).unwrap();
        let pattern_id = store.insert_pattern(&sample_pattern()).unwrap();
        store
            .upsert_pattern_embedding(pattern_id, &[1.0, 0.0])
            .unwrap();
        store
            .insert_recommendation(&sample_recommendation(pattern_id))
            .unwrap();

        store.delete_all_data().unwrap();

        assert!(store.list_patterns(None).unwrap().is_empty());
        assert!(store.list_recommendations(None).unwrap().is_empty());
        assert!(store
            .find_similar_patterns(&[1.0, 0.0], 10)
            .unwrap()
            .is_empty());
    }

    fn sample_provider(id: &str, active: bool) -> LlmProviderConfig {
        LlmProviderConfig {
            id: id.to_string(),
            provider_type: "ollama".to_string(),
            is_local: true,
            model_name: Some("qwen3:0.6b".to_string()),
            endpoint: Some("http://localhost:11434".to_string()),
            vault_key_ref: None,
            active,
        }
    }

    #[test]
    fn llm_provider_round_trips_and_lists() {
        let store = SqlCipherEventStore::open_in_memory(&test_key()).unwrap();
        store
            .upsert_llm_provider(&sample_provider("ollama-local", true))
            .unwrap();

        let providers = store.list_llm_providers().unwrap();
        assert_eq!(providers.len(), 1);
        assert_eq!(providers[0].id, "ollama-local");
        assert!(providers[0].active);
    }

    #[test]
    fn only_one_llm_provider_is_ever_active_at_once() {
        let store = SqlCipherEventStore::open_in_memory(&test_key()).unwrap();
        store
            .upsert_llm_provider(&sample_provider("ollama-local", true))
            .unwrap();
        store
            .upsert_llm_provider(&sample_provider("openai-cloud", false))
            .unwrap();

        store.set_active_llm_provider("openai-cloud").unwrap();

        let active = store.get_active_llm_provider().unwrap().unwrap();
        assert_eq!(active.id, "openai-cloud");
        let providers = store.list_llm_providers().unwrap();
        assert_eq!(providers.iter().filter(|p| p.active).count(), 1);
    }

    #[test]
    fn set_active_llm_provider_on_an_unregistered_id_errors_rather_than_silently_no_op() {
        let store = SqlCipherEventStore::open_in_memory(&test_key()).unwrap();
        assert!(store.set_active_llm_provider("does-not-exist").is_err());
    }

    #[test]
    fn get_active_llm_provider_is_none_when_nothing_is_registered() {
        let store = SqlCipherEventStore::open_in_memory(&test_key()).unwrap();
        assert_eq!(store.get_active_llm_provider().unwrap(), None);
    }

    #[test]
    fn upserting_a_provider_twice_updates_rather_than_duplicates() {
        let store = SqlCipherEventStore::open_in_memory(&test_key()).unwrap();
        store
            .upsert_llm_provider(&sample_provider("ollama-local", true))
            .unwrap();
        let mut updated = sample_provider("ollama-local", true);
        updated.model_name = Some("llama3.1:8b".to_string());
        store.upsert_llm_provider(&updated).unwrap();

        let providers = store.list_llm_providers().unwrap();
        assert_eq!(providers.len(), 1);
        assert_eq!(providers[0].model_name, Some("llama3.1:8b".to_string()));
    }

    #[test]
    fn setting_round_trips_arbitrary_json() {
        let store = SqlCipherEventStore::open_in_memory(&test_key()).unwrap();
        assert_eq!(store.get_setting("ui.complexity_tier").unwrap(), None);

        store
            .set_setting("ui.complexity_tier", &serde_json::json!("intermediate"))
            .unwrap();
        assert_eq!(
            store.get_setting("ui.complexity_tier").unwrap(),
            Some(serde_json::json!("intermediate"))
        );

        // Overwrite, not duplicate.
        store
            .set_setting("ui.complexity_tier", &serde_json::json!("advanced"))
            .unwrap();
        assert_eq!(
            store.get_setting("ui.complexity_tier").unwrap(),
            Some(serde_json::json!("advanced"))
        );
    }

    #[test]
    fn count_rows_reflects_real_inserted_data() {
        let store = SqlCipherEventStore::open_in_memory(&test_key()).unwrap();
        assert_eq!(store.count_rows("event_summaries").unwrap(), 0);
        store
            .insert_event_summary(&EventSummary::new(
                OffsetDateTime::now_utc(),
                "src",
                SignalType::AppFocusChange,
                PrivacyLevel::ApplicationMetadata,
                serde_json::json!({ "app": "Terminal" }),
                None,
            ))
            .unwrap();
        assert_eq!(store.count_rows("event_summaries").unwrap(), 1);
    }

    #[test]
    fn count_rows_rejects_a_table_name_outside_the_allowlist() {
        let store = SqlCipherEventStore::open_in_memory(&test_key()).unwrap();
        assert!(store.count_rows("sqlite_master").is_err());
    }
}
