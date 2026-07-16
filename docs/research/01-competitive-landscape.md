# Competitive Landscape

Evidence key: **[R]** = confirmed by the deep-research pass (adversarially verified, sourced) · **[G]** = general knowledge, not independently re-checked in this pass · **[U]** = unverified, treat as possibly obscure/renamed/nonexistent.

---

## Desktop AI Agents

These are the closest surface-level competitors — desktop-resident, AI-driven — but every verified one in this category is an **execution** agent, not an observation/advisory one. That distinction is the crux of HiddenSteps' differentiation (see [02](02-market-gaps-and-differentiation.md)).

### DeskWand — [R] high confidence

| | |
|---|---|
| Target audience | Individuals/prosumers wanting to delegate repetitive computer work |
| Architecture | Open-source, local-first (Windows/macOS/Linux), bring-your-own model key |
| Observation | Watches the user perform a task once, then captures the action sequence |
| Automation | **Executes** — opens browsers, fills forms, moves data; replays captured workflows as one-click flows |
| Privacy model | Data stays on-device; cloud sync is opt-in (claim only 2-of-3 verified as an architecture description, not as a rigorous privacy-guarantee comparator) |
| Strengths | Zero-friction "record once, replay forever"; genuinely local-first for the parts that are local |
| Weaknesses | It's an actor, not an advisor — no concept of "should this be automated," no explanation, no alternative-approaches comparison; execution risk (a replayed flow can silently misfire if the UI changes) |
| Pricing / deployment | Not confirmed in this pass |

**Why it's not what HiddenSteps is:** DeskWand's entire value proposition is "I will do this for you again." HiddenSteps' proposition is "here's why you're doing this, and here are five ways to stop doing it manually — you decide." DeskWand has no discovery layer across weeks/months, no cross-workflow reasoning, and no privacy-by-default posture once execution is in play (an execution agent needs broad permissions HiddenSteps' observation-only model doesn't).

### Lapu.ai Desktop AI — [R] high confidence

| | |
|---|---|
| Target audience | Individuals wanting local file/desktop task execution |
| Architecture | Hybrid: local execution of file/shell/desktop ops, but AI **reasoning is cloud-dependent** (routed to a hosted frontier model) |
| Observation | Not primarily observation-first; oriented around directed task execution |
| Automation | Explicit design goal: "to do the work, not just describe it" |
| Privacy model | Markets local file execution as reducing cloud-breach exposure, but reasoning itself leaves the device — not offline-capable |
| Strengths | Frontier-model reasoning quality without shipping files wholesale to the cloud |
| Weaknesses | Not local-first end-to-end (contradicts a "local-first, privacy-first" positioning); no continuous workflow-discovery capability |
| Pricing / deployment | Not confirmed in this pass |

### Deka — [U] unverified

No claims about a product named "Deka" survived research or independent verification in this pass. It could not be confirmed as a real, currently-shipping desktop AI agent. **Do not cite this name externally without independent re-verification** — it may be obscure, renamed, defunct, or a misremembered name from the original brief.

### OpenClaw Desktop — [U] unverified

Same status as Deka: no confirmed claims, existence not verified. Flag as needing independent verification before further use in competitive materials.

---

## Task Mining

### Platonic (platonicresearch.com) — [R] medium confidence

| | |
|---|---|
| Target audience | Enterprises running task-mining discovery programs |
| Architecture | Early-stage startup product |
| Observation | Screen-level telemetry: periodic screenshots, active window titles, clicks, scrolls, keyboard shortcuts, special keys |
| Privacy model | Explicitly **excludes** typed-text content, camera, microphone, continuous video capture — a genuine privacy boundary, though only vendor-documented (2-of-3 verified, no independent audit found) |
| Deployment | Markets on-premises and private-cloud options for data-residency-sensitive orgs |
| Strengths | Privacy-conscious observation design is closer to HiddenSteps' philosophy than any other product found in this research |
| Weaknesses | Thinly documented — no public pricing, security whitepaper, case studies, or third-party review; small/early-stage, could pivot |
| Pricing | Not published |

This is the single closest philosophical analog found among commercial products, but it's enterprise/aggregate-discovery-oriented (feeding a business case), not individual/continuous/advisory-oriented like HiddenSteps.

### Pega Task Mining — [G] general knowledge, not re-verified

