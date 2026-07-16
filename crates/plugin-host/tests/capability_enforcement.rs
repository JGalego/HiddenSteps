//! The capability-escape test suite named in
//! `docs/design/06-security-architecture.md` §7 and
//! `docs/roadmap/04-security-testing.md` §1: real `.wasm` modules (compiled from
//! WAT at test time via the `wat` crate — no `wasm32-unknown-unknown` Rust
//! toolchain required, since these are hand-written host-function-calling
//! fixtures, not compiled Rust plugins), instantiated through the real
//! `wasmtime`-backed `PluginHost`, attempting to call host functions their
//! granted capability set does or doesn't cover.

use hiddensteps_plugin_host::{Capability, HostError, PluginHost};

const BENIGN_MODULE: &str = r#"
(module
  (func (export "noop") (param i32) (result i32)
    local.get 0
    i32.const 1
    i32.add))
"#;

const NETWORK_MODULE: &str = r#"
(module
  (import "host" "network_fetch" (func $network_fetch (param i32) (result i32)))
  (func (export "attempt_network_call") (param i32) (result i32)
    local.get 0
    call $network_fetch))
"#;

const OBSERVE_MODULE: &str = r#"
(module
  (import "host" "emit_signal" (func $emit_signal (param i32)))
  (func (export "attempt_emit") (param i32) (result i32)
    local.get 0
    call $emit_signal
    i32.const 1))
"#;

/// Imports BOTH `network_fetch` and `emit_signal` — used to prove that granting
/// only one of a module's required capabilities is not enough; instantiation
/// must fail entirely if *any* import is unresolved, not partially succeed.
const DUAL_CAPABILITY_MODULE: &str = r#"
(module
  (import "host" "network_fetch" (func $network_fetch (param i32) (result i32)))
  (import "host" "emit_signal" (func $emit_signal (param i32)))
  (func (export "attempt_both") (param i32) (result i32)
    local.get 0
    call $emit_signal
    local.get 0
    call $network_fetch))
"#;

fn compile(wat_source: &str) -> Vec<u8> {
    wat::parse_str(wat_source).expect("test fixture WAT should be well-formed")
}

#[test]
fn a_module_requesting_no_capabilities_always_instantiates_and_runs() {
    let host = PluginHost::new();
    let mut instance = host
        .instantiate(&compile(BENIGN_MODULE), &[])
        .expect("benign module needs no capabilities");
    assert_eq!(instance.call_i32("noop", 41).unwrap(), 42);
}

#[test]
fn a_module_importing_network_fetch_fails_to_instantiate_without_the_capability() {
    let host = PluginHost::new();
    let result = host.instantiate(&compile(NETWORK_MODULE), &[]);
    assert!(
        matches!(result, Err(HostError::Instantiate(_))),
        "expected instantiation to fail outright when the import is unresolved"
    );
}

#[test]
fn a_module_importing_network_fetch_succeeds_and_the_call_is_logged_once_granted() {
    let host = PluginHost::new();
    let mut instance = host
        .instantiate(&compile(NETWORK_MODULE), &[Capability::NetworkOutbound])
        .expect("granted capability should allow instantiation");
    let result = instance.call_i32("attempt_network_call", 7).unwrap();
    assert_eq!(result, 0); // stub network_fetch's fixed return value
    assert_eq!(instance.call_log(), vec!["network_fetch"]);
}

#[test]
fn an_observation_module_fails_without_any_observe_capability_granted() {
    let host = PluginHost::new();
    let result = host.instantiate(&compile(OBSERVE_MODULE), &[Capability::NetworkOutbound]);
    assert!(matches!(result, Err(HostError::Instantiate(_))));
}

#[test]
fn an_observation_module_succeeds_with_any_observe_capability_granted() {
    let host = PluginHost::new();
    let mut instance = host
        .instantiate(&compile(OBSERVE_MODULE), &[Capability::ObserveActiveWindow])
        .expect("any Observe* capability should unlock emit_signal");
    assert_eq!(instance.call_i32("attempt_emit", 0).unwrap(), 1);
    assert_eq!(instance.call_log(), vec!["emit_signal"]);
}

#[test]
fn granting_only_one_of_two_required_capabilities_still_fails_instantiation_entirely() {
    let host = PluginHost::new();
    // The module needs BOTH network_fetch and emit_signal; granting only one
    // capability must not yield a partially-functional instance — instantiation
    // itself has to fail, because `emit_signal` (unrequested here) is still an
    // unresolved import.
    let result = host.instantiate(
        &compile(DUAL_CAPABILITY_MODULE),
        &[Capability::NetworkOutbound],
    );
    assert!(
        matches!(result, Err(HostError::Instantiate(_))),
        "partial capability grant must not yield a partially-working instance"
    );
}

#[test]
fn granting_both_required_capabilities_allows_full_instantiation() {
    let host = PluginHost::new();
    let mut instance = host
        .instantiate(
            &compile(DUAL_CAPABILITY_MODULE),
            &[Capability::NetworkOutbound, Capability::ObserveActiveWindow],
        )
        .expect("both capabilities granted should allow instantiation");
    instance.call_i32("attempt_both", 3).unwrap();
    let mut calls = instance.call_log();
    calls.sort();
    assert_eq!(calls, vec!["emit_signal", "network_fetch"]);
}

#[test]
fn a_capability_grant_the_module_never_uses_does_not_change_behavior() {
    // Granting extra, unused capabilities must not cause failures or unexpected
    // side effects — only *requested-and-unresolved* imports should matter.
    let host = PluginHost::new();
    let mut instance = host
        .instantiate(
            &compile(BENIGN_MODULE),
            &[Capability::NetworkOutbound, Capability::ProviderLlm],
        )
        .expect("unused granted capabilities should be harmless");
    assert_eq!(instance.call_i32("noop", 1).unwrap(), 2);
    assert!(instance.call_log().is_empty());
}
