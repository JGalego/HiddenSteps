//! The Plugin Framework (ADR-0009, `docs/design/08-plugin-architecture.md`): a
//! manifest schema with a closed capability enumeration, and a WASM host
//! (`wasmtime`) that links in only the host functions a plugin's granted
//! capabilities unlock.

mod host;
mod manifest;

pub use host::{CallLog, HostError, PluginHost, PluginInstance};
pub use manifest::{Capability, ManifestError, PluginManifest};
