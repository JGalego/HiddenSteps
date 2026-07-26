// Run with `node --test` (Node's built-in test runner — no dependency
// installation needed, matching this extension's zero-npm-dependency
// footprint). See `../README.md` for the exact command this repo's
// verification step uses.
import assert from "node:assert/strict";
import { test } from "node:test";

import {
  DEFAULT_PORT,
  LEVEL_CONTEXT_AWARE,
  LEVEL_WORKFLOW_METADATA,
  buildReportPayload,
  extractHostname,
  extractTitle,
  shouldReport,
} from "../src/extract.js";

test("extracts a bare hostname from a normal https URL", () => {
  assert.equal(extractHostname("https://example.com/path?query=1#frag"), "example.com");
});

test("extracts a hostname from an http URL with a port", () => {
  assert.equal(extractHostname("http://intranet.example.com:8080/dashboard"), "intranet.example.com");
});

test("never returns anything containing a path, query, or fragment", () => {
  const hostname = extractHostname("https://example.com/secret/path?token=abc123#section");
  assert.equal(hostname, "example.com");
  assert.doesNotMatch(hostname, /[/?#]/);
});

test("returns null for a malformed URL string", () => {
  assert.equal(extractHostname("not a url"), null);
});

test("returns null for a non-string input", () => {
  assert.equal(extractHostname(undefined), null);
  assert.equal(extractHostname(null), null);
  assert.equal(extractHostname(42), null);
});

test("returns null for an empty string", () => {
  assert.equal(extractHostname(""), null);
});

test("returns null for browser-internal schemes", () => {
  assert.equal(extractHostname("chrome://extensions"), null);
  assert.equal(extractHostname("about:blank"), null);
  assert.equal(extractHostname("edge://settings"), null);
});

test("returns null for a file:// URL", () => {
  assert.equal(extractHostname("file:///home/user/secret.txt"), null);
});

test("returns null for a data: URI", () => {
  assert.equal(extractHostname("data:text/plain;base64,c2VjcmV0"), null);
});

test("strips embedded userinfo, returning only the hostname", () => {
  const hostname = extractHostname("https://user:pass@example.com/");
  assert.equal(hostname, "example.com");
  assert.doesNotMatch(hostname, /user|pass/);
});

test("title is withheld below Level 3 (Context-aware)", () => {
  assert.equal(extractTitle("My Bank Account", LEVEL_WORKFLOW_METADATA), null);
  assert.equal(extractTitle("My Bank Account", 0), null);
  assert.equal(extractTitle("My Bank Account", 1), null);
});

test("title is reported at Level 3 and above", () => {
  assert.equal(extractTitle("My Bank Account", LEVEL_CONTEXT_AWARE), "My Bank Account");
  assert.equal(extractTitle("My Bank Account", 4), "My Bank Account");
});

test("title is trimmed and empty/whitespace-only titles become null", () => {
  assert.equal(extractTitle("  Padded Title  ", LEVEL_CONTEXT_AWARE), "Padded Title");
  assert.equal(extractTitle("   ", LEVEL_CONTEXT_AWARE), null);
  assert.equal(extractTitle("", LEVEL_CONTEXT_AWARE), null);
});

test("title is null for a non-string title even at a permitting level", () => {
  assert.equal(extractTitle(undefined, LEVEL_CONTEXT_AWARE), null);
  assert.equal(extractTitle(null, LEVEL_CONTEXT_AWARE), null);
});

test("shouldReport is true for the first report of a tab", () => {
  assert.equal(shouldReport(null, { hostname: "example.com", title: null }), true);
});

test("shouldReport is false when neither hostname nor title changed", () => {
  const previous = { hostname: "example.com", title: "Example" };
  const next = { hostname: "example.com", title: "Example" };
  assert.equal(shouldReport(previous, next), false);
});

test("shouldReport is true when the hostname changed", () => {
  const previous = { hostname: "example.com", title: null };
  const next = { hostname: "other.example.com", title: null };
  assert.equal(shouldReport(previous, next), true);
});

test("shouldReport is true when only the title changed (same domain)", () => {
  const previous = { hostname: "example.com", title: "Home" };
  const next = { hostname: "example.com", title: "About" };
  assert.equal(shouldReport(previous, next), true);
});

test("buildReportPayload omits the title key entirely when there is no title", () => {
  const payload = buildReportPayload("example.com", null);
  assert.deepEqual(payload, { domain: "example.com" });
  assert.equal("title" in payload, false);
});

test("buildReportPayload includes the title when present", () => {
  const payload = buildReportPayload("example.com", "Example Domain");
  assert.deepEqual(payload, { domain: "example.com", title: "Example Domain" });
});

test("DEFAULT_PORT matches the desktop bridge's documented default", () => {
  // Kept in sync by hand with
  // `crates/observation/src/browser_bridge.rs`'s `BrowserBridgeSource::DEFAULT_PORT`
  // — this test at least catches an accidental edit to one side without the
  // other, even though it can't catch both sides drifting together.
  assert.equal(DEFAULT_PORT, 49231);
});
