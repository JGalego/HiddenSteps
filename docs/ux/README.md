# HiddenSteps — Phase 3: UX

Phase 3 deliverable set, per `PROMPT.md`. Every screen and interaction here is a direct rendering of a guarantee established in Phase 2 (`../design/`) — the UX's job is to make those guarantees legible, not to introduce new ones.

## Reading order

1. [01-user-journeys.md](01-user-journeys.md) — five end-to-end journeys (first 24 hours, daily trust verification, recommendation review, enterprise deployment, portable mode) mapped to the personas in [../design/01-prd.md](../design/01-prd.md).
2. [02-onboarding-flow.md](02-onboarding-flow.md) — screen-by-screen wireframes for the mandated 8-step first-run flow (FR-17), with the reasoning for why each screen's content and ordering is fixed, not just conventional.
3. [03-privacy-dashboard.md](03-privacy-dashboard.md) — the persistent trust surface: status, live recent-events feed, pause/exclude/export/delete, audit log — each element traced back to the specific guarantee it makes verifiable.
4. [04-recommendations-ux.md](04-recommendations-ux.md) — the recommendation card and detail view, implementing PROMPT.md's own "Automation Architect" dialogue example field-for-field.
5. [05-settings-and-complexity-tiers.md](05-settings-and-complexity-tiers.md) — the Beginner/Intermediate/Advanced information architecture, and the rule for which settings must never be tier-gated.
6. [06-accessibility.md](06-accessibility.md) — screen reader, keyboard, visual, and localization requirements, plus the Phase 4 testing bar.

## Wireframe convention

Wireframes are ASCII sketches illustrating layout, content, and information hierarchy — not literal typography. The shipped UI (TypeScript/React inside Tauri's WebView, per [../design/adr/0001-desktop-shell-tauri.md](../design/adr/0001-desktop-shell-tauri.md)) uses normal proportional fonts, real spacing, and platform-appropriate visual styling; the box-drawing layout here exists only to fix element order, grouping, and copy, which is what actually needs to be reviewed and agreed on before implementation.

## Design principle carried through every screen

Per [../design/04-trust-model.md](../design/04-trust-model.md): a wary new user has no basis yet to trust HiddenSteps beyond what a given screen shows them in the moment. Every wireframe in this set was checked against the question *"if I didn't trust this app yet, would this screen give me a reason to, or a reason not to?"* — copy that hedges, buries a permission request, or frames a recommendation as evaluative rather than helpful failed that check and was rewritten.

## Next: Phase 4

Implementation roadmap, milestones, technology choices (already substantially fixed by Phase 2's ADRs), testing strategy, security testing, privacy testing, performance testing — per `PROMPT.md`'s Phase 4.
