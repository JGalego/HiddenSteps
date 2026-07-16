# Onboarding Flow (Wireframes)

Implements FR-17's mandated ordering exactly (PROMPT.md's First Run Experience) — the order is a requirement, not a suggestion: explain what it does → explain what it doesn't do → explain permissions → choose privacy level → choose provider → validate → consent → begin observing. No step may be skipped or reordered, and `start_observation` is architecturally unreachable before step 8 completes ([../design/03-data-flow-diagrams.md](../design/03-data-flow-diagrams.md) §2).

Wireframes are ASCII, screen-reader reading order is noted per screen (see [06-accessibility.md](06-accessibility.md) for the full accessibility spec).

## Screen 1 of 8 — What HiddenSteps does

```
┌─────────────────────────────────────────────────┐
│  ● ○ ○ ○ ○ ○ ○ ○                    Step 1 of 8  │
│                                                   │
│   Welcome to HiddenSteps                         │
│                                                   │
│   HiddenSteps learns how you work, over time,    │
│   and shows you specific ways to work less hard  │
│   at the repetitive parts — a shortcut, a         │
│   script, an automation, or just a better way     │
│   to organize a task.                            │
│                                                   │
│   It never acts on your behalf. It only          │
│   observes and suggests. You decide everything.  │
│                                                   │
│                                    [ Continue → ] │
└─────────────────────────────────────────────────┘
```
Reading order: title → body → Continue. "Continue" is the only focusable control besides a persistent "Learn more" link (opens the full [../research/02-market-gaps-and-differentiation.md](../research/02-market-gaps-and-differentiation.md) positioning statement in plain language, not the doc itself).

## Screen 2 of 8 — What HiddenSteps does NOT do

```
┌─────────────────────────────────────────────────┐
│  ● ● ○ ○ ○ ○ ○ ○                    Step 2 of 8  │
│                                                   │
│   What HiddenSteps will never do                 │
│                                                   │
│   ✗  Record video or audio of your screen        │
│      by default                                  │
│   ✗  Take actions on your computer without       │
│      you explicitly approving each one           │
│   ✗  Send your data anywhere without telling     │
│      you first                                   │
│   ✗  Share what it learns with your employer,    │
│      IT admin, or anyone else                     │
│   ✗  Require an account or internet connection   │
│      to work                                     │
│                                                   │
│                          [ ← Back ]  [ Continue → ]│
└─────────────────────────────────────────────────┘
```
This screen exists specifically to pre-empt the "is this spyware" reaction identified as the top trust risk ([../research/03-risk-analysis.md](../research/03-risk-analysis.md)) — stated as concretely as Screen 1's positive claims, not as a vague reassurance.

## Screen 3 of 8 — Permissions, explained before requested

```
┌─────────────────────────────────────────────────┐
│  ● ● ● ○ ○ ○ ○ ○                    Step 3 of 8  │
│                                                   │
│   Permissions HiddenSteps may ask for            │
│                                                   │
│   Depending on the privacy level you choose       │
│   next, your OS may ask you to approve:          │
│                                                   │
│   • Screen Recording / Accessibility access       │
│     → only needed for "Context-aware" or          │
│       "Maximum assistance" levels                │
│   • Automation / UI inspection access             │
│     → lets HiddenSteps see which app and window   │
│       you're using                                │
│                                                   │
│   You will only be asked for the permissions      │
│   your chosen level actually needs — not all      │
│   of them upfront.                                │
│                                                   │
│                          [ ← Back ]  [ Continue → ]│
└─────────────────────────────────────────────────┘
```
No OS permission dialog fires before this screen — and none fire for a permission tier the user's eventual level choice doesn't require ([../design/05-privacy-model.md](../design/05-privacy-model.md) §1, [../design/06-security-architecture.md](../design/06-security-architecture.md)'s elevation-of-privilege mitigation).

## Screen 4 of 8 — Choose privacy level

```
┌─────────────────────────────────────────────────┐
│  ● ● ● ● ○ ○ ○ ○                    Step 4 of 8  │
│                                                   │
│   How much should HiddenSteps see?               │
│                                                   │
│   ○ 0 · Manual — I'll add data myself. No         │
│         observation at all.                      │
│   ● 1 · App awareness (recommended to start)      │
│         Which apps you use, and when.             │
│   ○ 2 · Workflow awareness                        │
│         + browser domains, clipboard activity      │
│           type, file operations — never content.  │
│   ○ 3 · Context-aware                             │
│         + fuller in-app and browser context.      │
│   ○ 4 · Maximum assistance                        │
│         + optional screen text reading (OCR),      │
│           heavily filtered. Off unless you turn    │
│           it on here.                             │
│                                                   │
│   [ See exactly what each level collects → ]      │
│                                                   │
│                          [ ← Back ]  [ Continue → ]│
└─────────────────────────────────────────────────┘
```
"See exactly what each level collects" expands inline into the precise collected/retained/transmitted table from [../design/05-privacy-model.md](../design/05-privacy-model.md) — plain-language column headers, not the spec's own terminology. Level 1 is pre-selected as the least-invasive default with real value (per [../design/04-trust-model.md](../design/04-trust-model.md) §3's "earn the upgrade" principle); Level 4 is never pre-selected or visually emphasized as "recommended," avoiding the dark-pattern risk flagged in the ethical analysis.

