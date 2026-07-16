use std::collections::HashSet;

use hiddensteps_domain::PrivacyLevel;

/// Mirrors `docs/design/05-privacy-model.md` §3's three cloud-eligibility tiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloudEligibility {
    /// Level 1-2 pattern summaries with no verbatim strings — cloud-eligible as
    /// soon as the user has opted into cloud providers at all.
    AlwaysEligible,
    /// Level 2-3 content that includes a verbatim string (a domain, a window
    /// title, a file path) — requires separate, explicit per-content-class
    /// consent beyond general cloud-provider opt-in.
    RequiresConsent,
    /// Any Level 4 (Deep-mode) content — never cloud-eligible, regardless of any
    /// consent the user could give. This is a hard architectural rule (ADR-0004),
    /// not a configurable setting: there is no constructor parameter or method
    /// on anything in this crate that can make this return `AlwaysEligible` or
    /// `RequiresConsent` instead.
    NeverEligible,
}

pub fn cloud_eligibility(
    privacy_level: PrivacyLevel,
    contains_verbatim_strings: bool,
) -> CloudEligibility {
    if privacy_level == PrivacyLevel::MaximumAssistance {
        return CloudEligibility::NeverEligible;
    }
    if contains_verbatim_strings {
        CloudEligibility::RequiresConsent
    } else {
        CloudEligibility::AlwaysEligible
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DispatchDecision {
    Allow,
    /// Carries the content-class name the user needs to separately consent to
    /// (e.g. `"window_title"`, `"file_path"`) — enough for a caller to prompt for
    /// exactly that consent, per `docs/design/03-data-flow-diagrams.md` §5.
    RequiresConsent(String),
    Blocked,
}

/// The gate every `LlmProvider` call site must pass through before dispatching
/// anything to a non-local provider — see `PrivacyGatedProvider` in this crate for
/// the wrapper that makes bypassing it structurally awkward, not just discouraged.
#[derive(Debug, Clone, Default)]
pub struct DispatchGate {
    general_cloud_consent: bool,
    per_class_consent: HashSet<String>,
}

impl DispatchGate {
    pub fn new() -> Self {
        Self::default()
    }

    /// The one-time "I'm OK using a cloud provider at all" consent from
    /// onboarding (FR-15/FR-16) — necessary but not sufficient on its own for
    /// `RequiresConsent`-tier content.
    pub fn grant_general_cloud_consent(&mut self) {
        self.general_cloud_consent = true;
    }

    pub fn revoke_general_cloud_consent(&mut self) {
        self.general_cloud_consent = false;
    }

    pub fn grant_class_consent(&mut self, content_class: impl Into<String>) {
        self.per_class_consent.insert(content_class.into());
    }

    pub fn revoke_class_consent(&mut self, content_class: &str) {
        self.per_class_consent.remove(content_class);
    }

    pub fn evaluate(
        &self,
        provider_is_local: bool,
        privacy_level: PrivacyLevel,
        content_class: &str,
        contains_verbatim_strings: bool,
    ) -> DispatchDecision {
        // A local provider never leaves the device — nothing to gate, regardless
        // of privacy level. This mirrors ADR-0004's flowchart exactly.
        if provider_is_local {
            return DispatchDecision::Allow;
        }
        if !self.general_cloud_consent {
            return DispatchDecision::Blocked;
        }
        match cloud_eligibility(privacy_level, contains_verbatim_strings) {
            CloudEligibility::NeverEligible => DispatchDecision::Blocked,
            CloudEligibility::AlwaysEligible => DispatchDecision::Allow,
            CloudEligibility::RequiresConsent => {
                if self.per_class_consent.contains(content_class) {
                    DispatchDecision::Allow
                } else {
                    DispatchDecision::RequiresConsent(content_class.to_string())
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_provider_is_always_allowed_regardless_of_privacy_level_or_consent() {
        let gate = DispatchGate::new(); // no consent granted at all
        for level in PrivacyLevel::ALL {
            assert_eq!(
                gate.evaluate(true, level, "anything", true),
                DispatchDecision::Allow
            );
        }
    }

    #[test]
    fn cloud_provider_blocked_without_general_consent() {
        let gate = DispatchGate::new();
        assert_eq!(
            gate.evaluate(
                false,
                PrivacyLevel::ApplicationMetadata,
                "pattern_summary",
                false
            ),
            DispatchDecision::Blocked
        );
    }

    #[test]
    fn cloud_provider_allowed_for_shape_only_content_once_general_consent_granted() {
        let mut gate = DispatchGate::new();
        gate.grant_general_cloud_consent();
        assert_eq!(
            gate.evaluate(
                false,
                PrivacyLevel::WorkflowMetadata,
                "pattern_summary",
                false
            ),
            DispatchDecision::Allow
        );
    }

    #[test]
    fn verbatim_content_requires_separate_per_class_consent() {
        let mut gate = DispatchGate::new();
        gate.grant_general_cloud_consent();
        assert_eq!(
            gate.evaluate(false, PrivacyLevel::ContextAware, "window_title", true),
            DispatchDecision::RequiresConsent("window_title".to_string())
        );

        gate.grant_class_consent("window_title");
        assert_eq!(
            gate.evaluate(false, PrivacyLevel::ContextAware, "window_title", true),
            DispatchDecision::Allow
        );
    }

    #[test]
    fn level_four_content_is_never_eligible_even_with_every_consent_granted() {
        let mut gate = DispatchGate::new();
        gate.grant_general_cloud_consent();
        gate.grant_class_consent("ocr_text");
        assert_eq!(
            gate.evaluate(false, PrivacyLevel::MaximumAssistance, "ocr_text", false),
            DispatchDecision::Blocked
        );
    }

    #[test]
    fn revoking_consent_takes_effect_immediately() {
        let mut gate = DispatchGate::new();
        gate.grant_general_cloud_consent();
        gate.grant_class_consent("file_path");
        assert_eq!(
            gate.evaluate(false, PrivacyLevel::WorkflowMetadata, "file_path", true),
            DispatchDecision::Allow
        );
        gate.revoke_class_consent("file_path");
        assert_eq!(
            gate.evaluate(false, PrivacyLevel::WorkflowMetadata, "file_path", true),
            DispatchDecision::RequiresConsent("file_path".to_string())
        );

        gate.revoke_general_cloud_consent();
        assert_eq!(
            gate.evaluate(
                false,
                PrivacyLevel::WorkflowMetadata,
                "pattern_summary",
                false
            ),
            DispatchDecision::Blocked
        );
    }
}
