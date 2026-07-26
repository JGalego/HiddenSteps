# HiddenSteps Companion (browser extension)

This is the real producer behind `CapturedPayload::BrowserDomainVisited`
(Privacy Level 2) and `CapturedPayload::BrowserPageTitleViewed` (Level 3) in
`crates/domain/src/captured_signal.rs`. There is no OS-level API for "which
domain is the foreground browser tab showing," so this signal can only come
from an actual browser extension — see `crates/observation/src/lib.rs`'s and
`crates/observation/src/browser_bridge.rs`'s doc comments for the full
rationale and the security tradeoff behind the transport this extension talks
to.

## Scope: Chrome/Chromium only, Manifest V3

This extension targets Chrome and other Chromium-based browsers (Edge,
Brave, Opera, …) via Manifest V3. **Firefox is deliberately out of scope for
this pass**, not an oversight:

- Firefox's MV3 support (particularly background *service workers*, as
  opposed to Chrome's) is materially less mature/consistent as of this
  writing — Firefox still primarily supports MV3 with a persistent
  background page rather than the event-driven service worker this
  extension uses, and behavior around `chrome.alarms`/service-worker
  lifecycle differs enough that "the same `manifest.json` and `background.js`
  work unmodified on both" is not a safe assumption.
- There is no real Firefox (or Chrome, for that matter) browser available in
  this development sandbox to actually load and exercise either build against
  — only this extension's pure extraction logic (`src/extract.js`) is
  verified for real here, under Node (see "Testing" below). Shipping a
  second, *unverified* browser target in the same pass as the first would be
  writing-to-spec twice instead of once, which this repo's existing
  disclosed-gap convention (see `crates/observation/src/lib.rs` on macOS/
  Windows code paths) treats as worth calling out rather than doing quietly.

A Firefox build (`browser_specific_settings.gecko` block, a persistent
background page instead of a service worker, and real testing in an actual
Firefox instance) is a reasonable, separately-scoped follow-up.

## Architecture

- `manifest.json` — Manifest V3: a background service worker, `tabs` +
  `storage` + `alarms` permissions, and `host_permissions` scoped to
  `http://127.0.0.1:*/*` only (never a wildcard host) — this extension only
  ever talks to the local HiddenSteps desktop app, nothing else.
- `src/extract.js` — pure functions only (no `chrome.*` calls): hostname
  extraction via the `URL` API (never the raw `tab.url` string), title
  extraction gated by the currently active privacy level, tab-level
  debouncing, and the exact JSON shape `/v1/report` accepts. Kept separate
  from `background.js` specifically so it can be unit-tested under plain
  Node with no browser and no test framework dependency.
- `src/background.js` — the service worker: listens to
  `tabs.onUpdated`/`tabs.onActivated`, calls into `extract.js`, and `fetch`es
  the desktop app's bridge (`http://127.0.0.1:<port>/v1/report`), with a
  periodic `chrome.alarms`-driven refresh of `http://127.0.0.1:<port>/v1/status`
  to learn the currently active privacy level (the "tell the extension what
  level is active" handshake half of the design — see
  `crates/observation/src/browser_bridge.rs`'s doc comment for the other
  half, the transport-level enforcement that doesn't trust this extension to
  get it right).
- `src/options.html` / `src/options.js` — the pairing UI: paste the token
  shown in the HiddenSteps desktop app's Settings → Browser Extension panel,
  optionally adjust the port, and a "Test connection" button that hits
  `/v1/status` to confirm pairing worked.

## Setup (Chrome / Chromium)

1. Open `chrome://extensions`, enable "Developer mode" (top right).
2. Click "Load unpacked" and select this `apps/browser-extension` directory.
3. Open the extension's "Details" → "Extension options" (or the options page
   Chrome opens automatically on first install).
4. In the HiddenSteps desktop app, open Settings → "Browser Extension" and
   copy the pairing token shown there.
5. Paste the token into the options page, confirm the port matches (it
   defaults to `49231`, matching
   `BrowserBridgeSource::DEFAULT_PORT`), and click "Save", then
   "Test connection" to confirm.

Browser domain reporting takes effect once the desktop app's privacy level is
2 ("Workflow metadata") or higher; page-title reporting additionally requires
level 3 ("Context-aware") or higher — the extension checks this itself (via
`/v1/status`) before sending a title, and the desktop app's bridge enforces
the same rule again independently, so a stale or modified build of this
extension can't submit more than the active level allows.

## Testing

```sh
node --test test/extract.test.mjs
```

This runs real, for-real-executed unit tests (Node's built-in test runner —
no dependency to install) against `src/extract.js`'s hostname/title
extraction, debounce, and payload-building logic, including the URL-parsing
edge cases that matter most for this extension's core guarantee (never a
path, query string, fragment, userinfo, or non-http(s) scheme). What this
does **not** cover, and what remains written-to-spec rather than verified:
`background.js`'s `chrome.tabs.*`/`chrome.alarms`/`chrome.storage` event
wiring and `options.js`'s DOM wiring, both of which need a real browser
extension host to exercise — there is none in this sandbox. A manual
smoke test (load unpacked in a real Chrome install, pair with a running
desktop app, switch tabs, and confirm events show up in the desktop app's
Recent Events list) is the honest way to close that gap, matching this
repo's convention (see e.g. `crates/observation/src/lib.rs`'s disclosed
macOS/Windows gaps) of saying plainly what was verified for real versus
written to spec.
