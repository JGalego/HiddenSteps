# ADR-0001: Desktop shell — Tauri (Rust core + web-rendered UI)

Status: Accepted

## Context

PROMPT.md requires native-feeling support for Windows, macOS (Intel + Apple Silicon), and Linux, effortless installation, small footprint, and a security posture strong enough to justify handling sensitive behavioral data. The shell choice determines the language for the observation engine (which needs OS accessibility APIs, low-level window/process hooks, and background-process behavior) and the UI layer.

Options considered: Electron (Chromium + Node.js), Tauri (Rust core + OS-native WebView), fully native per-platform (Swift/AppKit, C++/Win32, GTK/Qt).

## Decision

Use **Tauri 2.x**: a Rust core process hosts the Observation Engine, Event Pipeline, Privacy/Redaction Engine, Security Layer, Plugin Framework, and data access; the UI renders in the OS's native WebView (WebKit/WebView2/WebKitGTK) using TypeScript + React, communicating with the Rust core over Tauri's typed IPC.

## Consequences

- Rust gives memory safety and a strong ecosystem for cryptography (`ring`, `rusqlite`+SQLCipher), OS credential vaults (`keyring`), and WASM plugin hosting (`wasmtime`) — all security-load-bearing.
- Binary size and idle resource footprint are far smaller than Electron (no bundled Chromium/Node runtime), which matters for a background-resident app users are already wary of.
- Tauri's IPC boundary between UI and core gives a natural, auditable trust boundary: the UI (web tech, larger attack surface if compromised, e.g. via a malicious plugin-supplied render) never gets direct filesystem/OS-API access — it only reaches the core through declared commands.
- Cost: Rust has a steeper contribution curve than TypeScript-only stacks (Electron), which may slow community/plugin contributions; mitigated by exposing most extensibility through the WASM plugin interface (ADR-0009) rather than requiring Rust to extend the product.
- Fully native per-platform was rejected: 3x the engineering surface for a team that needs to ship in months, not years, and cross-platform accessibility-API abstraction still has to be built regardless of shell choice.

## Alternatives considered

- **Electron**: mature ecosystem, but Node.js in-process is a materially larger attack surface for a product whose core asset is sensitive behavioral data, and bundled-Chromium footprint conflicts with the "not surveillance-feeling bloat" trust goal.
- **Fully native (3 codebases)**: best possible platform integration, rejected for cost/velocity at this stage; revisit per-platform native rewrites only if Tauri's accessibility-API bindings prove insufficient (tracked as a risk in [../../research/03-risk-analysis.md](../../research/03-risk-analysis.md)).
