use std::collections::VecDeque;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use hiddensteps_domain::{CapturedPayload, CapturedSignal, PrivacyLevel};
use serde::Deserialize;
use tiny_http::{Header, Method, Response, Server};

use crate::{ObservationSource, PollError};

/// The consumption side of the browser-extension bridge this crate's
/// `lib.rs` doc comment used to describe as unbuilt. `docs/design/05-privacy-
/// model.md` §1 requires Level 2's browser signal to be domain-only (never
/// path/query/content) and Level 3's addition to be page title only (still
/// never the full URL) — there is no OS API that exposes a foreground
/// browser tab's URL, so this signal can only ever come from an actual
/// browser extension (`apps/browser-extension`, Manifest V3,
/// Chrome/Chromium-based browsers) running as a separate process/context
/// from this one.
///
/// # Transport and its security tradeoff
///
/// `ObservationSource::poll` is deliberately synchronous and pull-based (see
/// that trait's doc comment), but a browser extension's tab-change reports
/// are push-based — it fires whenever the user changes tabs, not on this
/// process's schedule. This source bridges the two: it runs its own small
/// HTTP/1.1 server (via [`tiny_http`](https://crates.io/crates/tiny_http),
/// a synchronous, dependency-light server with no async-runtime requirement,
/// matching `poll`'s own contract) on a background thread, bound to
/// `127.0.0.1` **only** — never `0.0.0.0` — so nothing off-box can reach it
/// regardless of firewall configuration. `poll` itself just drains whatever
/// the server thread has buffered since the last call.
///
/// This was chosen over a native-messaging-host transport (the browser
/// spawning a short-lived executable per connection, talking length-prefixed
/// JSON over stdin/stdout, with a second local IPC hop — a Unix socket or
/// named pipe — to reach this long-running process) for a concrete, stated
/// tradeoff: native messaging anchors trust in the browser's own
/// allowlist-of-extension-IDs manifest rather than a shared secret, which is
/// the *stronger* security model of the two. But it requires registering a
/// native-messaging-host manifest in browser-specific OS locations — this
/// app has no installer-time registration step for anything today (see
/// `apps/desktop/src-tauri/main.rs` — first run only resolves a vault key
/// and an optional enterprise-policy file, no OS-level registration) — plus
/// a *second* transport (a socket/named pipe) between the ephemeral native-
/// messaging host process and this one, which is meaningfully more surface
/// to get right and verify in one sitting than a single local HTTP server.
/// Given that, this source's actual security boundary is **not** the
/// loopback binding or CORS (a webview/extension host bypasses CORS for
/// origins it holds `host_permissions` for, and any other local process
/// ignores CORS entirely) — it is the bearer token every request must
/// present, generated once per install (`apps/desktop/src-tauri`'s
/// `observation_loop::resolve_browser_bridge_token`) and compared in
/// constant time (`constant_time_eq` below). A future native-messaging
/// implementation remains a reasonable follow-up if the token-based model's
/// residual risk (any other local process that learns or brute-forces the
/// token could post fabricated domain/title signals) turns out to matter
/// more than the added registration/second-IPC-hop complexity.
///
/// # What this source enforces itself, on top of the pipeline
///
/// The Event Pipeline's `minimum_level_for` (per-signal-type, static) is the
/// authoritative gate — it drops a `BrowserPageTitleViewed` signal outright
/// if the active privacy level is below `ContextAware`, the same as it would
/// for any other source. This source additionally enforces the same rule at
/// the transport boundary, before a signal is ever queued: `/v1/report`
/// rejects a domain report outright below `WorkflowMetadata`, and silently
/// ignores a submitted title below `ContextAware`, matching the "never even
/// call `poll` below the level a source requires" discipline
/// `apps/desktop/src-tauri/src/observation_loop.rs`'s per-tick gate already
/// applies to every other source. `/v1/status` tells the extension which
/// level is currently active so a well-behaved extension never *submits* a
/// title it isn't allowed to send in the first place — this is the
/// handshake half of that discipline, enforced by the extension itself, with
/// the transport- and pipeline-level checks here as the defensive layers a
/// misbehaving or out-of-date extension build can't bypass.
pub struct BrowserBridgeSource {
    state: Arc<BridgeState>,
    server: Arc<Server>,
    worker: Option<JoinHandle<()>>,
}