| | |
|---|---|
| Target audience | Large enterprises already on the Pega platform, BPM/ops teams |
| Architecture | Client-side desktop recorder feeding a central analytics backend (cloud or on-prem Pega infrastructure) |
| Observation | Session-based and continuous capture modes of clicks, keystrokes (typically masked/redacted), application/window context |
| Automation | Feeds directly into Pega's own RPA/case-management automation build pipeline |
| Privacy model | Enterprise IT-governed; individual employees have essentially no control over what is captured or retained |
| Strengths | Deep integration with Pega's automation and case-management stack; mature enterprise deployment tooling |
| Weaknesses | Built for IT/ops to mine the workforce in aggregate, not for an individual to understand and improve their own work; not local-first; no individual agency over data |
| Pricing / deployment | Enterprise licensing, not self-serve |

### UiPath Task Mining — [R] high confidence

| | |
|---|---|
| Observation | Installed client app captures granular desktop-level clicks and keystrokes (PII-masked) to build task-flow diagrams |
| Automation | Discovery output is explicitly designed to feed UiPath's Automation Hub / RPA bot-deployment pipeline — an ROI/business-case funnel, not individual advice |
| Target audience | Enterprise automation/CoE teams |
| Deployment | Cloud or on-prem UiPath platform |
| Strengths | Mature, well-integrated with the broader UiPath RPA ecosystem |
| Weaknesses | Aggregate/business-case orientation; individual worker has no visibility into or control over their own captured data; not privacy-by-default |

### Microsoft Process Advisor (Power Automate) — [R] high confidence

| | |
|---|---|
| Observation | **Discrete, opt-in, session-based recording** — user manually starts a recorder, performs the task, stops, uploads; the tool generates a flow map. No ambient/always-on capture. |
| Target audience | Power Automate users wanting to document and later automate a specific process |
| Automation | Recording feeds Power Automate flow design, not an autonomous recommendation engine |
| Strengths | Session-based capture is inherently more privacy-respecting than continuous monitoring — worth borrowing as a *pattern*, not a full solution |
| Weaknesses | Manual/session-based capture misses the passive, longitudinal, cross-application pattern discovery that is HiddenSteps' whole premise; single-task scope, no cross-workflow reasoning |

---

## Process Mining

### Celonis — [G] general knowledge, not re-verified (a specific "event-log-only" claim was explicitly attempted and refuted in this pass — treat details below as directional, not verified)

| | |
|---|---|
| Target audience | Large enterprises, process excellence / ops / finance teams |
| Architecture | Reconstructs end-to-end processes primarily from backend system event logs (ERP, CRM, ITSM) rather than direct desktop observation, supplemented by task-mining capture in some product lines |
| Automation | Process-intelligence platform that surfaces bottleneck/variance insights and can trigger downstream automations (its own "Process Automation" capability plus RPA integrations) |
| Privacy model | Enterprise data-governance model; not designed around individual worker privacy or consent |
| Strengths | The category leader; extremely mature analytics on business-process variance and value leakage at the org level |
| Weaknesses | Business-process lens, not individual-workflow lens; heavy enterprise deployment/integration lift; not something an individual could install and get value from in a day |
| Pricing | Enterprise, typically six/seven-figure annual contracts |

### UiPath Process Mining — [R] high confidence
Positioned to feed discovery output into UiPath's own automation pipeline, same ecosystem logic as UiPath Task Mining above — process-level (event-log) rather than desktop-level observation.

### IBM Process Mining — [G] general knowledge, not re-verified

| | |
|---|---|
| Target audience | Enterprises, often already IBM/Cloud Pak customers |
| Architecture | Event-log-based process reconstruction, integrated into IBM's broader automation (Cloud Pak for Business Automation) suite |
| Strengths | Integrates with IBM's existing enterprise automation/RPA tooling |
| Weaknesses | Same category limitation as Celonis/UiPath: business-process aggregate lens, enterprise deployment complexity, no individual-level product |

**Category-level takeaway:** every process-mining product reconstructs processes from *systems of record*, not from how an individual actually spends their day across applications, browser tabs, and ad-hoc tools. That's a structural blind spot HiddenSteps can occupy (see [02](02-market-gaps-and-differentiation.md)).

---

## Workflow Automation

### n8n, Zapier, Make, Power Automate — [R] high confidence (deployment-model comparison)

The confirmed finding: **n8n is the only one of the four offering true self-hosting** — the workflow engine and data can stay entirely inside the user's own infrastructure. Zapier and Make offer only network-level bridges to reach on-prem systems (VPC peering, an "on-premise agent"); Power Automate offers an on-premises data gateway. None of the other three let you self-host the core engine itself. This is commonly marketed as a data-governance/compliance differentiator for n8n specifically.

| | n8n | Zapier | Make | Power Automate |
|---|---|---|---|---|
| Deployment | Self-hostable (open-source) or cloud | Cloud-only | Cloud-only | Cloud-only (Microsoft-hosted) |
| Data residency control | Full, if self-hosted | None (bridge only) | None (bridge only) | Gateway only |
| Target audience | Technical users, teams wanting data control | Broad SMB/prosumer | Broad SMB/prosumer | Microsoft 365 / Power Platform shops |

