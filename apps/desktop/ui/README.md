# HiddenSteps UI

TypeScript/React frontend (ADR-0001), communicating with the Rust core exclusively through `src/tauriBridge.ts` — a typed wrapper over the commands in [`../../../docs/design/09-api-specification.md`](../../../docs/design/09-api-specification.md) §2. No component calls `@tauri-apps/api` directly, so tests mock one small module instead of the whole Tauri runtime.

## What's implemented and tested here

- `PrivacyDashboard` — the persistent trust surface from [`docs/ux/03-privacy-dashboard.md`](../../../docs/ux/03-privacy-dashboard.md): status line with pause/resume, the recent-events feed (rendered as exactly what `get_recent_events` returns, asserted by test), and the delete-all confirmation flow (asserted to require the second, explicit click before `delete_all_data` is ever called).
- `RecommendationCard` — the card/detail view from [`docs/ux/04-recommendations-ux.md`](../../../docs/ux/04-recommendations-ux.md), rendering PROMPT.md's own worked example shape, with the "Why?" detail expansion, mark-implemented, and dismiss-with-reason flows.

## What's not implemented (a real, disclosed gap)

Onboarding ([`docs/ux/02-onboarding-flow.md`](../../../docs/ux/02-onboarding-flow.md)'s 8 screens), Settings ([`docs/ux/05-settings-and-complexity-tiers.md`](../../../docs/ux/05-settings-and-complexity-tiers.md)), and Diagnostics have no components here yet. `App.tsx` is a minimal shell wiring the two components above together, not the full app navigation. Scope was prioritized toward the two components with the highest concentration of trust/explainability requirements (per the UX docs), verified end-to-end with real tests, over broader-but-shallower coverage.

## Running this yourself

```sh
npm install
npm test        # vitest, real jsdom rendering + @testing-library interaction
npx tsc -b       # typecheck
npm run build    # production build (used by `../src-tauri`'s beforeBuildCommand)
npm run dev      # dev server on :1420 (used by `../src-tauri`'s beforeDevCommand)
```

All of the above were actually run in this repository's dev environment as part of building this UI — unlike `../src-tauri`, nothing here required a system dependency this sandbox lacked.