struct BridgeState {
    queue: Mutex<VecDeque<CapturedSignal>>,
    token: String,
    current_level: Arc<AtomicU8>,
    /// Debounces repeat reports for the same tab/domain, mirroring
    /// `linux::clipboard::ClipboardMetadataSource`'s `last_owner` field — the
    /// same "only a *change* is a new signal" discipline, applied here
    /// because the extension itself firing on every `tabs.onUpdated` event is
    /// not, on its own, a guarantee against sending the same domain twice in
    /// a row (e.g. a page updating its own title without navigating).
    last_domain: Mutex<Option<String>>,
    last_title: Mutex<Option<String>>,
}

/// Upper bound on undrained signals this source holds in memory at once.
/// `poll` only drains what's here when the observation loop is actually
/// ticking at `WorkflowMetadata` or above (per `observation_loop.rs`'s
/// per-tick gate); while paused, below Level 2, or simply between ticks, the
/// HTTP server thread keeps accepting and queuing reports independently.
/// Without a cap, a long pause with the extension still reporting would grow
/// this queue unboundedly — the bounded-ring-buffer discipline
/// `docs/design/05-privacy-model.md` §2 requires of pre-summarize content
/// applied here at the observation layer, one call-site earlier than usual.
const MAX_BUFFERED_SIGNALS: usize = 256;

#[derive(Deserialize)]
struct ReportBody {
    domain: String,
    #[serde(default)]
    title: Option<String>,
}

impl BrowserBridgeSource {
    /// Matches the `observation_sources.id` column
    /// (`docs/design/07-database-schema.md`) and is also the `source_id` on
    /// every `CapturedSignal` this source produces, whether the payload is a
    /// domain or a title.
    pub const SOURCE_ID: &'static str = "browser_bridge.extension";

    /// Chosen from the dynamic/private port range (RFC 6335 §6:
    /// 49152-65535), which nothing else on a typical machine registers a
    /// fixed listener on, rather than a well-known or registered port that
    /// something else might already be bound to. Fixed (not re-randomized
    /// per launch) so the browser extension's options page can point at one
    /// unchanging address instead of needing to be re-paired after every
    /// desktop-app restart.
    pub const DEFAULT_PORT: u16 = 49231;

    /// Binds the bridge's local HTTP server and starts its background accept
    /// thread. `port` is normally [`Self::DEFAULT_PORT`]; this module's own
    /// tests pass `0` (an OS-chosen ephemeral port, read back via
    /// [`Self::port`]) so parallel test runs never collide on one fixed
    /// port. `current_level` is a cell shared with the caller (see
    /// `apps/desktop/src-tauri/src/observation_loop.rs`'s `run` loop, which
    /// updates it every tick from the persisted privacy state) — sharing the
    /// cell directly, rather than exposing a setter method here, means the
    /// caller doesn't need to downcast out of the type-erased
    /// `Box<dyn ObservationSource>` it stores this source as, just to keep
    /// the active level current.
    pub fn start(
        token: impl Into<String>,
        port: u16,
        current_level: Arc<AtomicU8>,
    ) -> Result<Self, PollError> {
        let server = Server::http(("127.0.0.1", port)).map_err(|e| {
            PollError::Backend(format!("failed to bind browser bridge server: {e}"))
        })?;
        let server = Arc::new(server);
        let state = Arc::new(BridgeState {
            queue: Mutex::new(VecDeque::new()),
            token: token.into(),
            current_level,
            last_domain: Mutex::new(None),
            last_title: Mutex::new(None),
        });

        let worker_server = Arc::clone(&server);
        let worker_state = Arc::clone(&state);
        let worker = std::thread::spawn(move || {
            for request in worker_server.incoming_requests() {
                handle_request(&worker_state, request);
            }
        });

        Ok(Self {
            state,
            server,
            worker: Some(worker),
        })
    }

