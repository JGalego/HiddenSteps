//! The Enterprise Policy Engine, per `docs/design/05-privacy-model.md` §6 and
//! `docs/design/08-plugin-architecture.md` §6.
//!
//! `EnterprisePolicy`'s field list **is** the enforcement mechanism for what a
//! policy pack can and can't constrain — not a convention layered on top of a
//! more permissive schema. It has exactly two knobs (a privacy-level floor, a
//! provider allowlist), matching the two things `docs/design/05-privacy-model.md`
//! §6 says policy may set. There is no field for a redaction-confidence
//! threshold, no field to disable the Level-4-never-cloud-eligible rule, no field
//! to hide a trust-dashboard feature, no field to extend retention or disable
//! deletion — those rules are enforced entirely inside
//! `hiddensteps-privacy-engine`, `hiddensteps-redaction`, and
//! `hiddensteps-event-store` respectively, none of which take an
//! `EnterprisePolicy` as input at all. A policy author cannot request any of
//! those things because there is nowhere in this schema to write the request,
//! and even if a raw JSON policy file contains such keys (see
//! `parse_ignores_fields_outside_the_allowed_schema` below), nothing anywhere in
//! the codebase ever reads them back out of this type.

use hiddensteps_domain::{PrivacyLevel, PrivacyLevelError};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EnterprisePolicy {
    /// Free-text identifier of where this policy came from (a file path, an MDM
    /// profile id, ...) — shown in Diagnostics per
    /// `docs/design/06-security-architecture.md` §5, never used for enforcement.
    pub policy_source: Option<String>,
    /// The minimum privacy level a device must run at, if observation is active
    /// at all. `None` means no floor is imposed.
    pub privacy_level_floor: Option<u8>,
    /// If present, only these provider ids may be selected/activated. `None`
    /// means no restriction (any provider the user configures is allowed).
    pub provider_allowlist: Option<Vec<String>>,
}

#[derive(Debug, thiserror::Error)]
pub enum PolicyError {
    #[error("failed to parse policy: {0}")]
    Parse(#[from] serde_json::Error),
    #[error("invalid privacy_level_floor: {0}")]
    InvalidFloor(#[from] PrivacyLevelError),
}

impl EnterprisePolicy {
    /// Parses a policy file's contents. Fields outside this struct's own three
    /// (`policy_source`, `privacy_level_floor`, `provider_allowlist`) are
    /// silently ignored by `serde`'s default behavior — a policy author putting
    /// `"redaction_confidence_threshold": 0.0` or `"disable_deletion": true` in
    /// the JSON produces a policy that parses successfully and behaves exactly
    /// as if those keys weren't there, because nothing in this type has anywhere
    /// to put that data.
    pub fn parse(json: &str) -> Result<Self, PolicyError> {
        let policy: EnterprisePolicy = serde_json::from_str(json)?;
        if let Some(floor) = policy.privacy_level_floor {
            PrivacyLevel::from_u8(floor)?;
        }
        Ok(policy)
    }

    /// Raises `user_choice` to the policy floor if the floor is higher —
    /// never lowers it. A user who has chosen a *more* protective level than the
    /// floor requires keeps their own choice; the floor is a minimum, not a
    /// fixed assignment.
    pub fn effective_privacy_level(&self, user_choice: PrivacyLevel) -> PrivacyLevel {
        match self
            .privacy_level_floor
            .and_then(|f| PrivacyLevel::from_u8(f).ok())
        {
            Some(floor) if floor > user_choice => floor,
            _ => user_choice,
        }
    }

    /// `true` if no allowlist is configured (unrestricted) or `provider_id` is on
    /// it.
    pub fn is_provider_allowed(&self, provider_id: &str) -> bool {
        match &self.provider_allowlist {
            None => true,
            Some(allowed) => allowed.iter().any(|p| p == provider_id),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_policy_loaded_means_no_constraints() {
        let policy = EnterprisePolicy::default();
        assert_eq!(
            policy.effective_privacy_level(PrivacyLevel::Manual),
            PrivacyLevel::Manual
        );
        assert!(policy.is_provider_allowed("anything"));
    }

    #[test]
    fn floor_raises_a_lower_user_choice() {
        let policy = EnterprisePolicy {
            privacy_level_floor: Some(PrivacyLevel::WorkflowMetadata.as_u8()),
            ..Default::default()
        };
        assert_eq!(
            policy.effective_privacy_level(PrivacyLevel::Manual),
            PrivacyLevel::WorkflowMetadata
        );
    }

    #[test]
    fn floor_never_lowers_a_higher_user_choice() {
        let policy = EnterprisePolicy {
            privacy_level_floor: Some(PrivacyLevel::ApplicationMetadata.as_u8()),
            ..Default::default()
        };
        assert_eq!(
            policy.effective_privacy_level(PrivacyLevel::MaximumAssistance),
            PrivacyLevel::MaximumAssistance
        );
    }

    #[test]
    fn provider_allowlist_restricts_to_named_providers_only() {
        let policy = EnterprisePolicy {
            provider_allowlist: Some(vec![
                "ollama".to_string(),
                "internal-selfhosted".to_string(),
            ]),
            ..Default::default()
        };
        assert!(policy.is_provider_allowed("ollama"));
        assert!(!policy.is_provider_allowed("openai"));
    }

    #[test]
    fn rejects_an_out_of_range_privacy_level_floor() {
        let json = r#"{"privacy_level_floor": 9}"#;
        assert!(matches!(
            EnterprisePolicy::parse(json),
            Err(PolicyError::InvalidFloor(_))
        ));
    }

    #[test]
    fn parse_ignores_fields_outside_the_allowed_schema() {
        // A maximally adversarial policy file per
        // docs/roadmap/05-privacy-testing.md §6: every excluded-by-design field
        // a policy author might hope constrains something it structurally
        // cannot.
        let json = r#"{
            "policy_source": "corp-mdm",
            "privacy_level_floor": 1,
            "provider_allowlist": ["ollama"],
            "redaction_confidence_threshold": 0.0,
            "disable_deletion": true,
            "disable_trust_dashboard": true,
            "extend_retention_forever": true,
            "allow_level_4_cloud_dispatch": true
        }"#;
        let policy = EnterprisePolicy::parse(json).expect("should parse despite extra fields");
        assert_eq!(policy.policy_source, Some("corp-mdm".to_string()));
        assert_eq!(policy.privacy_level_floor, Some(1));
        assert_eq!(policy.provider_allowlist, Some(vec!["ollama".to_string()]));
        // Nothing else survived parsing — there is no field on this struct that
        // could hold any of the other five keys above, so re-serializing shows
        // only the three legitimate fields.
        let round_tripped = serde_json::to_value(&policy).unwrap();
        let keys: std::collections::BTreeSet<_> =
            round_tripped.as_object().unwrap().keys().cloned().collect();
        assert_eq!(
            keys,
            ["policy_source", "privacy_level_floor", "provider_allowlist"]
                .into_iter()
                .map(String::from)
                .collect()
        );
    }
}
