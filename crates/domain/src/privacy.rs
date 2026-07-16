use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

/// The five observation privacy levels, per `docs/design/05-privacy-model.md` §1.
///
/// Ordering matters: `PrivacyLevel` derives `PartialOrd`/`Ord` so callers can write
/// `level >= PrivacyLevel::ContextAware` to gate behavior, mirroring how the spec
/// describes each level as a superset of the one below it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum PrivacyLevel {
    /// Level 0 — no `ObservationSource` plugin is active.
    Manual = 0,
    /// Level 1 — active app, window title, focus timestamps, shortcut invocation.
    ApplicationMetadata = 1,
    /// Level 2 — + app action events, browser domain, clipboard/file-op metadata.
    WorkflowMetadata = 2,
    /// Level 3 — + fuller in-app action context, browser page title, file-op context.
    ContextAware = 3,
    /// Level 4 — + explicit opt-in OCR / screenshot / accessibility-tree capture.
    MaximumAssistance = 4,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum PrivacyLevelError {
    #[error("{0} is not a valid privacy level (expected 0-4)")]
    OutOfRange(u8),
}

impl PrivacyLevel {
    pub const ALL: [PrivacyLevel; 5] = [
        PrivacyLevel::Manual,
        PrivacyLevel::ApplicationMetadata,
        PrivacyLevel::WorkflowMetadata,
        PrivacyLevel::ContextAware,
        PrivacyLevel::MaximumAssistance,
    ];

    pub fn as_u8(self) -> u8 {
        self as u8
    }

    pub fn from_u8(value: u8) -> Result<Self, PrivacyLevelError> {
        Self::ALL
            .into_iter()
            .find(|level| level.as_u8() == value)
            .ok_or(PrivacyLevelError::OutOfRange(value))
    }

    /// Whether content captured at this level may ever be dispatched to a
    /// cloud `LlmProvider`, per `docs/design/05-privacy-model.md` §3.
    ///
    /// This is a hard architectural rule, not a configurable setting — it is
    /// deliberately not parameterized by any caller-supplied flag.
    pub fn is_cloud_eligible(self) -> bool {
        self != PrivacyLevel::MaximumAssistance
    }

    /// Whether this level's capture requires Deep-mode TTL sweeping
    /// (`docs/design/05-privacy-model.md` §2).
    pub fn requires_ttl_sweep(self) -> bool {
        self == PrivacyLevel::MaximumAssistance
    }
}

/// The singleton row from the `privacy_state` table
/// (`docs/design/07-database-schema.md`) — current level, which manifest version
/// the user consented to, and whether observation is currently active.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PrivacyState {
    pub current_level: PrivacyLevel,
    pub consented_manifest_version: i64,
    pub observation_active: bool,
    pub updated_at: OffsetDateTime,
}

impl PrivacyState {
    /// The state a brand-new profile starts in: Level 0 (Manual), nothing consented
    /// yet, observation off — per FR-2, observation is off until onboarding
    /// completes and the user explicitly starts it.
    pub fn initial(now: OffsetDateTime) -> Self {
        Self {
            current_level: PrivacyLevel::Manual,
            consented_manifest_version: 0,
            observation_active: false,
            updated_at: now,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_u8() {
        for level in PrivacyLevel::ALL {
            assert_eq!(PrivacyLevel::from_u8(level.as_u8()), Ok(level));
        }
    }

    #[test]
    fn rejects_out_of_range_values() {
        assert_eq!(
            PrivacyLevel::from_u8(5),
            Err(PrivacyLevelError::OutOfRange(5))
        );
        assert_eq!(
            PrivacyLevel::from_u8(255),
            Err(PrivacyLevelError::OutOfRange(255))
        );
    }

    #[test]
    fn orders_from_least_to_most_invasive() {
        assert!(PrivacyLevel::Manual < PrivacyLevel::ApplicationMetadata);
        assert!(PrivacyLevel::ApplicationMetadata < PrivacyLevel::WorkflowMetadata);
        assert!(PrivacyLevel::WorkflowMetadata < PrivacyLevel::ContextAware);
        assert!(PrivacyLevel::ContextAware < PrivacyLevel::MaximumAssistance);
    }

    #[test]
    fn only_maximum_assistance_is_cloud_ineligible() {
        for level in PrivacyLevel::ALL {
            assert_eq!(
                level.is_cloud_eligible(),
                level != PrivacyLevel::MaximumAssistance
            );
        }
    }
}