    /// The actual bound port. Equal to whatever was passed to [`Self::start`]
    /// unless that was `0`.
    pub fn port(&self) -> u16 {
        match self.server.server_addr() {
            tiny_http::ListenAddr::IP(addr) => addr.port(),
            // tiny_http also supports Unix-socket listen addresses on some
            // platforms/feature combinations; this source never constructs
            // one (`start` always binds an IP loopback address), so this arm
            // should be unreachable in practice — 0 is a safe, honest
            // "no port" fallback rather than a panic.
            _ => 0,
        }
    }
}

impl Drop for BrowserBridgeSource {
    /// Unblocks the accept thread's `incoming_requests()` loop and joins it,
    /// so a source constructed and dropped repeatedly (observation toggled
    /// off and on, or just process shutdown) doesn't leak a thread or leave
    /// the port bound past this value's lifetime — the same cleanup
    /// discipline `linux::clipboard::ClipboardMetadataSource` already
    /// applies to its X11 window.
    fn drop(&mut self) {
        self.server.unblock();
        if let Some(handle) = self.worker.take() {
            let _ = handle.join();
        }
    }
}

impl ObservationSource for BrowserBridgeSource {
    fn id(&self) -> &str {
        Self::SOURCE_ID
    }

    /// The floor for this source as a whole: it may produce a
    /// `BrowserDomainVisited` signal (Level 2). Whether a given signal it
    /// actually queues is allowed at the *current* level is a stricter,
    /// per-signal check `/v1/report` and the pipeline's `minimum_level_for`
    /// both separately enforce (see this module's doc comment).
    fn min_privacy_level(&self) -> PrivacyLevel {
        PrivacyLevel::WorkflowMetadata
    }

    fn poll(&mut self) -> Result<Vec<CapturedSignal>, PollError> {
        let mut queue =
            self.state.queue.lock().map_err(|_| {
                PollError::Backend("browser bridge queue lock poisoned".to_string())
            })?;
        Ok(queue.drain(..).collect())
    }
}

