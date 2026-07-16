# ADR-0005: Observation sources as capability-scoped plugins, one per platform/signal-type pair

Status: Accepted

## Context

PROMPT.md requires multiple observation modes (Minimal/Standard/Deep) drawing on many possible signal sources (active application, window changes, keyboard shortcuts, clipboard metadata, browser domains, filesystem activity, command history, accessibility APIs, optional screenshots/OCR/UI-tree), across three OS platforms whose native hooks for these signals are entirely different (macOS Accessibility API + NSWorkspace, Windows UI Automation + Win32 hooks, Linux AT-SPI/X11-or-Wayland-specific approaches). The threat model ([../../research/06-threat-model.md](../../research/06-threat-model.md)) flags the plugin surface as the highest-leverage place to get capability enforcement right.

## Decision

Each observation signal source is an independent plugin implementing the `ObservationSource` trait, declaring a manifest of exactly what it captures (signal type, e.g. `active_window_metadata`, `clipboard_metadata`, `browser_domain`, `screenshot_ocr`) and what OS permission tier that requires. In-tree, first-class sources cover the Minimal and Standard modes per platform (these are trusted, reviewed, part of the core install). Deep-mode sources (OCR, screenshot, UI-tree) are also in-tree but are structurally gated: they cannot activate unless (a) the user has explicitly opted into Level 4, and (b) the Privacy Engine has confirmed redaction is active for that session. Third-party observation plugins (e.g., a community-built integration for a specific niche application) go through the WASM plugin path (ADR-0009) with the same manifest-declared, runtime-enforced capability model.

## Consequences

- Adding Linux Wayland support (which lacks some of X11's global-hook capabilities) becomes a matter of shipping a Wayland-specific implementation of the same `ObservationSource` trait with a reduced capability set, surfaced honestly to the user via the diagnostics page — not a redesign.
- The manifest becomes the enforceable unit for the privacy dashboard's "what is currently being observed" display (PROMPT.md's Trust Features) — the UI can render exactly the declared capabilities of every active plugin, verified against runtime enforcement rather than trusting the plugin's own claims.
- OS permission requests are scoped to what the currently active privacy level actually needs (per ADR-0009's/the threat model's elevation-of-privilege mitigation): a user on Level 1 is never prompted for Accessibility/Screen-Recording permissions their level doesn't use.

## Alternatives considered

- **A single cross-platform observation abstraction with conditional-compiled internals**: rejected as the sole model — it's still needed at the trait level, but treating each signal source as its own plugin (rather than one monolithic "observer" per platform) keeps capability declarations granular enough for the privacy dashboard and permission-scoping to be meaningful per PROMPT.md's per-level transparency requirement.
