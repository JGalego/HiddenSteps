//! Proves `PluginHost::instantiate_from_manifest` actually ties manifest
//! validation to capability granting — the gap `PluginHost::instantiate`
//! alone leaves open, since it takes a bare `&[Capability]` slice with no
//! structural link to a `PluginManifest` at all.

use hiddensteps_plugin_host::{Capability, HostError, PluginHost, PluginManifest};

const BENIGN_MODULE: &str = r#"
(module
  (func (export "noop") (param i32) (result i32)
    local.get 0
    i32.const 1
    i32.add))
"#;

fn compile(wat_source: &str) -> Vec<u8> {
    wat::parse_str(wat_source).expect("test fixture WAT should be well-formed")
}

fn manifest(min_privacy_level: u8, capabilities: Vec<Capability>) -> PluginManifest {
    PluginManifest {
        id: "com.example.plugin".to_string(),
        name: "Example".to_string(),
        version: "1.0.0".to_string(),
        plugin_type: "observation_source".to_string(),
        min_privacy_level,
        capabilities,
    }
}

#[test]
fn a_manifest_that_fails_validation_is_rejected_before_any_capability_is_granted() {
    // ObserveScreenshot requires min_privacy_level 4 — this manifest declares
    // only 2, so it fails PluginManifest::validate(). instantiate_from_manifest
    // must refuse to instantiate at all, not fall back to granting anyway.
    let host = PluginHost::new();
    let invalid = manifest(2, vec![Capability::ObserveScreenshot]);
    let result = host.instantiate_from_manifest(&compile(BENIGN_MODULE), &invalid, &[]);
    assert!(matches!(result, Err(HostError::ManifestInvalid(_))));
}

#[test]
fn granting_a_capability_the_manifest_never_declared_is_rejected() {
    // The manifest only declares NetworkOutbound; a caller asking to also
    // grant ObserveActiveWindow — never requested — must be rejected, not
    // silently allowed through instantiate()'s bare capability slice.
    let host = PluginHost::new();
    let valid = manifest(2, vec![Capability::NetworkOutbound]);
    let result = host.instantiate_from_manifest(
        &compile(BENIGN_MODULE),
        &valid,
        &[Capability::NetworkOutbound, Capability::ObserveActiveWindow],
    );
    assert!(matches!(
        result,
        Err(HostError::GrantedCapabilityNotDeclared(
            Capability::ObserveActiveWindow
        ))
    ));
}

#[test]
fn a_valid_manifest_with_a_proper_capability_subset_instantiates_successfully() {
    let host = PluginHost::new();
    let valid = manifest(
        2,
        vec![Capability::NetworkOutbound, Capability::ProviderLlm],
    );
    // Granting only one of the two declared capabilities is a legitimate
    // "user approved less than requested" case, not an error.
    let mut instance = host
        .instantiate_from_manifest(
            &compile(BENIGN_MODULE),
            &valid,
            &[Capability::NetworkOutbound],
        )
        .expect("a valid manifest with a proper capability subset should instantiate");
    assert_eq!(instance.call_i32("noop", 1).unwrap(), 2);
}

#[test]
fn a_valid_manifest_at_the_required_level_allows_its_gated_capability() {
    let host = PluginHost::new();
    let valid = manifest(4, vec![Capability::ObserveScreenshot]);
    let result = host.instantiate_from_manifest(
        &compile(BENIGN_MODULE),
        &valid,
        &[Capability::ObserveScreenshot],
    );
    assert!(result.is_ok());
}
