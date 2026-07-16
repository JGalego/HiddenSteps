# HiddenSteps Desktop Shell

The Tauri application (ADR-0001) that wires the Rust core crates under `../../crates/` to a TypeScript/React UI.

## Why this isn't in the root Cargo workspace

`crates/*` (the root `Cargo.toml`'s `members`) builds and tests cleanly in this repository's Linux dev environment with zero system dependencies beyond what `cargo` fetches itself — that was a deliberate constraint (see `../../crates/README.md`) so the core logic could be genuinely compiled and tested here, not just written and hoped-correct.

`src-tauri/` cannot: Tauri's Linux backend links against `webkit2gtk-4.1` (and friends) via `pkg-config`, which is not installed in this environment and cannot be installed without root/administrative access this session doesn't have (confirmed: no passwordless `sudo`, no working `nix`/package-manager path found — see the session's environment survey). Adding this crate to the root workspace would make `cargo build --workspace` fail here, breaking the one thing in this repository that *is* fully verified.

**This is real, complete source** — `src-tauri/src/main.rs` wires every crate under `../../crates/` into the IPC command surface specified in `../../docs/design/09-api-specification.md` — but it has **not been compiled**. On a machine with the Tauri Linux prerequisites installed (or on macOS/Windows, where the platform-native WebView needs no separate install), build it directly:

```sh
cd apps/desktop/src-tauri
cargo build
```

and fix whatever the compiler finds — API surface drift in `tauri` itself between versions is the most likely source of errors, not the application logic, which reuses the already-tested core crates directly.

## Layout

```
src-tauri/       Rust: Tauri commands/events wrapping the core crates, per docs/design/09-api-specification.md
ui/              TypeScript/React: onboarding, privacy dashboard, recommendations — see ui/README.md
```

## UI is independently buildable and tested here

Unlike `src-tauri/`, `ui/` has no native dependency — it's a normal Vite/React project, and its component tests run against a mocked Tauri IPC bridge (see `ui/README.md`), so it *is* built and tested in this environment, same rigor as the Rust core.
