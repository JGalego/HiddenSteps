<!-- Thanks for contributing! Keep this focused — one logical change per PR. -->

## What & why

<!-- What does this change do, and why? Link any issue it closes. -->

## Privacy / security impact

<!-- Required if this touches observation, redaction, the cloud-dispatch gate,
     the plugin sandbox, or the encrypted store. State how the change preserves
     the guarantees in docs/design/04-trust-model.md / 05-privacy-model.md.
     Write "none" only if you're confident it genuinely has no such impact. -->

## Checklist

- [ ] `cargo fmt --check` and `cargo clippy --workspace --all-targets -- -D warnings` pass
- [ ] `cargo test --workspace` passes (and `npx tsc -b && npm test` in `apps/desktop/ui` if the UI changed)
- [ ] New behavior has a test that exercises the real edge case, not just a happy path
- [ ] Docs updated if this changes behavior or closes a disclosed gap (incl. test counts in `crates/README.md` / `apps/desktop/ui/README.md`)
- [ ] Commits are focused and their messages explain *why*
