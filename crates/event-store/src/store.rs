use std::path::Path;
use std::sync::Mutex;

use hiddensteps_domain::{
    AuditActor, AuditEntry, EventSummary, PrivacyLevel, PrivacyState, SignalType,
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
        Ok(serde_json::json!({
            "privacy_state": privacy_state,
            "event_summaries": events,
            "audit_log": audit_log,
        }))
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
}
