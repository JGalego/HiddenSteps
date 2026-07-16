# Recommendations UX

Directly implements PROMPT.md's "Automation Architect" dialogue example and FR-10/FR-11/FR-13's explainability requirements. Every recommendation card is a rendering of one `recommendations` row ([../design/07-database-schema.md](../design/07-database-schema.md)) plus its linked `patterns`/`pattern_events` traceability.

## Wireframe — recommendation card (list view)

```
┌───────────────────────────────────────────────────────────────┐
│  📋 Copying ticket data into the weekly report                 │
│                                                                 │
│  Observed 31 times over the last 2 weeks.                      │
│  Estimated cost: ~11 hours/month.                               │
│                                                                 │
│  Recommended: Hybrid — Playwright + local LLM                   │
│  Confidence: ●●●●○ High · Difficulty: Medium                    │
│                                                                 │
│  [ Why? ]  [ See alternatives ]  [ Implemented ]  [ Dismiss ]  │
└───────────────────────────────────────────────────────────────┘
```

This is a direct rendering of PROMPT.md's own worked example:

> "I observed this workflow 31 times over the last two weeks. Estimated monthly cost: 11 hours. Possible solutions: Excel Macro · Python · Playwright · n8n · Power Automate · AI Agent · Hybrid workflow. Recommended approach: Hybrid workflow using Playwright + local LLM because it offers the best balance of reliability, privacy, and maintenance."

The card format is the compressed version of that dialogue; expanding "Why?" reveals the full explanation.

## Wireframe — detail view ("Why?" expanded)

```
┌───────────────────────────────────────────────────────────────┐
│  ← Back              Copying ticket data into the weekly report│
├───────────────────────────────────────────────────────────────┤
│  WHAT WE SAW                                                   │
│  31 occurrences, Jul 1–14. Pattern: Jira → clipboard →          │
│  Excel → save, repeated ~2.2×/day on weekdays.                  │
│  [ View the 31 contributing observations → ]                   │
│                                                                 │
│  WHY THIS RECOMMENDATION                                        │
│  This exact app sequence recurs with high regularity and        │
│  low variation — a strong candidate for automation rather       │
│  than a one-off shortcut, but the source data (Jira ticket       │
│  fields) is structured enough that a script can read it          │
│  directly instead of relying on clipboard copying.               │
│                                                                 │
│  CONFIDENCE: High                                                │
│  Based on 31 consistent occurrences across 10 working days.      │
│  Confidence would be Medium below ~10 occurrences or with        │
│  higher sequence variation.                                      │
│                                                                 │
│  ASSUMPTIONS MADE                                                │
│  • You have (or can get) API access to your Jira instance        │
│  • The report format hasn't changed in the observed window       │
│                                                                 │
│  WHAT WE IGNORED                                                 │
│  • 3 occurrences on a different machine — not correlated          │
│    across devices (not currently supported)                      │
│  • Whether this task is actually still needed at all — that's    │
│    a judgment only you can make                                 │
│                                                                 │
│  ALTERNATIVES CONSIDERED                                         │
│  ┌─────────────────┬──────────┬─────────────┬────────────────┐ │
│  │ Approach         │ Effort   │ Maintenance │ Privacy        │ │
│  ├─────────────────┼──────────┼─────────────┼────────────────┤ │
│  │ Excel macro      │ Low      │ Low         │ Fully local     │ │
│  │ Python script     │ Medium   │ Medium      │ Fully local     │ │
│  │ Playwright        │ Medium   │ Medium      │ Fully local     │ │
│  │ n8n workflow      │ Medium   │ Low         │ Local if self-  │ │
│  │                   │          │              │ hosted          │ │
│  │ Power Automate     │ Low      │ Low         │ Cloud (MS)      │ │
│  │ ★ Hybrid (chosen)  │ Medium   │ Low         │ Fully local     │ │
│  └─────────────────┴──────────┴─────────────┴────────────────┘ │
│                                                                 │
│  ESTIMATED TIME SAVED: ~11 hours/month                           │
│  IMPLEMENTATION EFFORT: ~2-3 hours one-time setup                │
│                                                                 │
│              [ Mark implemented ]      [ Dismiss ▾ ]           │
└───────────────────────────────────────────────────────────────┘
```

Every labeled section above maps 1:1 to a `recommendations` column ([../design/07-database-schema.md](../design/07-database-schema.md)): WHAT WE SAW → `pattern_events` join, WHY → `why`, CONFIDENCE → `confidence`, ASSUMPTIONS → `assumptions_json`, WHAT WE IGNORED → `ignored_information_json`, ALTERNATIVES → `alternatives_json`. This is deliberate — the UI cannot render a recommendation that's missing any of these fields, which is how FR-10's "every recommendation must include" requirement gets enforced rather than just documented.

## Dismiss flow — capturing a reason (feeds ranking)

```
┌─────────────────────────────────────────────┐
│  Why are you dismissing this?                │
│                                               │
│  ○ Not worth the effort                       │
│  ○ I already do this a different way          │
│  ○ Wrong / doesn't match what I actually do    │
│  ○ Privacy concern with the approach           │
│  ○ Other: [___________________]               │
│                                               │
│                    [ Cancel ]   [ Dismiss ]   │
└─────────────────────────────────────────────┘
```
"Wrong / doesn't match" dismissals are weighted most heavily in future pattern-detection tuning (a correctness signal), distinct from "not worth the effort" (a preference signal) — both stored in `dismissal_reason` but treated differently by the ranking logic, never conflated into one generic "not interested" bucket.

## Notification pattern (first recommendation, Zero-to-Value moment)

```
┌───────────────────────────────────┐
│  🔍 HiddenSteps found something     │
│  You've repeated a task 6 times     │
│  today. Worth a look?               │
│              [ Dismiss ] [ Show me ]│
└───────────────────────────────────┘
```
Framed as a discovery ("found something"), never as a warning or an audit finding — directly implementing the "recommendation-as-surveillance-artifact" mitigation from [../design/04-trust-model.md](../design/04-trust-model.md) §5: this must read as something *for* the user, not a report *about* the user.

## Empty state (honest, per FR-12)

```
┌───────────────────────────────────────────────────────────────┐
│  Still learning your patterns.                                 │
│                                                                 │
│  Nothing has repeated often enough yet to suggest a change.     │
│  Most people see their first recommendation within a day or     │
│  two of active use.                                             │
└───────────────────────────────────────────────────────────────┘
```
