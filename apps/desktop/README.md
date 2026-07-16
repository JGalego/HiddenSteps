# HiddenSteps Desktop Shell

The Tauri application (ADR-0001) that wires the Rust core crates under `../../crates/` to a TypeScript/React UI.

## Why this isn't in the root Cargo workspace

`crates/*` (the root `Cargo.toml`'s `members`) builds and tests cleanly in this repository's Linux dev environment with zero system dependencies beyond what `cargo` fetches itself — that was a deliberate constraint (see `../../crates/README.md`) so the core logic could be genuinely compiled and tested here, not just written and hoped-correct.

`src-tauri/` cannot in *this* dev sandbox: Tauri's Linux backend links against `webkit2gtk-4.1` (and friends) via `pkg-config`, which isn't installed here and can't be installed without root/administrative access this session doesn't have (confirmed: no passwordless `sudo`, no working `nix`/package-manager path found). Adding this crate to the root workspace would make `cargo build --workspace` fail here, breaking the one thing in this repository that *is* fully verified in this environment. It also needs its own (empty) `[workspace]` table in `Cargo.toml` for exactly this reason — see that file's comment.

**This now compiles for real, on all three platforms** — verified in [CI](../../.github/workflows/ci.yml), which has real `webkit2gtk` (Linux), a real macOS toolchain, and a real Windows SDK that this sandbox doesn't. That first real CI run caught and fixed three genuine bugs no amount of local review would have found: a missing `#[derive(Serialize)]` on a type crossing the Tauri IPC boundary ([`DetectedRuntime`](../../crates/llm-provider/src/detection.rs) — the root workspace's own tests couldn't catch this, since nothing there depends on `tauri`), a missing `tray-icon` Cargo feature (the config declared a tray icon; the feature flag enabling the API to create one wasn't set), and a missing generated icon set (`icons/icon.ico` etc. — `tauri icon` output, committed now). See the commit history for the exact fixes.

Releases are built and published automatically by [`release.yml`](../../.github/workflows/release.yml) on a version tag — see the root README's "Get it" section. To build it yourself:

```sh
cd apps/desktop/src-tauri
cargo build
```

## Layout

```
src-tauri/       Rust: Tauri commands/events wrapping the core crates, per docs/design/09-api-specification.md
ui/              TypeScript/React: onboarding, privacy dashboard, recommendations — see ui/README.md
```

## UI is independently buildable and tested here

Unlike `src-tauri/`, `ui/` has no native dependency — it's a normal Vite/React project, and its component tests run against a mocked Tauri IPC bridge (see `ui/README.md`), so it *is* built and tested in this environment, same rigor as the Rust core.
