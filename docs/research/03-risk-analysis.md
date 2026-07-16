# Risk Analysis

Each risk is rated Likelihood/Impact (L/M/H) and paired with the mitigation that Phase 2 architecture should encode as a constraint, not an afterthought.

## Product and trust risks

| Risk | L/I | Detail | Mitigation direction |
|---|---|---|---|
| Perceived as spyware | H/H | The core mechanic — background observation of a person's work — is structurally identical to what employee-monitoring software does. First impression determines adoption. | Observation off by default until explicit informed consent (Level 0 start); observation status always visible; no observation-related feature ships without a corresponding entry in the privacy dashboard. See [04](04-ethical-analysis.md), [05](05-privacy-analysis.md). |
| Employer misuse | H/H | Even if built for individuals, nothing stops an employer from mandating installation and demanding export of a report. | Design so employer-mandated deployment without individual consent is explicitly out of scope / against license terms; no built-in "manager view" or aggregate export feature; local-only storage makes silent employer access harder by construction. |
| Trust erosion from a single bad recommendation | M/M | An automation suggestion that breaks something (e.g., a bad script suggestion) will disproportionately damage trust versus the credit gained from good suggestions. | Every recommendation ships with confidence, assumptions, and "what could go wrong" — never silently execute; require explicit user action to implement anything. |
| Low retention after novelty wears off | H/M | Productivity tools notoriously see high install, low 30-day retention. | Front-load a genuine "quick win" within 24 hours (per PROMPT.md's Zero-to-Value requirement); make the ongoing value legible (a running "time reclaimed" ledger). |

## Technical risks

| Risk | L/I | Detail | Mitigation direction |
|---|---|---|---|
| Local LLM quality insufficient for useful recommendations | H/M | Small local models may under-perform at pattern synthesis and explanation quality versus frontier cloud models. | Design the recommendation engine to degrade gracefully — rule-based/heuristic pattern detection should work even with a weak or absent LLM; treat the LLM as an explanation/synthesis layer on top of deterministic signal detection, not the sole source of insight. |
| Cross-platform observation parity | H/M | macOS accessibility APIs, Windows UI Automation, and Linux (X11 vs Wayland) differ substantially; feature parity across all three is genuinely hard. | Plugin-based observation architecture (per PROMPT.md) so platform-specific capability gaps are explicit and gracefully degrade rather than block the whole product. |
| Resource consumption (battery, CPU, disk) from continuous observation + local inference | M/M | A background app that's always watching and periodically running a local model can visibly hurt battery/performance — a fast way to get uninstalled. | Configurable observation intensity; batch/idle-time inference scheduling; self-diagnostics page exposing exactly what's costing what (per PROMPT.md's Self-Diagnostics requirement). |
| Encryption/key-management complexity leading to data loss | M/H | Encrypted local DB + OS credential vault integration is more failure-prone than plaintext SQLite; a botched migration or lost key can destroy months of accumulated insight. | Design key recovery/backup path from day one; test migration/upgrade paths as a first-class QA scenario, not an afterthought. |

## Market and adoption risks

| Risk | L/I | Detail | Mitigation direction |
|---|---|---|---|
| Category confusion at the point of sale | H/M | "Is this a screen recorder? An RPA tool? A time tracker?" — the pitch requires explaining what it's *not*, which is a harder sell than a feature list. | Positioning must lead with the differentiator (observe-and-recommend, never execute, local-first) in every first-run and marketing surface, per the differentiation strategy in [02](02-market-gaps-and-differentiation.md). |
| Incumbent response | M/M | If HiddenSteps demonstrates real demand, task-mining vendors (well-funded, enterprise-distribution-rich) could bolt on a "personal mode." | Durable differentiation has to be architectural (local-first, no aggregate employer view, execution-optional-by-design) and hard to retrofit onto an enterprise-surveillance business model — not just a feature checklist a competitor could copy in a quarter. |
| Regulatory drift | M/H | Workplace-monitoring and AI-transparency regulation (EU AI Act, US state biometric/monitoring laws, EU/UK worker-surveillance case law) is actively evolving; "observation software" is a regulatory target even when privacy-respecting. | Build the privacy-level/consent/audit-log architecture (per [05](05-privacy-analysis.md), [06](06-threat-model.md)) to be defensible under the strictest plausible near-term regime, not just current rules. |
| Small-vendor credibility gap | M/M | Unlike Celonis/UiPath, HiddenSteps has no enterprise sales-and-trust apparatus; individual users must trust a new, unaudited product with sensitive observational data. | Open, inspectable local storage; exportable/deletable data by design; ideally open-source core so trust claims are independently verifiable rather than asserted. |

## Ethical and legal risk (see [04-ethical-analysis.md](04-ethical-analysis.md) for full treatment)

| Risk | L/I | Detail |
|---|---|---|
| Secondary use of "quick win" recommendations to justify headcount reduction | M/H | If a recommendation ("this task takes 11 hours/month") reaches a manager, it becomes ammunition against the very employee it was meant to help. |
| Sensitive-data exposure through captured signals | M/H | Clipboard/browser/file signals can capture PII, PHI, credentials, or privileged legal/financial content even under "minimal" observation modes. |
