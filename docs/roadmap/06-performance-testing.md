# Performance Testing Plan

Targets the "must not be perceptible" non-functional requirement in [../design/01-prd.md](../design/01-prd.md) §6 and the resource-consumption risk in [../research/03-risk-analysis.md](../research/03-risk-analysis.md) — a background-resident app that visibly costs battery/CPU is a fast path to uninstall regardless of how good its recommendations are.

## 1. Budgets (targets to test against, on a mid-range 2020+ laptop baseline)

| Resource | Idle (observation active, no active inference) | During batched pipeline/inference work |
|---|---|---|
| CPU | <1-2% sustained | Bursty, idle-time-scheduled (ADR-0006); never sustained high CPU while the user is actively typing/clicking in a foreground app |
| RAM | <150 MB core process (excluding a loaded local model's own footprint, which is disclosed separately in Diagnostics) | Bounded growth — no unbounded buffer growth from the pipeline's ring buffer or the WASM plugin host |
| Disk I/O | Negligible at idle (summarize/embed/store batched, not per-event) | Bounded write bursts at idle-time scheduling windows |
| Battery impact (laptop) | Not measurably different from baseline-idle in OS battery reporting | Local inference bursts are the expected exception, time-boxed and user-configurable in intensity |

These are starting targets to validate/refine once real profiling data exists in M1-M2 ([01-implementation-roadmap.md](01-implementation-roadmap.md)) — treat as hypotheses to test, not committed SLAs, until measured.

## 2. Test scenarios

| Scenario | What it validates |
|---|---|
| 8-hour idle-observation soak test (Minimal/Standard levels, no AI activity) | Baseline resource footprint matches budgets; no memory leak over a full workday |
| Heavy-multitasking stress test (rapid app/window switching, high-frequency clipboard use) | Event Pipeline keeps up without dropping events it should have redacted/stored, and without blocking foreground input |
| Local inference latency test (Ollama, small/medium model, on both GPU and CPU-only hardware) | Recommendation-synthesis latency is acceptable for a batched/idle-time UX (not a blocking-UI concern, but still bounded enough that recommendations don't lag days behind pattern detection) |
| `sqlite-vec` similarity-search scaling test (synthetic pattern volumes from hundreds to tens of thousands of embeddings) | Confirms the exact-search approach (ADR-0007) stays fast enough at realistic single-user volumes, and identifies the point where the ANN-mode upgrade path would actually be needed |
| Plugin-host overhead test (multiple concurrent WASM observation-source plugins) | WASM sandboxing overhead (ADR-0009) stays acceptable under realistic plugin counts, not just a single-plugin benchmark |
| Deep-mode (OCR/screenshot) capture-rate test | Confirms Deep-mode's heavier capture doesn't degrade foreground responsiveness, since this is the mode most likely to be resource-intensive |
| Cold-start test | Time from launch to "observation active + dashboard responsive" — a slow cold start undermines the "under five minutes" onboarding goal on subsequent launches too |

## 3. Hardware coverage

- A representative low/mid/high hardware matrix (per [../design/01-prd.md](../design/01-prd.md)'s equity note in the ethical analysis): a several-years-old CPU-only laptop, a mid-range machine with an integrated GPU, and a machine with a dedicated GPU — local-model performance and the hardware-suitability recommendation shown at onboarding ([../ux/02-onboarding-flow.md](../ux/02-onboarding-flow.md) Screen 5) should be validated against real measurements on each tier, not estimated.
- Explicitly include an under-resourced-hardware scenario in results reporting (not just the best-case machine) — the equity concern in [../research/04-ethical-analysis.md](../research/04-ethical-analysis.md) means "how bad is it on weak hardware" is itself a metric worth publishing honestly in Diagnostics, not hiding.

## 4. Cadence

- Idle-soak and multitasking-stress scenarios run automatically on a schedule (e.g., nightly) against the current build, with resource-usage trend tracked over time — a regression here should be caught within a day, not discovered at release.
- Local-inference-latency and `sqlite-vec`-scaling tests run per release candidate, since they depend more on model/library version changes than on day-to-day code changes.
- Hardware-matrix runs happen at least once per milestone from M2 onward (once AI is in the loop) and always before GA.
