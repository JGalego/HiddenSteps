// Pure functions only — no `chrome.*` calls, no network, no module-level
// state — so this file can be imported unmodified both by the extension's
// background service worker and by a plain Node test runner (see
// `../test/extract.test.mjs`). Keeping the actual URL-parsing logic here,
// separate from `background.js`'s event wiring, is what makes "never let
// the full tab URL leave this extraction step" something a test can check
// directly, rather than something that only shows up as a side effect deep
// inside an event listener.

/**
 * Mirrors `crates/domain/src/privacy.rs`'s `PrivacyLevel` numbering exactly
 * (`docs/design/05-privacy-model.md` §1): Level 2 ("Workflow metadata") is
 * where browser *domain* first appears; Level 3 ("Context-aware") adds page
 * *title*. There is no build-time link between this file and the Rust enum
 * it mirrors — same situation `SettingsPage.tsx`'s
 * `DEEP_MODE_SCREENSHOT_OCR_SETTING_KEY` constant already documents for its
 * own hand-kept-in-sync string constant.
 */
export const LEVEL_WORKFLOW_METADATA = 2;
export const LEVEL_CONTEXT_AWARE = 3;

/** Must match `hiddensteps_observation::BrowserBridgeSource::DEFAULT_PORT`. */
export const DEFAULT_PORT = 49231;

/**
 * Extracts *only* the hostname from a tab URL — never the scheme, path,
 * query string, fragment, or userinfo. Returns `null` (never the raw input)
 * for anything that isn't a reportable `http`/`https` page: browser-internal
 * pages (`chrome://`, `about:`, `edge://`), local files (`file://`), data
 * URIs, extension pages, and anything that fails to parse as a URL at all —
 * none of those have a meaningful "domain" to report, and guessing one from
 * a malformed string would risk leaking more than a hostname.
 *
 * This is the one function in this extension that ever touches a tab's full
 * URL string; every caller downstream only ever sees this function's return
 * value, never `tab.url` itself.
 *
 * @param {unknown} rawUrl
 * @returns {string | null}
 */
export function extractHostname(rawUrl) {
  if (typeof rawUrl !== "string" || rawUrl.length === 0) {
    return null;
  }
  let parsed;
  try {
    parsed = new URL(rawUrl);
  } catch {
    return null;
  }
  if (parsed.protocol !== "http:" && parsed.protocol !== "https:") {
    return null;
  }
  return parsed.hostname.length > 0 ? parsed.hostname : null;
}

/**
 * Returns the page title to report, or `null` when either the current
 * privacy level doesn't permit it (below Level 3 — domain-only at Level 2,
 * per `docs/design/05-privacy-model.md` §1) or there's no non-empty title to
 * send.
 *
 * @param {unknown} rawTitle
 * @param {number} currentLevel
 * @returns {string | null}
 */
export function extractTitle(rawTitle, currentLevel) {
  if (typeof currentLevel !== "number" || currentLevel < LEVEL_CONTEXT_AWARE) {
    return null;
  }
  if (typeof rawTitle !== "string") {
    return null;
  }
  const trimmed = rawTitle.trim();
  return trimmed.length > 0 ? trimmed : null;
}

/**
 * Debounces repeat reports for one tab: a report is only worth sending when
 * the hostname or (permitted) title actually changed since the last one this
 * tab sent — mirrors the "only a *change* is a new signal" discipline
 * `crates/observation/src/linux/clipboard.rs`'s `last_owner` field and
 * `crates/observation/src/browser_bridge.rs`'s `last_domain`/`last_title`
 * fields both already apply on the desktop side; this is the same rule
 * enforced one hop earlier, before a report is even sent over the wire.
 *
 * @param {{hostname: string, title: string | null} | null} previous
 * @param {{hostname: string, title: string | null}} next
 * @returns {boolean}
 */
export function shouldReport(previous, next) {
  if (!previous) {
    return true;
  }
  return previous.hostname !== next.hostname || previous.title !== next.title;
}

/**
 * Builds the exact JSON body `/v1/report` accepts
 * (`crates/observation/src/browser_bridge.rs`'s `ReportBody`): `domain`
 * always, `title` only when non-null, so a Level 2 report never even has a
 * `title` key for the desktop app's redaction/classification path to reason
 * about.
 *
 * @param {string} hostname
 * @param {string | null} title
 * @returns {{domain: string, title?: string}}
 */
export function buildReportPayload(hostname, title) {
  return title ? { domain: hostname, title } : { domain: hostname };
}