None of these four are competitors to HiddenSteps directly — they're **candidate implementation targets** HiddenSteps' recommendation engine should be able to point at ("this repeated task could become an n8n flow"). Their AI capabilities, pricing specifics, and observation mechanics were not confirmed in this research pass and shouldn't be cited without a fresh check.

---

## Agent Frameworks (adjacent infrastructure, not competitors) — [G] general knowledge

These matter to HiddenSteps only as *potential building blocks* for its own recommendation/reasoning layer or as targets it might recommend building on, not as competitors — none are positioned as personal workflow-observation products.

| Framework | Nature | Relevance to HiddenSteps |
|---|---|---|
| LangChain / LangGraph | General agent-orchestration frameworks; LangGraph adds explicit state-graph control flow | Candidate for HiddenSteps' own internal recommendation/reasoning orchestration, or a target for recommended custom agent builds |
| CrewAI | Multi-agent "crew" orchestration framework | Same — a possible recommendation target for hybrid workflows |
| OpenAI Agents SDK | OpenAI's own lightweight agent-building SDK | Candidate provider-side integration if the user opts into OpenAI as a provider |
| Semantic Kernel / AutoGen | Microsoft's agent/orchestration frameworks (there is a claimed 2026 unification into a single "Microsoft Agent Framework" — this specific claim did **not** survive adversarial verification in this pass, so treat the merger as plausible-but-unconfirmed rather than fact) | Enterprise-integration relevance if targeting Microsoft-shop users |
| LlamaIndex | Primarily a RAG/data-indexing framework, with an agent-workflow layer added over time | Relevant to HiddenSteps' own embedding/knowledge-base layer more than as a competitor |

None of these frameworks are marketed as, or architecturally suited to be, passive individual work-pattern observers. They are reasoning/orchestration substrate, not observation products.

---

## Local AI Platforms (adjacent infrastructure, not competitors) — [G] general knowledge

| Platform | Nature | Relevance |
|---|---|---|
| Ollama | Simple local model runner built on llama.cpp; the default "just works" on-ramp for local LLMs | **Primary integration target** for HiddenSteps' local-first AI provider layer — matches the "detect and auto-configure" onboarding requirement |
| LM Studio | GUI-first local model runner/chat client | Secondary integration target; good for less technical users who already have it installed |
| llama.cpp | The underlying inference engine most local tools build on; CPU-first with optional GPU acceleration, targets consumer hardware | Fallback/embedded inference option for HiddenSteps if bundling a model directly |
| vLLM | High-throughput inference server built for GPU-accelerated serving of concurrent requests (PagedAttention) | Relevant for power users self-hosting a shared inference server, not typical single-user desktop use |
| LocalAI | OpenAI-API-compatible local inference server | Alternative local backend, useful if HiddenSteps standardizes its provider interface on the OpenAI API shape |

None of these are competitors — they are exactly the kind of local-AI runtime HiddenSteps' "Local AI (Mandatory)" requirement should auto-detect and integrate with.

---

## Cross-cutting summary

| Category | What it optimizes for | What it's missing (relative to HiddenSteps) |
|---|---|---|
| Desktop AI agents (DeskWand, Lapu.ai) | Executing a task on the user's behalf | No discovery layer, no "should I even automate this" judgment, execution risk, and (for Lapu.ai) not actually local-first |
| Task/Process mining (Pega, UiPath, Celonis, IBM, MS Process Advisor) | Aggregate, business-case-driven process discovery for the *organization* | No individual agency, not privacy-by-default, not local-first, not continuous+passive at the individual level |
| Workflow automation (n8n, Zapier, Make, Power Automate) | Executing a *predefined* workflow reliably | No discovery of what should be automated in the first place — pure execution engines |
| Platonic | Privacy-conscious enterprise task-mining telemetry | Still enterprise/aggregate-oriented, not individual-advisory |
| MindMirror (academic, not commercial) | Local-first, multimodal, state-aware individual support | Emotional/reflective support, not workflow-improvement recommendations; six-user unreleased prototype, not a product |

No product found across all 16 named names plus adjacent infrastructure combines: (1) individual-level, (2) continuous/longitudinal, (3) local-first/offline-capable, (4) observation-and-recommend-only (no execution by default), and (5) explainable recommendations spanning shortcuts → scripts → RPA → AI agents. That combination is HiddenSteps' addressable gap — detailed in [02-market-gaps-and-differentiation.md](02-market-gaps-and-differentiation.md).
