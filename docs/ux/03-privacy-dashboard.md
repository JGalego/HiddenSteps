# Privacy Dashboard

The single most trust-load-bearing surface in the product ([../design/04-trust-model.md](../design/04-trust-model.md) §2) — every trust feature PROMPT.md requires lives here, reachable in one click/shortcut from anywhere in the app or from the tray/menu-bar icon.

## Wireframe — main view

```
┌───────────────────────────────────────────────────────────────┐
│  Privacy Dashboard                                    [ ⓧ ]   │
├───────────────────────────────────────────────────────────────┤
│  ● Observing — Level 1 · App awareness        [ Pause ]        │
│  Provider: Ollama (local) · llama3.1:8b        [ Change level ]│
│                                                                 │
│  ┌─── What's being captured right now ──────────────────────┐ │
│  │  10:42:03  App focus → VS Code                            │ │
│  │  10:41:58  App focus → Slack                               │ │
│  │  10:41:40  Shortcut used → Cmd+Shift+4 (screenshot)         │ │
│  │  10:39:12  App focus → Chrome                              │ │
│  │  ...                                             [ More ] │ │
│  └────────────────────────────────────────────────────────────┘ │
│                                                                 │
│  [ Exclude an app/site ]  [ Export my data ]  [ Delete data ▾ ]│
│                                                                 │
│  Storage: 4.2 MB encrypted · Last backup: never · [Diagnostics]│
└───────────────────────────────────────────────────────────────┘
```

- **Status line** (`observation::status_changed` event) is always the first thing rendered, never below the fold.
- **Recent-events feed** streams live via `observation::event_captured` ([../design/09-api-specification.md](../design/09-api-specification.md) §3), showing the exact post-redaction summary that was stored — not a paraphrase. This is deliberate: it lets a skeptical user directly compare "what the privacy level promised" against "what actually got captured," which is the dashboard's entire reason for existing.
- **Pause** takes effect within one render frame of the click — no confirmation dialog (pausing is the safe direction; confirmation friction belongs on irreversible actions like delete, not on the "do less" action).
- **Delete data** expands into: "Delete selected events," "Delete everything," each with a distinct, honestly-worded confirmation (see below) — never a single ambiguous "Delete" button.

## Delete confirmation (irreversible-action pattern)

```
┌─────────────────────────────────────────────┐
│  Delete all HiddenSteps data?                 │
│                                               │
│  This removes every captured summary,        │
│  pattern, recommendation, and setting —       │
│  permanently. This cannot be undone.          │
│                                               │
│  Your encryption key will also be deleted,    │
│  so even a backup copy of this data becomes   │
│  unreadable.                                  │
│                                               │
│         [ Cancel ]      [ Delete everything ]│
└─────────────────────────────────────────────┘
```
Copy explicitly states the key-deletion consequence from [../design/06-security-architecture.md](../design/06-security-architecture.md) §2 — a user should understand *why* this is irreversible, not just that it is.

## "What's being captured" — expanded/filtered view

```
┌───────────────────────────────────────────────────────────────┐
│  Recent activity                    Filter: [ All sources ▾ ]  │
├───────────────────────────────────────────────────────────────┤
│  Time      Source           Type                 Detail        │
│  10:42:03  App observer      app_focus_change      VS Code      │
│  10:41:58  App observer      app_focus_change      Slack        │
│  10:41:40  Shortcuts         shortcut_used          Cmd+Shift+4  │
│  10:39:12  Browser observer   domain_visited         github.com  │
├───────────────────────────────────────────────────────────────┤
│  Nothing here was screen content, keystrokes typed, or          │
│  clipboard contents — Level 1/2 never capture those.             │
│                                            [ Back to dashboard ] │
└───────────────────────────────────────────────────────────────┘
```
The reassurance line at the bottom is level-specific and generated from the same manifest data driving [../design/05-privacy-model.md](../design/05-privacy-model.md) §1 — it is never a static string that could drift out of sync with what the level actually captures.

## Exclusion rules panel

```
┌───────────────────────────────────────────────────────────────┐
│  Excluded apps & sites                          [ + Add rule ]│
├───────────────────────────────────────────────────────────────┤
│  1Password                              (app)         [ ✕ ]    │
│  *.mychartpatientportal.com             (domain)       [ ✕ ]    │
├───────────────────────────────────────────────────────────────┤
│  HiddenSteps suggested excluding "Epic Hyperspace" —            │
│  it looks like an EHR application.        [ Exclude ]  [ No ] │
└───────────────────────────────────────────────────────────────┘
```
The proactive suggestion row implements the ethical-analysis mitigation for sensitive-population applications ([../research/04-ethical-analysis.md](../research/04-ethical-analysis.md)) — offered, never auto-applied, since a false-positive exclusion (e.g., misdetecting a benign app) should cost the user one click to dismiss, not silently degrade their observation without notice.

## Audit log view (Diagnostics-adjacent, linked from dashboard)

```
┌───────────────────────────────────────────────────────────────┐
│  Activity log                                                  │
├───────────────────────────────────────────────────────────────┤
│  2026-07-16 09:14  Privacy level changed: 0 → 1                │
│  2026-07-16 09:14  Observation started                          │
│  2026-07-16 14:02  Rule added: exclude 1Password                │
├───────────────────────────────────────────────────────────────┤
│  This log records actions, never the content HiddenSteps         │
│  observed. [ Export full data instead → ]                       │
└───────────────────────────────────────────────────────────────┘
```

## Component-to-guarantee traceability

| Dashboard element | API source | Guarantee it makes verifiable |
|---|---|---|
| Status line | `get_observation_status`, `observation::status_changed` | "You always know if it's watching" |
| Recent-events feed | `get_recent_events`, `observation::event_captured` | "What's collected is exactly what's shown" |
| Pause/Resume | `pause_observation`/`resume_observation` | "You can stop it instantly" |
| Exclude rule | `set_exclusion_rule` | "You control the boundaries" |
| Export | `export_data` | "You can leave with everything, anytime" |
| Delete | `delete_events`/`delete_all_data` | "You can make it forget, verifiably" |
| Activity log | `get_audit_log` | "Actions taken are recorded, not just promised" |
