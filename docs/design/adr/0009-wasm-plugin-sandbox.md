# ADR-0009: WASM component-model sandbox for all third-party plugins

Status: Accepted

## Context

PROMPT.md requires everything to be pluggable (observation, LLM/embedding providers, automation providers, enterprise policies, recommendation engines, pattern detectors, integrations). The threat model identifies "plugin escaping its declared capability scope" as the single highest-leverage risk in the whole system — manifest-declared trust without runtime enforcement is not a real security boundary.

## Decision

All third-party (non-in-tree) plugins run inside a **WASM sandbox** (`wasmtime`, using the WASM Component Model for typed interfaces defined in WIT). Each plugin ships a manifest declaring its requested capabilities from a closed, enumerable set (e.g., `observe:active_window`, `observe:clipboard_metadata`, `observe:screenshot` [Deep-mode only, additional consent-gated], `network:outbound(host_allowlist)`, `provider:llm`, `provider:embedding`, `filesystem:read(path_scope)`). The WASM host grants **only** the capabilities the manifest declares and the user has approved — there is no ambient capability a plugin can reach outside what's explicitly wired into its sandbox instance (e.g., a plugin without `network:outbound` literally has no socket import available to call, not just a policy telling it not to use one).

## Consequences

- Capability enforcement is structural (the sandbox doesn't expose the syscall/API surface at all) rather than advisory, closing the exact gap the threat model flagged as highest-leverage.
- In-tree/first-party plugins (the default observation sources for Minimal/Standard modes, Ollama/OpenAI/Anthropic providers) are reviewed and trusted at build time and may run natively for performance reasons, but are held to the same manifest-declaration discipline so the privacy dashboard's "what's active" view is uniform regardless of a plugin's trust tier.
- WASM's performance overhead is acceptable for this workload (plugins run periodically/on-event, not in a hot per-frame loop); if a specific observation plugin needs native-speed hooking (e.g., a global low-level keyboard hook), that capability is exposed as a narrow, host-provided WIT interface (the plugin calls out to a host function that does the native work under host-enforced constraints), not by giving the plugin native code execution.
- Plugin distribution/signing is still required on top of sandboxing (a sandboxed-but-malicious plugin can still request unwanted capabilities the user has to be prompted about) — the sandbox limits blast radius if a granted capability is later abused via a bug, it doesn't replace informed consent to grant the capability in the first place.

## Alternatives considered

- **OS-process-per-plugin isolation (e.g., separate subprocess with OS-level sandboxing/seccomp)**: rejected as the default — heavier resource cost per plugin (relevant given many small observation-source plugins may run concurrently), harder to get a uniform capability model across Windows/macOS/Linux where OS sandboxing primitives differ significantly; may be revisited for specific high-risk plugin categories in a later phase.
- **Trust-by-manifest with no runtime enforcement**: rejected outright — this is precisely the gap identified in the threat model.
