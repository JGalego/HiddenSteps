# Settings and Progressive Complexity

Implements FR-18: three complexity tiers gate *UI surface area*, not *capability* — every setting an Advanced user can reach, a Beginner user's account can also reach (via "Show advanced settings"), just not by default.

## Information architecture

```
Settings
├── Privacy                                    [all tiers]
│   ├── Privacy level (0-4)
│   ├── Excluded apps & sites
│   ├── Retention (Deep-mode TTL override)      [Intermediate+]
│   ├── Redaction sensitivity threshold          [Advanced only]
│   └── Consent history / manifest versions      [Advanced only]
├── AI Provider                                 [all tiers]
│   ├── Active provider + model
│   ├── Detected local runtimes
│   ├── Cloud provider keys                      [Intermediate+]
│   ├── Per-content-class cloud-send consent       [Advanced only]
│   └── Custom endpoint / self-hosted config       [Advanced only]
├── Recommendations                             [all tiers]
│   ├── Categories to include/exclude (shortcuts, scripts, RPA, ...)
│   ├── Notification frequency
│   └── Custom prompt templates for synthesis      [Advanced only]
├── Plugins                                      [Intermediate+]
│   ├── Installed plugins + granted capabilities
│   ├── Install from file
│   └── Capability usage log                       [Advanced only]
├── Enterprise Policy                             [visible only if a policy is loaded]
│   └── Read-only: current floor/allowlist, policy source
├── Diagnostics                                   [Intermediate+, summary visible to Beginner from a "Something wrong?" link]
│   ├── Provider/model status, GPU/CPU/memory/storage
│   ├── Observation permissions
│   ├── Security/encryption status
│   ├── Update status
│   └── Network activity log (air-gapped verification) [Advanced only]
└── About & Data
    ├── Export data                               [all tiers]
    ├── Delete all data                           [all tiers]
    └── Audit log                                 [Intermediate+]
```

## Tier definitions

| Tier | Default for | What's added vs. the tier below |
|---|---|---|
| Beginner | New users at first run | Just enough to onboard and use the privacy dashboard/recommendations; a single "Show advanced settings" toggle at the bottom of Settings reveals everything else without switching tiers permanently |
| Intermediate | Auto-suggested after a user changes any non-default setting, or opens Diagnostics unprompted | Model selection, plugin management, retention overrides, audit log — matches PROMPT.md's Intermediate spec directly |
| Advanced | Opt-in only, never auto-suggested | Custom prompts, custom providers/endpoints, custom observation plugins, per-content-class cloud consent granularity, enterprise controls visibility |

Tiers are a **display filter over one settings model**, not three different settings schemas — switching tiers never migrates or resets a value; it only changes what's rendered. This avoids the failure mode where a Beginner who briefly views Advanced settings, then switches back, loses a change they made.

## Wireframe — Settings home (Beginner view)

```
┌───────────────────────────────────────────────────────────────┐
│  Settings                                                      │
├───────────────────────────────────────────────────────────────┤
│  Privacy                                                        │
│    Level: 1 · App awareness                     [ Change ]     │
│    Excluded apps & sites (2)                      [ Manage ]    │
│                                                                 │
│  AI Provider                                                    │
│    Ollama (local) · llama3.1:8b                  [ Change ]     │
│                                                                 │
│  Recommendations                                                 │
│    Notify me: [ As they're found ▾ ]                            │
│                                                                 │
│  Data                                                            │
│    [ Export my data ]        [ Delete all data ]                │
│                                                                 │
│  Something wrong? [ Run diagnostics → ]                          │
│                                                                 │
│                                    [ Show advanced settings ▾ ] │
└───────────────────────────────────────────────────────────────┘
```

## Wireframe — Settings home (Advanced view, expanded)

```
┌───────────────────────────────────────────────────────────────┐
│  Settings                                       [Simple view] │
├───────────────────────────────────────────────────────────────┤
│  Privacy                                                        │
│    Level: 1 · App awareness                      [ Change ]     │
│    Excluded apps & sites (2)                       [ Manage ]    │
│    Deep-mode retention: 90 days                    [ Change ]    │
│    Redaction sensitivity: Conservative (default)    [ Change ]    │
│    Consent history: 3 manifest acknowledgments       [ View ]    │
│                                                                 │
│  AI Provider                                                     │
│    Active: Ollama (local) · llama3.1:8b             [ Change ]    │
│    Cloud providers configured: 0                     [ Add ]      │
│    Per-content-class cloud consent: none granted       [ Manage ] │
│                                                                 │
│  Plugins (2 installed)                                          │
│    Jira Workflow Observer · capabilities: observe,network  [Manage]│
│    Custom Recommendation Synthesizer                 [Manage]     │
│                                              [ Install plugin ]  │
│                                                                 │
│  Diagnostics                                     [ Full report → ]│
│  Enterprise Policy: none loaded                                   │
│                                                                 │
│  Data                                                             │
│    [ Export ]  [ Delete all ]  [ View audit log ]                │
└───────────────────────────────────────────────────────────────┘
```

## Design rule

Any setting that changes what's collected or where data can go (privacy level, exclusion rules, cloud provider consent) must be reachable from **both** the Beginner and Advanced views — those are never Advanced-gated, because gating them would mean a Beginner has no way to correct or tighten their own privacy configuration without "leveling up," contradicting the trust model's requirement that control be immediate and universal ([../design/04-trust-model.md](../design/04-trust-model.md) §2). Only settings that are purely about *sophistication of use* (custom prompts, custom endpoints, plugin capability micromanagement) are tier-gated.
