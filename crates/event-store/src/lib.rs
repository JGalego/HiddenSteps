//! The single encrypted SQLite (SQLCipher) store, per ADR-0003 and
//! `docs/design/07-database-schema.md`.
//!
//! Everything in this crate operates on `hiddensteps_domain::EventSummary` and
//! `AuditEntry` — never on `CapturedSignal`. There is no `insert_raw_signal` method
//! anywhere here; that omission is deliberate (see `CapturedSignal`'s doc comment
//! in the domain crate for why).

mod error;
mod store;
mod time_fmt;

pub use error::EventStoreError;
pub use store::SqlCipherEventStore;