fn handle_request(state: &Arc<BridgeState>, request: tiny_http::Request) {
    if !has_valid_token(&request, &state.token) {
        respond(request, 401, "unauthorized");
        return;
    }

    let method = request.method().clone();
    let url = request.url().to_string();
    match (method, url.as_str()) {
        (Method::Get, "/v1/status") => {
            let level = state.current_level.load(Ordering::Relaxed);
            respond_json(request, 200, &format!(r#"{{"privacy_level":{level}}}"#));
        }
        (Method::Post, "/v1/report") => handle_report(state, request),
        _ => respond(request, 404, "not found"),
    }
}

/// Constant-time comparison so a bearer-token check can't leak how many
/// leading bytes matched via response-timing side channel. Overkill for most
/// localhost-only tools, but cheap to get right, and this token is this
/// bridge's entire security boundary (see this module's doc comment) — worth
/// not cutting a corner on.
fn constant_time_eq(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.bytes()
        .zip(b.bytes())
        .fold(0u8, |acc, (x, y)| acc | (x ^ y))
        == 0
}

fn has_valid_token(request: &tiny_http::Request, expected: &str) -> bool {
    request
        .headers()
        .iter()
        .find(|h| h.field.equiv("authorization"))
        .map(|h| h.value.as_str())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|presented| constant_time_eq(presented, expected))
        .unwrap_or(false)
}

/// Rejects anything that isn't a bare hostname — no scheme, path, query
/// string, fragment, or embedded whitespace. Per
/// `docs/design/05-privacy-model.md` §1, the extension itself must never
/// send more than the hostname (see `apps/browser-extension/src/extract.js`),
/// but this check means a bug in a future extension build, or literally any
/// other client holding a valid token, can't smuggle a full URL through this
/// endpoint either — the guarantee holds at the transport boundary, not just
/// by convention on the sender's side.
fn is_bare_hostname(value: &str) -> bool {
    !value.is_empty()
        && !value.contains(['/', '?', '#', ' ', '\\'])
        && !value.contains("://")
        && !value.contains('@')
}

fn push_signal(state: &BridgeState, signal: CapturedSignal) {
    let mut queue = state.queue.lock().unwrap();
    if queue.len() >= MAX_BUFFERED_SIGNALS {
        queue.pop_front();
    }
    queue.push_back(signal);
}

fn handle_report(state: &Arc<BridgeState>, mut request: tiny_http::Request) {
    let mut body = String::new();
    if request.as_reader().read_to_string(&mut body).is_err() {
        respond(request, 400, "could not read request body");
        return;
    }

    let report: ReportBody = match serde_json::from_str(&body) {
        Ok(report) => report,
        Err(_) => {
            respond(request, 400, "malformed JSON body");
            return;
        }
    };

    if !is_bare_hostname(&report.domain) {
        respond(
            request,
            400,
            "domain must be a bare hostname, never a full URL",
        );
        return;
    }

    let current_level = PrivacyLevel::from_u8(state.current_level.load(Ordering::Relaxed))
        .unwrap_or(
            // A level value this bridge itself never wrote (corrupt shared
            // state) fails safe to Manual, i.e. "accept nothing."
            PrivacyLevel::Manual,
        );
    if current_level < PrivacyLevel::WorkflowMetadata {
        respond(
            request,
            403,
            "observation is below the privacy level browser activity requires",
        );
        return;
    }

    let mut queued = 0u32;
    {
        let mut last_domain = state.last_domain.lock().unwrap();
        if last_domain.as_deref() != Some(report.domain.as_str()) {
            push_signal(
                state,
                CapturedSignal::new(
                    BrowserBridgeSource::SOURCE_ID,
                    CapturedPayload::BrowserDomainVisited {
                        domain: report.domain.clone(),
                    },
                ),
            );
            *last_domain = Some(report.domain.clone());
            // A new domain makes whatever title was last reported stale, so
            // the next matching title report — even one whose text happens
            // to repeat verbatim from a previous, different page — is queued
            // again instead of being suppressed as a false-positive dupe.
            *state.last_title.lock().unwrap() = None;
            queued += 1;
        }
    }

    if current_level >= PrivacyLevel::ContextAware {
        if let Some(title) = report
            .title
            .as_deref()
            .map(str::trim)
            .filter(|t| !t.is_empty())
        {
            let mut last_title = state.last_title.lock().unwrap();
            if last_title.as_deref() != Some(title) {
                push_signal(
                    state,
                    CapturedSignal::new(
                        BrowserBridgeSource::SOURCE_ID,
                        CapturedPayload::BrowserPageTitleViewed {
                            title: title.to_string(),
                        },
                    ),
                );
                *last_title = Some(title.to_string());
                queued += 1;
            }
        }
    }

    respond_json(request, 202, &format!(r#"{{"queued":{queued}}}"#));
}

fn respond(request: tiny_http::Request, status: u16, message: &str) {
    let response = Response::from_string(message).with_status_code(status);
    let _ = request.respond(response);
}

fn respond_json(request: tiny_http::Request, status: u16, body: &str) {
    let mut response = Response::from_string(body).with_status_code(status);
    if let Ok(header) = Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]) {
        response = response.with_header(header);
    }
    let _ = request.respond(response);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::net::TcpStream;
    use std::time::Duration;

    fn start_test_source(level: PrivacyLevel) -> (BrowserBridgeSource, Arc<AtomicU8>, String) {
        let token = "test-token-0123456789".to_string();
        let level_cell = Arc::new(AtomicU8::new(level.as_u8()));
        let source = BrowserBridgeSource::start(token.clone(), 0, Arc::clone(&level_cell))
            .expect("bind an ephemeral loopback port");
        (source, level_cell, token)
    }

    /// Sends a raw HTTP/1.1 request over a real `TcpStream` to the bridge's
    /// bound port and returns `(status_code, body)`. Deliberately not using a
    /// higher-level HTTP client crate: this is a real round trip against
    /// `tiny_http`, not a mock, and the request is simple enough that hand-
    /// building it keeps this test module dependency-free.
    fn raw_request(
        port: u16,
        method: &str,
        path: &str,
        token: Option<&str>,
        body: &str,
    ) -> (u16, String) {
        let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect to bridge");
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let auth_header = match token {
            Some(t) => format!("Authorization: Bearer {t}\r\n"),
            None => String::new(),
        };
        let request = format!(
            "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1\r\n{auth_header}Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        stream.write_all(request.as_bytes()).unwrap();
        stream.flush().unwrap();

        let mut buf = Vec::new();
        let _ = std::io::Read::read_to_end(&mut stream, &mut buf);
        let response = String::from_utf8_lossy(&buf).into_owned();

        let status = response
            .lines()
            .next()
            .and_then(|line| line.split_whitespace().nth(1))
            .and_then(|code| code.parse().ok())
            .unwrap_or(0);
        let response_body = response
            .split_once("\r\n\r\n")
            .map(|(_, b)| b.to_string())
            .unwrap_or_default();
        (status, response_body)
    }

    #[test]
    fn reports_correct_minimum_privacy_level() {
        let (source, _level, _token) = start_test_source(PrivacyLevel::WorkflowMetadata);
        assert_eq!(source.min_privacy_level(), PrivacyLevel::WorkflowMetadata);
    }

    #[test]
    fn source_id_matches_the_documented_signal_source() {
        let (source, _level, _token) = start_test_source(PrivacyLevel::WorkflowMetadata);
        assert_eq!(source.id(), "browser_bridge.extension");
    }

    #[test]
    fn second_poll_with_no_new_activity_yields_nothing() {
        let (mut source, level, token) = start_test_source(PrivacyLevel::WorkflowMetadata);
        level.store(PrivacyLevel::WorkflowMetadata.as_u8(), Ordering::Relaxed);
        let port = source.port();
        let (status, _) = raw_request(
            port,
            "POST",
            "/v1/report",
            Some(&token),
            r#"{"domain":"example.com"}"#,
        );
        assert_eq!(status, 202);

        let first = source.poll().unwrap();
        assert_eq!(first.len(), 1);
        let second = source.poll().unwrap();
        assert!(second.is_empty());
    }

    #[test]
    fn domain_report_is_queued_as_browser_domain_visited() {
        let (mut source, _level, token) = start_test_source(PrivacyLevel::WorkflowMetadata);
        let port = source.port();
        let (status, _) = raw_request(
            port,
            "POST",
            "/v1/report",
            Some(&token),
            r#"{"domain":"example.com"}"#,
        );
        assert_eq!(status, 202);

        let signals = source.poll().unwrap();
        assert_eq!(signals.len(), 1);
        match &signals[0].payload {
            CapturedPayload::BrowserDomainVisited { domain } => assert_eq!(domain, "example.com"),
            other => panic!("expected BrowserDomainVisited, got {other:?}"),
        }
    }

    #[test]
    fn duplicate_domain_report_is_deduplicated() {
        let (mut source, _level, token) = start_test_source(PrivacyLevel::WorkflowMetadata);
        let port = source.port();
        for _ in 0..3 {
            let (status, _) = raw_request(
                port,
                "POST",
                "/v1/report",
                Some(&token),
                r#"{"domain":"example.com"}"#,
            );
            assert_eq!(status, 202);
        }

        let signals = source.poll().unwrap();
        assert_eq!(
            signals.len(),
            1,
            "three reports of the same domain should collapse into one signal"
        );
    }

    #[test]
    fn title_is_queued_at_context_aware_and_above() {
        let (mut source, _level, token) = start_test_source(PrivacyLevel::ContextAware);
        let port = source.port();
        let (status, _) = raw_request(
            port,
            "POST",
            "/v1/report",
            Some(&token),
            r#"{"domain":"example.com","title":"Example Domain"}"#,
        );
        assert_eq!(status, 202);

        let signals = source.poll().unwrap();
        assert_eq!(signals.len(), 2);
        assert!(signals
            .iter()
            .any(|s| matches!(&s.payload, CapturedPayload::BrowserDomainVisited { domain } if domain == "example.com")));
        assert!(signals
            .iter()
            .any(|s| matches!(&s.payload, CapturedPayload::BrowserPageTitleViewed { title } if title == "Example Domain")));
    }

    #[test]
    fn title_is_dropped_below_context_aware_even_when_submitted() {
        let (mut source, _level, token) = start_test_source(PrivacyLevel::WorkflowMetadata);
        let port = source.port();
        let (status, _) = raw_request(
            port,
            "POST",
            "/v1/report",
            Some(&token),
            r#"{"domain":"example.com","title":"Example Domain"}"#,
        );
        assert_eq!(status, 202);

        let signals = source.poll().unwrap();
        assert_eq!(
            signals.len(),
            1,
            "a title submitted below Level 3 must never be queued, even if the extension sends one"
        );
        assert!(matches!(
            &signals[0].payload,
            CapturedPayload::BrowserDomainVisited { .. }
        ));
    }

    #[test]
    fn domain_report_is_rejected_below_workflow_metadata() {
        let (mut source, _level, token) = start_test_source(PrivacyLevel::ApplicationMetadata);
        let port = source.port();
        let (status, _) = raw_request(
            port,
            "POST",
            "/v1/report",
            Some(&token),
            r#"{"domain":"example.com"}"#,
        );
        assert_eq!(status, 403);
        assert!(source.poll().unwrap().is_empty());
    }

    #[test]
    fn missing_token_is_rejected() {
        let (mut source, _level, _token) = start_test_source(PrivacyLevel::WorkflowMetadata);
        let port = source.port();
        let (status, _) = raw_request(
            port,
            "POST",
            "/v1/report",
            None,
            r#"{"domain":"example.com"}"#,
        );
        assert_eq!(status, 401);
        assert!(source.poll().unwrap().is_empty());
    }

    #[test]
    fn wrong_token_is_rejected() {
        let (mut source, _level, _token) = start_test_source(PrivacyLevel::WorkflowMetadata);
        let port = source.port();
        let (status, _) = raw_request(
            port,
            "POST",
            "/v1/report",
            Some("not-the-right-token"),
            r#"{"domain":"example.com"}"#,
        );
        assert_eq!(status, 401);
        assert!(source.poll().unwrap().is_empty());
    }

    #[test]
    fn a_full_url_instead_of_a_bare_hostname_is_rejected() {
        let (mut source, _level, token) = start_test_source(PrivacyLevel::WorkflowMetadata);
        let port = source.port();
        let (status, _) = raw_request(
            port,
            "POST",
            "/v1/report",
            Some(&token),
            r#"{"domain":"https://example.com/secret/path?token=abc"}"#,
        );
        assert_eq!(status, 400);
        assert!(source.poll().unwrap().is_empty());
    }

    #[test]
    fn malformed_json_body_is_rejected() {
        let (mut source, _level, token) = start_test_source(PrivacyLevel::WorkflowMetadata);
        let port = source.port();
        let (status, _) = raw_request(port, "POST", "/v1/report", Some(&token), "not json");
        assert_eq!(status, 400);
        assert!(source.poll().unwrap().is_empty());
    }

    #[test]
    fn status_endpoint_reports_the_current_privacy_level() {
        let (source, level, token) = start_test_source(PrivacyLevel::ContextAware);
        let port = source.port();
        level.store(PrivacyLevel::ContextAware.as_u8(), Ordering::Relaxed);
        let (status, body) = raw_request(port, "GET", "/v1/status", Some(&token), "");
        assert_eq!(status, 200);
        assert!(body.contains(&format!(
            "\"privacy_level\":{}",
            PrivacyLevel::ContextAware.as_u8()
        )));
    }

    #[test]
    fn status_endpoint_also_requires_the_token() {
        let (source, _level, _token) = start_test_source(PrivacyLevel::ContextAware);
        let port = source.port();
        let (status, _) = raw_request(port, "GET", "/v1/status", None, "");
        assert_eq!(status, 401);
    }

    #[test]
    fn unknown_route_is_a_404() {
        let (source, _level, token) = start_test_source(PrivacyLevel::WorkflowMetadata);
        let port = source.port();
        let (status, _) = raw_request(port, "GET", "/v1/does-not-exist", Some(&token), "");
        assert_eq!(status, 404);
    }
}
