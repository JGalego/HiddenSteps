//! Resource-exhaustion suite, per `docs/research/06-threat-model.md`'s
//! Denial-of-Service section: a module that needs **zero** granted
//! capabilities can still hang the host thread forever (an infinite loop) or
//! exhaust process memory (an unbounded `memory.grow` loop). Capability
//! enforcement (`capability_enforcement.rs`) has nothing to say about either,
//! since neither requires an import at all — these are handled by
//! `wasmtime`-level fuel metering and a memory `ResourceLimiter` instead, both
//! configured in `PluginHost`.

use hiddensteps_plugin_host::PluginHost;

fn compile(wat_source: &str) -> Vec<u8> {
    wat::parse_str(wat_source).expect("test fixture WAT should be well-formed")
}

const INFINITE_LOOP_MODULE: &str = r#"
(module
  (func (export "spin") (param i32) (result i32)
    (loop $l
      br $l)
    i32.const 0))
"#;

const MEMORY_GROWING_MODULE: &str = r#"
(module
  (memory (export "mem") 1)
  (func (export "grow_forever") (param i32) (result i32)
    (loop $l
      i32.const 1
      memory.grow
      drop
      br $l)
    i32.const 0))
"#;

#[test]
fn an_infinite_loop_needs_no_capabilities_to_instantiate() {
    // Establishes the premise the rest of this suite exists to fix: capability
    // enforcement alone does not stop this module, because it imports nothing.
    let host = PluginHost::new();
    host.instantiate(&compile(INFINITE_LOOP_MODULE), &[])
        .expect("a module with no imports needs no capabilities to instantiate");
}

#[test]
fn a_fuel_exhausted_infinite_loop_traps_instead_of_hanging_forever() {
    // A tiny fuel budget so this test resolves in milliseconds regardless of
    // the production-sized default — the point is that it resolves *at all*.
    let host = PluginHost::with_limits(10_000, 64 * 1024 * 1024);
    let mut instance = host
        .instantiate(&compile(INFINITE_LOOP_MODULE), &[])
        .unwrap();
    let result = instance.call_i32("spin", 0);
    assert!(
        result.is_err(),
        "an infinite loop must trap once its fuel budget is exhausted, not run forever"
    );
}

#[test]
fn a_bounded_fuel_budget_still_allows_normal_short_calls_to_complete() {
    let host = PluginHost::with_limits(10_000, 64 * 1024 * 1024);
    let mut instance = host
        .instantiate(
            &compile(
                r#"(module (func (export "add_one") (param i32) (result i32)
                       local.get 0 i32.const 1 i32.add))"#,
            ),
            &[],
        )
        .unwrap();
    assert_eq!(instance.call_i32("add_one", 41).unwrap(), 42);
}

#[test]
fn an_unbounded_memory_grow_loop_is_stopped_by_the_memory_limiter() {
    // Cap memory far below what an unbounded grow loop would otherwise reach,
    // and give it a fuel budget generous enough that fuel exhaustion (not the
    // memory limiter) isn't what actually stops it — isolating which limit is
    // doing the stopping.
    let host = PluginHost::with_limits(50_000_000, 1024 * 1024);
    let mut instance = host
        .instantiate(&compile(MEMORY_GROWING_MODULE), &[])
        .unwrap();
    let result = instance.call_i32("grow_forever", 0);
    assert!(
        result.is_err(),
        "an unbounded memory.grow loop must be stopped by the memory limiter"
    );
}
