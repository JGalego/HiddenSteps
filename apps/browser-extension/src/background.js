// Manifest V3 background service worker. Listens for tab navigation/
// activation, extracts only the hostname (and, at Level 3+, the title) via
// `extract.js`'s pure functions, and posts that — nothing else — to the
// HiddenSteps desktop app's local bridge. `tab.url`/`tab.title` are read
// exactly once each, right here, and immediately narrowed through
// `extractHostname`/`extractTitle` before being held in any variable that
// outlives this function; nothing here ever logs a raw `tab` object or its
// `.url`.
import {
  DEFAULT_PORT,
  LEVEL_WORKFLOW_METADATA,
  buildReportPayload,
  extractHostname,
  extractTitle,
  shouldReport,
} from "./extract.js";

const STATUS_REFRESH_ALARM = "hiddensteps-status-refresh";

// In-memory only — cleared on every service-worker restart, which just means
// the next tab event for a given tab reports once more than strictly
// necessary. Correctness (never reporting more than the active privacy
// level allows) never depends on this surviving a restart; it only
// suppresses noise.
const lastReportedByTab = new Map();
let cachedLevel = 0;

async function getConfig() {
  const { token, port } = await chrome.storage.local.get(["token", "port"]);
  return { token: token || "", port: typeof port === "number" ? port : DEFAULT_PORT };
}

/**
 * Refreshes `cachedLevel` from the desktop app's `/v1/status` — the
 * handshake half of the level enforcement `browser_bridge.rs`'s doc comment
 * describes: this extension only ever *submits* a title when it already
 * believes the active level allows one, rather than relying solely on the
 * desktop app to reject an over-eager report after the fact (which it also
 * does — see that module's `/v1/report` handler — as the defensive layer a
 * stale or misbehaving build of this extension can't bypass).
 */
async function refreshLevel() {
  const { token, port } = await getConfig();
  if (!token) {
    return;
  }
  try {
    const response = await fetch(`http://127.0.0.1:${port}/v1/status`, {
      headers: { Authorization: `Bearer ${token}` },
    });
    if (!response.ok) {
      return;
    }
    const body = await response.json();
    if (typeof body.privacy_level === "number") {
      cachedLevel = body.privacy_level;
    }
  } catch {
    // The desktop app isn't running, hasn't started its bridge yet, or the
    // port/token is stale — leave `cachedLevel` as it was. The next report
    // attempt will fail the same way (see `reportTab`'s own catch) and
    // there's nothing more useful to do from a background alarm handler.
  }
}

async function reportTab(tab) {
  if (!tab || typeof tab.id !== "number") {
    return;
  }
  const hostname = extractHostname(tab.url);
  if (!hostname) {
    return;
  }
  if (cachedLevel < LEVEL_WORKFLOW_METADATA) {
    return;
  }

  const title = extractTitle(tab.title, cachedLevel);
  const next = { hostname, title };
  const previous = lastReportedByTab.get(tab.id) ?? null;
  if (!shouldReport(previous, next)) {
    return;
  }
  lastReportedByTab.set(tab.id, next);

  const { token, port } = await getConfig();
  if (!token) {
    return;
  }
  try {
    await fetch(`http://127.0.0.1:${port}/v1/report`, {
      method: "POST",
      headers: {
        Authorization: `Bearer ${token}`,
        "Content-Type": "application/json",
      },
      body: JSON.stringify(buildReportPayload(hostname, title)),
    });
  } catch {
    // Desktop app not running/reachable right now — drop this report. The
    // next tab-change event re-derives `next` from the tab's current state
    // and tries again; there's no queue to retry into here, matching this
    // extension's role as a thin, stateless-between-events reporter.
  }
}

chrome.tabs.onUpdated.addListener((_tabId, changeInfo, tab) => {
  // `status === "complete"` covers a navigation finishing; `changeInfo.title`
  // covers a same-document title change (e.g. a single-page app updating
  // `document.title` without a full navigation) — both are moments the
  // active tab's reportable state may have changed.
  if (changeInfo.status === "complete" || typeof changeInfo.title === "string") {
    reportTab(tab);
  }
});

chrome.tabs.onActivated.addListener(({ tabId }) => {
  chrome.tabs.get(tabId).then(reportTab).catch(() => {
    // The tab may have closed between the activation event firing and this
    // lookup running — nothing to report.
  });
});

chrome.tabs.onRemoved.addListener((tabId) => {
  lastReportedByTab.delete(tabId);
});

chrome.alarms.onAlarm.addListener((alarm) => {
  if (alarm.name === STATUS_REFRESH_ALARM) {
    refreshLevel();
  }
});

function scheduleStatusRefresh() {
  chrome.alarms.create(STATUS_REFRESH_ALARM, { periodInMinutes: 1 });
  refreshLevel();
}

chrome.runtime.onStartup.addListener(scheduleStatusRefresh);
chrome.runtime.onInstalled.addListener(scheduleStatusRefresh);
