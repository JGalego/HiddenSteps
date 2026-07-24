# Contributing to HiddenSteps

Thanks for wanting to help. HiddenSteps is a local-first workflow-intelligence
tool whose whole point is being trustworthy with sensitive data, so
contributions are held to that bar — but the project is early and genuinely
welcomes help, especially the kinds listed under "Especially wanted" below.

## Ground rules

- **The docs are the spec.** Behavior is defined in [`docs/`](docs/) — the PRD,
  ADRs, privacy/trust/threat models, and UX specs. If a change contradicts a
  doc, either the change is wrong or the doc needs updating in the same PR;
  don't leave them disagreeing. `crates/README.md` and
  `apps/desktop/README.md` are the ground truth for *current implementation
  state* (as opposed to the design docs' target state).
- **Privacy guarantees are not negotiable in a drive-by PR.** Anything touching
  observation, redaction, the cloud-dispatch gate, the plugin sandbox, or the
  encrypted store should explain in the PR how it preserves the guarantees in
  `docs/design/04-trust-model.md` / `05-privacy-model.md`. When in doubt, open
  an issue to discuss first.
- **Report security issues privately** — see [SECURITY.md](SECURITY.md), not a
  public issue.

## Development setup

Two independent pieces (see the root [README](README.md#building-it-yourself)
for the longer version):

```sh
# 1. The Rust core — no system dependencies beyond cargo.
cargo test --workspace

# 2. The UI — a normal Vite/React project.
cd apps/desktop/ui && npm install && npm test
```

The native Tauri shell (`apps/desktop/src-tauri`) needs
[Tauri's Linux prerequisites](https://tauri.app/start/prerequisites/)
(`libwebkit2gtk-4.1-dev` and friends) to compile; it is verified in CI on all
three OSes.

## Before you open a PR

Run the same checks CI runs — a green local run is the fastest path to a
merge:

```sh
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
# and in apps/desktop/ui:
npx tsc -b && npm test
```

- **Every behavior change needs a test.** This codebase's tests exercise real
  edge cases and adversarial inputs, not happy paths — match that.
- **Keep commits focused.** One logical change per commit, with a message that
  says *why*, not just *what*.
- **Update the docs and the test counts** in `crates/README.md` /
  `apps/desktop/ui/README.md` if your change adds tests or closes a
  disclosed gap.

## Especially wanted

Per the README: actually running a release build day-to-day and reporting what
breaks; a real design pass on the app icon; signing/notarization setup;
browser-domain observation (needs a separate browser-extension component); the
plugin-management and exclusion-rule UI; and filling in any of the gaps
disclosed throughout `crates/README.md` and `apps/desktop/*/README.md`.

## Code of conduct

By participating you agree to uphold our
[Code of Conduct](CODE_OF_CONDUCT.md).

## License

By contributing, you agree that your contributions will be licensed under the
project's [MIT License](LICENSE).
