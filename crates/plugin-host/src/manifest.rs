use serde::{Deserialize, Serialize};

/// The closed capability enumeration from `docs/design/08-plugin-architecture.md`
/// §2. A manifest requesting anything outside this enum fails to parse — there is
/// no `Capability::Other(String)` catch-all variant, which is what makes "closed"
/// a real property of this type rather than a description in a doc comment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    ObserveActiveWindow,
    ObserveClipboardMetadata,
    ObserveFileOperations,
    /// Deep-mode only; a manifest requesting this is also, separately, required
    /// to declare `min_privacy_level: 4` — enforced in `PluginManifest::validate`.
    ObserveScreenshot,
    NetworkOutbound,
    FilesystemReadMetadata,
    ProviderLlm,
    ProviderEmbedding,
    PolicyRead,
}

impl Capability {
    /// Capabilities that may only be granted to a plugin declaring
    /// `min_privacy_level >= 4`, per `docs/design/05-privacy-model.md` §1.
    pub fn requires_level_four(self) -> bool {
        matches!(self, Capability::ObserveScreenshot)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub plugin_type: String,
    pub min_privacy_level: u8,
    pub capabilities: Vec<Capability>,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ManifestError {
    #[error("capability {0:?} requires min_privacy_level 4, but the manifest declares {1}")]
    CapabilityBelowRequiredLevel(Capability, u8),
    #[error("min_privacy_level {0} is out of range (expected 0-4)")]
    InvalidPrivacyLevel(u8),
}

impl PluginManifest {
    pub fn parse(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Structural validation independent of signature verification (which is a
    /// separate, later step against the plugin distribution mechanism, not this
    /// type's concern) — this only checks that the manifest's own declared
    /// fields are internally consistent.
    pub fn validate(&self) -> Result<(), ManifestError> {
        if self.min_privacy_level > 4 {
            return Err(ManifestError::InvalidPrivacyLevel(self.min_privacy_level));
        }
        for capability in &self.capabilities {
            if capability.requires_level_four() && self.min_privacy_level < 4 {
                return Err(ManifestError::CapabilityBelowRequiredLevel(
                    *capability,
                    self.min_privacy_level,
                ));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_an_unknown_capability_at_parse_time() {
        let json = r#"{
            "id": "com.example.plugin", "name": "Example", "version": "1.0.0",
            "plugin_type": "observation_source", "min_privacy_level": 2,
            "capabilities": ["read_arbitrary_memory"]
        }"#;
        assert!(PluginManifest::parse(json).is_err());
    }

    #[test]
    fn accepts_a_well_formed_manifest() {
        let json = r#"{
            "id": "com.example.jira-observer", "name": "Jira Observer", "version": "1.2.0",
            "plugin_type": "observation_source", "min_privacy_level": 2,
            "capabilities": ["observe_active_window", "network_outbound"]
        }"#;
        let manifest = PluginManifest::parse(json).unwrap();
        assert!(manifest.validate().is_ok());
        assert_eq!(manifest.capabilities.len(), 2);
    }

    #[test]
    fn screenshot_capability_below_level_four_fails_validation() {
        let manifest = PluginManifest {
            id: "com.example.screen-reader".to_string(),
            name: "Screen Reader".to_string(),
            version: "1.0.0".to_string(),
            plugin_type: "observation_source".to_string(),
            min_privacy_level: 2,
            capabilities: vec![Capability::ObserveScreenshot],
        };
        assert_eq!(
            manifest.validate(),
            Err(ManifestError::CapabilityBelowRequiredLevel(
                Capability::ObserveScreenshot,
                2
            ))
        );
    }

    #[test]
    fn screenshot_capability_at_level_four_passes_validation() {
        let manifest = PluginManifest {
            id: "com.example.screen-reader".to_string(),
            name: "Screen Reader".to_string(),
            version: "1.0.0".to_string(),
            plugin_type: "observation_source".to_string(),
            min_privacy_level: 4,
            capabilities: vec![Capability::ObserveScreenshot],
        };
        assert!(manifest.validate().is_ok());
    }
}