## Screen 5 of 8 — Choose AI provider

```
┌─────────────────────────────────────────────────┐
│  ● ● ● ● ● ○ ○ ○                    Step 5 of 8  │
│                                                   │
│   Choose how HiddenSteps thinks                  │
│                                                   │
│   We found these on your computer:                │
│   ✓ Ollama — running locally, 2 models found      │
│     [ Use Ollama (llama3.1:8b) ]                  │
│                                                   │
│   Nothing else found. You can also:               │
│   ○ Install a local AI runtime (we'll guide you)  │
│   ○ Connect a cloud provider (OpenAI, Anthropic,  │
│     Google, and others) — requires an API key      │
│     and sends some data off your device           │
│                                                   │
│   Your hardware (16GB RAM, no dedicated GPU)       │
│   comfortably runs small-to-medium local models.  │
│                                                   │
│                          [ ← Back ]  [ Continue → ]│
└─────────────────────────────────────────────────┘
```
Local detection results are shown *before* the cloud option is even visible (scroll or expand to reveal), operationalizing FR-14/15 and the "local-first is structurally enforced during onboarding" rule from [../design/adr/0004-llm-provider-trait-local-first.md](../design/adr/0004-llm-provider-trait-local-first.md). If the user picks "Install a local AI runtime," a guided sub-flow (not detailed here) walks through installing Ollama and pulling a recommended model, then returns to this screen.

## Screen 6 of 8 — Validate configuration

```
┌─────────────────────────────────────────────────┐
│  ● ● ● ● ● ● ○ ○                    Step 6 of 8  │
│                                                   │
│   Checking your setup...                          │
│                                                   │
│   ✓ Ollama connection: OK (42ms)                  │
│   ✓ Model loaded: llama3.1:8b                     │
│   ✓ Encrypted storage: initialized                │
│   ✓ OS credential vault: accessible               │
│                                                   │
│                          [ ← Back ]  [ Continue → ]│
└─────────────────────────────────────────────────┘
```
On any failure (e.g., provider unreachable), this screen shows a specific, actionable error inline and blocks "Continue" — never silently proceeds with a non-functional provider.

## Screen 7 of 8 — Final summary and consent

```
┌─────────────────────────────────────────────────┐
│  ● ● ● ● ● ● ● ○                    Step 7 of 8  │
│                                                   │
│   Ready to start                                  │
│                                                   │
│   Level 1 · App awareness                         │
│   Ollama (local) · llama3.1:8b                    │
│   Nothing leaves your device.                     │
│                                                   │
│   You can change any of this — or pause or        │
│   delete everything — at any time from the        │
│   privacy dashboard.                              │
│                                                   │
│   ☐ I understand what HiddenSteps will observe    │
│     and consent to start.                        │
│                                                   │
│                    [ ← Back ]  [ Start observing ]│
└─────────────────────────────────────────────────┘
```
"Start observing" is disabled until the checkbox is checked — this is the explicit consent gate FR-17 requires; it is a real, separate affirmative action, not the same click as "Continue" on prior screens.

## Screen 8 of 8 — Confirmation

```
┌─────────────────────────────────────────────────┐
│  ● ● ● ● ● ● ● ●                    Step 8 of 8  │
│                                                   │
│   ✓ HiddenSteps is now observing                  │
│                                                   │
│   You'll usually hear from us within a day —       │
│   sooner if something obvious turns up.           │
│                                                   │
│   The privacy dashboard is always one click        │
│   away from the tray/menu-bar icon.               │
│                                                   │
│                              [ Go to dashboard ]  │
└─────────────────────────────────────────────────┘
```

## Complexity-tier interaction

This exact 8-screen flow is identical for Beginner/Intermediate/Advanced (per [05-settings-and-complexity-tiers.md](05-settings-and-complexity-tiers.md) — progressive complexity applies to ongoing settings surface area, not to onboarding, since informed consent must be complete regardless of technical sophistication). An Advanced user sees the same screens; they simply have more to configure later from Settings.
