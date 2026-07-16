use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use crate::EventStoreError;

/// All timestamp columns in the schema are stored as RFC 3339 text (per the `TEXT`
/// columns in `docs/design/07-database-schema.md`), not SQLite's numeric `julianday`
/// — this keeps the raw database file human-inspectable, which matters for the
/// "open, inspectable local storage" trust claim in `docs/design/04-trust-model.md` §4.
pub fn to_rfc3339(dt: OffsetDateTime) -> String {
    dt.format(&Rfc3339)
        .expect("OffsetDateTime -> RFC3339 formatting cannot fail for in-range dates")
}

pub fn from_rfc3339(text: &str) -> Result<OffsetDateTime, EventStoreError> {
    OffsetDateTime::parse(text, &Rfc3339).map_err(|e| EventStoreError::Timestamp(e.to_string()))
}
