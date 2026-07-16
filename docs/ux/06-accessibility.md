# Accessibility and Localization

Operationalizes FR-23/FR-24 (WCAG 2.1 AA target, tested in Phase 4) and PROMPT.md's Installation & Onboarding accessibility requirements. Because Tauri renders the UI in an OS-native WebView (ADR-0001), standard web accessibility techniques apply directly — there is no custom-rendered canvas UI to special-case.

## 1. Screen reader support

- Every screen in the onboarding flow ([02-onboarding-flow.md](02-onboarding-flow.md)) is a semantic document: heading (`<h1>`) for the screen title, landmark regions, and a single, unambiguous primary-action button per screen with an accessible name matching its visible label exactly (no "click here" mismatches between visible and accessible text).
- The privacy dashboard's live recent-events feed ([03-privacy-dashboard.md](03-privacy-dashboard.md)) uses `aria-live="polite"` for new entries — informative but not interrupting, since this is ambient/reference information, not an alert.
- Recommendation notifications ([04-recommendations-ux.md](04-recommendations-ux.md)) use `aria-live="polite"` as well — a new recommendation is not an emergency and should not seize focus or interrupt a screen-reader user's current task, mirroring the sighted-user design intent ("non-intrusive notification").
- Destructive-action confirmations (delete-all) trap focus within the dialog and restore focus to the triggering control on cancel/close, with the dialog's consequence text (the key-deletion explanation) as the first thing announced.
- Data tables (the alternatives-comparison table in the recommendation detail view, the exclusion-rules list, the audit log) use proper `<table>` markup with header associations (`scope="col"`), not div-based visual tables — screen-reader users need the same row/column relationships sighted users get from the grid layout.

## 2. Keyboard navigation

- Full keyboard operability for every flow in this document: onboarding, dashboard, recommendations, settings. No mouse-only interaction (e.g., hover-to-reveal controls) gates any required action.
- Logical tab order matches visual/reading order in every wireframe above (top-to-bottom, left-to-right within a row).
- A global keyboard shortcut opens the privacy dashboard from anywhere in the app (configurable, with a sensible platform-appropriate default) — since "one click away" (per [03-privacy-dashboard.md](03-privacy-dashboard.md)) must also mean "one keystroke away" for a keyboard-only user.
- Standard dialog conventions: `Esc` closes/cancels non-destructive dialogs; destructive-action dialogs (delete-all) require an explicit affirmative keyboard action on the "Delete everything" control itself — `Enter` on a focused-by-default Cancel, never on Delete, so a stray keypress can't trigger data loss.

## 3. Visual accessibility

- High-contrast theme is a first-class theme, not a filter bolted onto the default theme — color choices for both light/dark/high-contrast modes are validated against WCAG AA contrast ratios (4.5:1 for body text, 3:1 for large text) as a build-time check, not a manual spot-check.
- All status/meaning conveyed by color (e.g., the `●` observation-active indicator, confidence dots in recommendation cards) is paired with text or shape, never color alone — colorblind users must be able to read "Observing" and "●●●●○ High confidence" without relying on hue.
- Font scaling: all UI text uses relative units (rem/em, not fixed px) so OS-level or in-app font-scaling requests resize the entire UI proportionally, including the ASCII-style dashboard layouts above (which are wireframe illustrations here, not literal monospace UI — the shipped UI uses a normal proportional-font layout with equivalent information density).
- Motion (e.g., a subtle animation on a new recommendation notification) respects `prefers-reduced-motion` and is disabled entirely when set.

## 4. Localization architecture

- All UI strings — onboarding copy, dashboard labels, recommendation-explanation templates, error messages — are externalized into resource files from the first implementation commit, even though only English ships initially (per PROMPT.md, treated as additive infrastructure, not a retrofit).
- Recommendation explanations are LLM-generated (ADR-0010) but structured (why/confidence/assumptions/alternatives as discrete fields) specifically so the *labels* around them can be localized independently of the *generated content* — a French-locale user sees "Pourquoi cette recommandation" as a localized label wrapping whatever language the configured LLM was prompted to respond in (the LLM prompt template itself should request the UI's configured locale for generated text, to avoid a jarring language mismatch between chrome and content).
- Date/time, number, and currency-adjacent formatting (time-saved estimates) use locale-aware formatting throughout, not hardcoded US conventions.
- Right-to-left layout is accounted for in the IA (§ of [05-settings-and-complexity-tiers.md](05-settings-and-complexity-tiers.md)) via logical CSS properties (`margin-inline-start` etc.) rather than physical left/right properties, so adding an RTL locale later doesn't require re-authoring layout code.

## 5. Testing bar for Phase 4

- Automated: axe-core (or equivalent) integrated into UI build CI, zero critical/serious violations as a merge gate.
- Manual: full onboarding-to-first-recommendation journey (Journey 1, [01-user-journeys.md](01-user-journeys.md)) completed using only a screen reader (VoiceOver/NVDA) and using only keyboard, on each target platform, before any release.
- Contrast: automated contrast-ratio check across both light/dark/high-contrast themes for every color pairing used in the wireframes above.
