/// A raw, pre-redaction signal captured by an `ObservationSource`.
///
/// Per ADR-0006 (`docs/design/adr/0006-capture-classify-redact-summarize-pipeline.md`),
/// raw capture must never reach durable, unencrypted, or long-lived storage. This type
/// enforces that as a structural property rather than a policy someone could forget:
///
/// - It deliberately does **not** derive `serde::Serialize` — there is no way to turn a
///   `CapturedSignal` into bytes for storage or transmission without writing new,
///   clearly-out-of-place code to do so.
/// - `EventStore` (in the `hiddensteps-event-store` crate) has no API that accepts this
///   type; only the post-redaction, post-summarize `EventSummary` can be persisted.
/// - It carries no `Clone`/`Copy` either, so a reference to raw content can't be
///   casually stashed somewhere and outlive the pipeline stage that produced it.
#[derive(Debug)]
pub struct CapturedSignal {
    pub source_id: String,
    pub payload: CapturedPayload,
}

/// The raw payload shape varies by signal type; this is intentionally a small,
/// closed set mirroring the manifest capability enumeration in
/// `docs/design/08-plugin-architecture.md` §2 — an `ObservationSource` cannot invent
/// a new payload shape the pipeline doesn't already know how to classify/redact.
#[derive(Debug)]
pub enum CapturedPayload {
    AppFocusChange {
        app_identifier: String,
    },
    WindowTitle {
        title: String,
    },
    ShortcutInvoked {
        shortcut: String,
    },
    BrowserDomainVisited {
        domain: String,
    },
    /// Level 3 ("Context-aware")'s addition to Level 2's `BrowserDomainVisited`,
    /// per `docs/design/05-privacy-model.md` §1 — still never the full URL, path,
    /// query string, or fragment, only the page's title text. Modeled as its own
    /// sibling variant (mirroring `AppFocusChange`/`WindowTitle` above: two
    /// related-but-distinct signals from the same underlying source, not one
    /// struct with an optional field) so `minimum_level_for` in
    /// `hiddensteps-pipeline` can keep mapping each variant to exactly one
    /// static privacy level — a page title requires Level 3, a domain alone
    /// only Level 2, and an `ObservationSource` may legitimately produce one
    /// without the other.
    BrowserPageTitleViewed {
        title: String,
    },
    ClipboardMetadata {
        content_type: String,
        size_bytes: usize,
    },
    FileOperation {
        path: String,
        operation: String,
    },
    OcrText {
        text: String,
    },
    Screenshot {
        raw_bytes: Vec<u8>,
    },
    AccessibilityTree {
        serialized: String,
    },
}

impl CapturedSignal {
    pub fn new(source_id: impl Into<String>, payload: CapturedPayload) -> Self {
        Self {
            source_id: source_id.into(),
            payload,
        }
    }
}
