# HiddenSteps UI

TypeScript/React frontend (ADR-0001), communicating with the Rust core exclusively through `src/tauriBridge.ts` — a typed wrapper over the commands in [`../../../docs/design/09-api-specification.md`](../../../docs/design/09-api-specification.md) §2. No component calls `@tauri-apps/api` directly, so tests mock one small module instead of the whole Tauri runtime.

## What's implemented and tested here

- `OnboardingWizard` — the full 8-screen first-run flow from [`docs/ux/02-onboarding-flow.md`](../../../docs/ux/02-onboarding-flow.md), in the mandated order (FR-17): what it does / doesn't do, permissions explained, privacy level choice, provider detection + choice, a validation step that gates `Continue` on a real successful connectivity check, and a consent checkbox that gates `Start observing` — asserted by test to actually block progression, not just visually suggest it.
- `PrivacyDashboard` — the persistent trust surface from [`docs/ux/03-privacy-dashboard.md`](../../../docs/ux/03-privacy-dashboard.md): status line with pause/resume, the recent-events feed (rendered as exactly what `get_recent_events` returns, asserted by test), and the delete-all confirmation flow (asserted to require the second, explicit click before `delete_all_data` is ever called).
- `RecommendationCard` — the card/detail view from [`docs/ux/04-recommendations-ux.md`](../../../docs/ux/04-recommendations-ux.md), rendering PROMPT.md's own worked example shape, with the "Why?" detail expansion, mark-implemented, and dismiss-with-reason flows.
- `SettingsPage` — current privacy level (raise/lower) and the active AI provider list, from [`docs/ux/05-settings-and-complexity-tiers.md`](../../../docs/ux/05-settings-and-complexity-tiers.md).
- `DiagnosticsPage` — PROMPT.md's Self-Diagnostics page: real event/pattern/recommendation/audit-log counts and real on-disk storage size from `get_diagnostics`, plus an explicit line naming what it doesn't report yet (see below).
- `App.tsx` gates the whole app on `get_onboarding_state` (onboarding wizard vs. a four-tab shell: dashboard/recommendations/settings/diagnostics), asserted by test.

## What's not implemented (a real, disclosed gap)

- **Exclusion rules** and **plugin management** UI don't exist — the schema/backend work for per-app/per-domain exclusion rules wasn't built this milestone either (a real gap, not just a missing screen).
- **Progressive complexity tiers** (Beginner/Intermediate/Advanced, [`docs/ux/05-settings-and-complexity-tiers.md`](../../../docs/ux/05-settings-and-complexity-tiers.md)) aren't implemented as a display filter — `SettingsPage` currently shows one fixed view.
- Navigation is plain tab-button state, not the richer chrome the UX docs imply (no keyboard-shortcut-driven dashboard access, no tray-icon integration on the UI side).

## Running this yourself

```sh
npm install
npm test        # vitest, real jsdom rendering + @testing-library interaction
npx tsc -b       # typecheck
npm run build    # production build (used by `../src-tauri`'s beforeBuildCommand)
npm run dev      # dev server on :1420 (used by `../src-tauri`'s beforeDevCommand)
```

All of the above were actually run in this repository's dev environment as part of building this UI — unlike `../src-tauri`, nothing here required a system dependency this sandbox lacked. Current count: 26 tests across 6 test files, all passing.
