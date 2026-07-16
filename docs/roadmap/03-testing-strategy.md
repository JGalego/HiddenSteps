# Testing Strategy

General test approach; [04-security-testing.md](04-security-testing.md), [05-privacy-testing.md](05-privacy-testing.md), and [06-performance-testing.md](06-performance-testing.md) cover the three specialized suites PROMPT.md calls out by name.

## 1. Test pyramid

| Level | Scope | Tooling | Owner-of-correctness |
|---|---|---|---|
| Unit | Individual use-cases and domain logic behind their ports ([../design/02-system-architecture.md](../design/02-system-architecture.md)) — e.g., a redaction classifier given a crafted string, the numeric-drift validator given a mismatched LLM output | `cargo test`, mocked ports | Fast, run on every commit |
| Integration | Real adapters wired together within a crate boundary — e.g., the Event Pipeline against a real (test) SQLCipher file, `sqlite-vec` similarity search against seeded embeddings | `cargo test` with `tempdir`-scoped real SQLCipher instances | Run on every PR |
| Cross-crate / application | A full use-case flow through multiple modules — e.g., capture → pipeline → pattern detection → recommendation generation, with a stub `LlmProvider` | In-process test harness assembling the real Application layer with test-double Infrastructure | Run on every PR |
| End-to-end (UI) | Full journeys from [../ux/01-user-journeys.md](../ux/01-user-journeys.md) driven through the real Tauri app | `tauri-driver` (WebDriver) | Run pre-merge to `main` and pre-release |
| Manual/exploratory | Accessibility (screen reader/keyboard), cross-platform look-and-feel, real hardware variety for local AI | Human testers per the [../ux/06-accessibility.md](../ux/06-accessibility.md) §5 bar | Pre-release gate |

## 2. What must be tested with real components, not mocks

Per this project's own trust claims, certain guarantees are meaningless if only verified against mocks:

- **Redaction drop-on-uncertainty** ([../design/05-privacy-model.md](../design/05-privacy-model.md) §4) must be tested against the real classifier with real adversarial inputs, not a mock that always returns "confident" — a mocked redactor tells you the pipeline *wiring* is correct, never whether redaction actually works. See [05-privacy-testing.md](05-privacy-testing.md).
- **Plugin capability enforcement** ([../design/adr/0009-wasm-plugin-sandbox.md](../design/adr/0009-wasm-plugin-sandbox.md)) must be tested against the real `wasmtime` sandbox attempting real capability-escaping calls — a mocked host trivially "passes" this test regardless of whether the real sandbox is sound. See [04-security-testing.md](04-security-testing.md).
- **Encryption at rest** must be tested by attempting to open the real `hiddensteps.db` file with a generic SQLite tool (expecting failure) and by confirming `delete_all_data` leaves the file's remnants genuinely unreadable once the vault key is gone.
- **The numeric-drift validator** ([../design/adr/0010-hybrid-deterministic-plus-llm-recommendation-engine.md](../design/adr/0010-hybrid-deterministic-plus-llm-recommendation-engine.md)) must be tested against real (or realistically fuzzed) LLM output, including cases where a small local model paraphrases a number in prose ("about eleven hours") rather than altering the structured field — the validator must catch drift in both the structured and prose-explanation paths, not just the structured one.

## 3. Test data and fixtures

- Synthetic workflow traces (scripted sequences of app-focus/clipboard/file events) generated to deterministically produce known patterns, used across pipeline/pattern-detection/recommendation tests — avoids needing real personal data (which the project's own principles argue against collecting even for testing) while still exercising the full pipeline realistically.
- A small corpus of known secret/PII/PHI-shaped strings (synthetic, not real credentials) for redaction testing, expanded continuously as new evasion techniques are found in adversarial testing.
- A "hostile plugin" fixture: a WASM component deliberately written to attempt capability escapes, kept in the test suite permanently as a regression guard, not deleted after the sandbox is believed sound.

## 4. Regression discipline

- Every confirmed security or privacy finding (from [04-security-testing.md](04-security-testing.md)/[05-privacy-testing.md](05-privacy-testing.md)) gets a permanent regression test before the fix is considered complete — the finding class must be impossible to silently reintroduce.
- CI blocks merge on: unit/integration/cross-crate test failures, dependency-direction lint violations ([../design/adr/0002-clean-architecture-modular-core.md](../design/adr/0002-clean-architecture-modular-core.md)), and accessibility critical/serious violations ([../ux/06-accessibility.md](../ux/06-accessibility.md)).
- E2E and manual suites run pre-release, not per-PR (cost/latency tradeoff), but any E2E failure blocks that release, not just triggers a follow-up ticket.
