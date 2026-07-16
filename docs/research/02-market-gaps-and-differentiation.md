# Market Gaps and Differentiation Strategy

## The three existing categories, and why none of them fit

| Category | Unit of analysis | Optimizes for | Structural limit |
|---|---|---|---|
| Process mining (Celonis, UiPath, IBM) | The organization's business process | Finding value leakage across a process, at scale | Reconstructs from *systems of record* (event logs), not from how a person actually spends their day across apps, tabs, and ad-hoc tools. Individual has no agency. |
| Task mining (Pega, UiPath, Platonic, MS Process Advisor) | The individual's desktop actions, but for *organizational* benefit | Building a business case for automation investment | Data serves the employer's automation pipeline, not the employee's own understanding. Capture is either continuous-and-surveillant or session-based-and-narrow. |
| Desktop AI agents (DeskWand, Lapu.ai) | A single task, executed | Doing the work | No discovery layer across time; no "should this even be automated" judgment; execution carries its own risk and (often) its own privacy cost. |

HiddenSteps' unit of analysis is different from all three: **the individual's workflow, observed with their consent, for their own benefit, over weeks and months** — closer in spirit to a fitness tracker than to any of the above.

## The specific gap

No product found in the research (see [01](01-competitive-landscape.md)) combines all five of:

1. **Individual-level** — serves the person doing the work, not the organization mining them.
2. **Continuous and longitudinal** — weeks/months, not a single recorded session.
3. **Local-first / offline-capable** — can run and reason without a mandatory cloud dependency.
4. **Observe-and-recommend by default, not execute** — the product's default posture is advisory; automation is something the *user* chooses to build or approve, not something the tool silently does.
5. **Explainable across the full solution space** — a recommendation can be "use a keyboard shortcut" just as easily as "build an n8n flow" or "write a Python script," with reasoning, confidence, and alternatives shown.

Platonic comes closest on (3) and privacy-conscious observation, but stays enterprise/aggregate (1) and execution-adjacent-to-automation-pipeline rather than advisory (4). MindMirror comes closest on (1)+(3)+(4) but is an unreleased six-user academic prototype focused on emotional/reflective support, not workflow economics — and isn't a product.

## What HiddenSteps is uniquely positioned to do

- **Sit between categories, not inside one.** It can borrow process-mining's pattern-detection rigor, task-mining's desktop-level signal vocabulary, and automation platforms' execution targets — without inheriting any one category's business model (which is what pushes task/process mining toward employer-serving surveillance).
- **Make "should this be automated at all" a first-class question**, not an assumed yes. Every desktop agent and RPA/automation vendor is economically incentivized to say "yes, automate it, buy more of our product." HiddenSteps has no such incentive if it's not selling automation execution — it can honestly recommend "use this shortcut" over "buy this integration."
- **Compete on trust, not surveillance depth.** Every adjacent category (task mining especially) treats more observation as strictly better. HiddenSteps can treat *minimum sufficient observation* as the actual product quality bar — see [05-privacy-analysis.md](05-privacy-analysis.md).
- **Be usable by an individual with no IT department**, in under five minutes, with zero enterprise procurement — none of the process/task-mining tools can do this; they all require organizational deployment.
- **Recommend across the full automation spectrum** (shortcut → template → script → RPA → workflow platform → AI agent → hybrid) with an explicit difficulty/maintenance/privacy tradeoff analysis — no single competitor spans this range; each vendor above only ever recommends its own category of solution.

## What HiddenSteps must deliberately refuse to become

Differentiation is as much about restraint as capability:

- **Not a screen recorder.** Continuous screenshotting/OCR is the task-mining default and the fastest way to become "surveillance software" (see [04-ethical-analysis.md](04-ethical-analysis.md)). HiddenSteps should treat Deep/OCR modes as explicit, narrow, opt-in exceptions, not the default data source.
- **Not an execution agent by default.** DeskWand and Lapu.ai's core bet is "let the AI act for you." HiddenSteps' core bet is "let the human decide, with full information." Execution can be an optional downstream feature (e.g., one-click "set this up for me" after a recommendation is approved), but it is not the product's identity.
- **Not an enterprise-mining tool wearing a consumer skin.** The moment retained data primarily serves anyone other than the observed individual (a manager, an aggregate dashboard, a vendor's training set), HiddenSteps has become task mining under a different name.
- **Not cloud-dependent by default.** Every mature competitor in task/process mining assumes a cloud backend. HiddenSteps' local-first requirement is a genuine architectural commitment, not a checkbox — it changes what's technically possible for observation, storage, and inference, and that constraint is the point.

## Positioning statement

> HiddenSteps is a local-first workflow intelligence tool that watches how you actually work — not to do the work for you, and not to report on you — but to help you see, over time, where your own effort is going, and what your real options are for getting it back.
