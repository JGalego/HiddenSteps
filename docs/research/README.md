# HiddenSteps — Phase 1: Research

Phase 1 deliverable set, per `PROMPT.md`. Produced before any architecture or product design work.

| Doc | Contents |
|---|---|
| [01-competitive-landscape.md](01-competitive-landscape.md) | Product-by-product analysis across Desktop AI Agents, Task Mining, Process Mining, Workflow Automation, Agent Frameworks, Local AI Platforms |
| [02-market-gaps-and-differentiation.md](02-market-gaps-and-differentiation.md) | What gap HiddenSteps fills, and what it should refuse to become |
| [03-risk-analysis.md](03-risk-analysis.md) | Product, market, technical, and adoption risks |
| [04-ethical-analysis.md](04-ethical-analysis.md) | Surveillance, consent, power-asymmetry, and dual-use concerns |
| [05-privacy-analysis.md](05-privacy-analysis.md) | Data minimization analysis and the case against raw retention |
| [06-threat-model.md](06-threat-model.md) | STRIDE-style threat model for the observation pipeline and data-at-rest |

## Methodology and evidence quality

The competitive landscape was researched via a multi-agent deep-research pass (5 search angles → 21 sources fetched → 79 claims extracted → 25 adversarially verified with a 2-of-3-refute kill rule → 16 confirmed / 9 refuted). That pass is the primary source for **DeskWand, Lapu.ai, Platonic, MindMirror, Microsoft Process Advisor, UiPath Task Mining/Process Mining**, and the **n8n/Zapier/Make/Power Automate deployment-model comparison** — those sections cite specific sources and confidence levels.

The same pass returned **zero verified claims** for Deka, OpenClaw Desktop, Celonis, IBM Process Mining, Pega Task Mining, and the entire Agent Frameworks / Local AI Platforms categories — either because they weren't substantively searched in that run, or because attempted claims failed adversarial verification. Rather than leave those blank, [01-competitive-landscape.md](01-competitive-landscape.md) fills them from general knowledge (training data through Jan 2026) and marks every such entry `[unverified — general knowledge, not independently re-checked]`. Two names — **Deka** and **OpenClaw Desktop** — could not be verified as real, currently-shipping products through either method; they are flagged as such rather than described with invented detail.

Anything load-bearing for a go/no-go product decision (pricing, security posture, exact capability boundaries of a close competitor) should be re-verified against primary sources before being cited externally — this is a snapshot assembled in one research pass, not a maintained market-intelligence feed.
