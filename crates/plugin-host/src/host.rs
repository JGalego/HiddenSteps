use std::sync::{Arc, Mutex};

use wasmtime::{Engine, Instance, Linker, Module, Store};

use crate::manifest::Capability;

#[derive(Debug, thiserror::Error)]
pub enum HostError {
    #[error("failed to compile module: {0}")]
    Compile(String),
    #[error("failed to link/instantiate module: {0}")]
    Instantiate(String),
    #[error("failed to call exported function: {0}")]
    Call(String),
    #[error("export '{0}' not found or has the wrong signature")]
    ExportMismatch(String),
}

/// Records which gated host functions a plugin instance actually invoked, for
/// the runtime capability-usage audit-log entries described in
/// `docs/design/08-plugin-architecture.md` §3. Shared (`Arc<Mutex<_>>`, not
/// `Rc<RefCell<_>>`) between the host-function closures and the caller —
/// `wasmtime`'s `Linker::func_wrap` requires `Send + Sync` closures even for the
/// synchronous, single-threaded embedding used here.
#[derive(Debug, Default, Clone)]
pub struct CallLog {
    inner: Arc<Mutex<Vec<&'static str>>>,
}

impl CallLog {
    fn record(&self, call: &'static str) {
        self.inner
            .lock()
            .expect("call log mutex poisoned")
            .push(call);
    }

    pub fn calls(&self) -> Vec<&'static str> {
        self.inner.lock().expect("call log mutex poisoned").clone()
    }
}

/// The WASM component host (ADR-0009): compiles a plugin module and instantiates
/// it with **only** the host-function imports its granted capabilities unlock.
/// An ungranted capability's corresponding import is never linked — if the
/// module imports it anyway, `instantiate` fails outright, which is the
/// structural (not policy-based) enforcement the threat model
/// (`docs/research/06-threat-model.md`) calls the single highest-leverage
/// control in the system.
///
/// Host functions here (`network_fetch`, `read_file_metadata`, `emit_signal`,
/// `llm_complete`) are minimal, real, callable implementations sufficient to
/// prove the capability boundary works end-to-end — they are not yet wired to
/// the real Event Pipeline, `LlmProvider`, or filesystem, which is separate
/// integration work for a later milestone. What they prove now: an ungranted
/// capability's import is genuinely absent, not present-but-unused.
pub struct PluginHost {
    engine: Engine,
}

impl Default for PluginHost {
    fn default() -> Self {
        Self::new()
    }
}

impl PluginHost {
    pub fn new() -> Self {
        Self {
            engine: Engine::default(),
        }
    }

    pub fn instantiate(
        &self,
        wasm_bytes: &[u8],
        granted_capabilities: &[Capability],
    ) -> Result<PluginInstance, HostError> {
        let module =
            Module::new(&self.engine, wasm_bytes).map_err(|e| HostError::Compile(e.to_string()))?;

        let mut linker: Linker<()> = Linker::new(&self.engine);
        let call_log = CallLog::default();

        if granted_capabilities.contains(&Capability::NetworkOutbound) {
            let log = call_log.clone();
            linker
                .func_wrap("host", "network_fetch", move |_url_ptr: i32| -> i32 {
                    log.record("network_fetch");
                    // Minimal real behavior: report "not actually fetched" (0)
                    // rather than fabricating response data — there is no real
                    // network integration wired in at this milestone.
                    0
                })
                .map_err(|e| HostError::Instantiate(e.to_string()))?;
        }

        if granted_capabilities.contains(&Capability::FilesystemReadMetadata) {
            let log = call_log.clone();
            linker
                .func_wrap("host", "read_file_metadata", move |_path_ptr: i32| -> i32 {
                    log.record("read_file_metadata");
                    0
                })
                .map_err(|e| HostError::Instantiate(e.to_string()))?;
        }

        let has_observe_capability = granted_capabilities.iter().any(|c| {
            matches!(
                c,
                Capability::ObserveActiveWindow
                    | Capability::ObserveClipboardMetadata
                    | Capability::ObserveFileOperations
                    | Capability::ObserveScreenshot
            )
        });
        if has_observe_capability {
            let log = call_log.clone();
            linker
                .func_wrap("host", "emit_signal", move |_signal_ptr: i32| {
                    log.record("emit_signal");
                })
                .map_err(|e| HostError::Instantiate(e.to_string()))?;
        }

        if granted_capabilities.contains(&Capability::ProviderLlm) {
            let log = call_log.clone();
            linker
                .func_wrap("host", "llm_complete", move |_prompt_ptr: i32| -> i32 {
                    log.record("llm_complete");
                    0
                })
                .map_err(|e| HostError::Instantiate(e.to_string()))?;
        }

        let mut store = Store::new(&self.engine, ());
        let instance = linker
            .instantiate(&mut store, &module)
            .map_err(|e| HostError::Instantiate(e.to_string()))?;

        Ok(PluginInstance {
            store,
            instance,
            call_log,
        })
    }
}

pub struct PluginInstance {
    store: Store<()>,
    instance: Instance,
    call_log: CallLog,
}

impl PluginInstance {
    pub fn call_log(&self) -> Vec<&'static str> {
        self.call_log.calls()
    }

    /// Calls an exported `(i32) -> i32` function — the shape every test fixture
    /// in this crate uses. A real plugin ABI would be richer (memory-passed
    /// structs via the Component Model, per the WIT sketch in
    /// `docs/design/08-plugin-architecture.md` §4); this core-module-level
    /// calling convention is what `wasmtime`'s plain `Linker`/`Instance` API
    /// gives directly, and is enough to prove capability enforcement without
    /// requiring a full `wit-bindgen` toolchain in this milestone.
    pub fn call_i32(&mut self, export_name: &str, arg: i32) -> Result<i32, HostError> {
        let func = self
            .instance
            .get_typed_func::<i32, i32>(&mut self.store, export_name)
            .map_err(|_| HostError::ExportMismatch(export_name.to_string()))?;
        func.call(&mut self.store, arg)
            .map_err(|e| HostError::Call(e.to_string()))
    }
}
