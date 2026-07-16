use hiddensteps_domain::{CapturedSignal, PrivacyLevel};

#[derive(Debug, thiserror::Error)]
pub enum PollError {
    #[error("observation source backend error: {0}")]
    Backend(String),
}

/// One capability-scoped observation source (ADR-0005): a single platform/signal
/// -type pair, e.g. "the Linux active-window watcher" or "the Linux file-operation
/// watcher." A running app holds one instance per active source and calls `poll`
/// on a schedule; each call drains whatever new signals have accumulated since the
/// last call.
///
/// Polling (rather than a push/callback API) is deliberate: it keeps this trait
/// free of async-runtime or threading assumptions, so the Event Pipeline that
/// consumes `poll`'s output doesn't need to know anything about how a given
/// source gathers its signals internally (a background OS hook thread, an inotify
/// watch, a timer) — that's each implementation's own concern.
pub trait ObservationSource: Send {
    /// Stable identifier, e.g. `"linux.active_window"` — matches the
    /// `observation_sources.id` column in `docs/design/07-database-schema.md`.
    fn id(&self) -> &str;

    /// The lowest privacy level at which this source may run at all
    /// (`docs/design/08-plugin-architecture.md` §2's `min_privacy_level`). A host
    /// must not call `poll` on a source while the active privacy level is below
    /// this — enforced by the host, not by this trait, since the source itself
    /// has no way to know the current level.
    fn min_privacy_level(&self) -> PrivacyLevel;

    /// Drains and returns whatever signals have accumulated since the last call.
    /// Returns an empty vec, not an error, when there's simply nothing new.
    fn poll(&mut self) -> Result<Vec<CapturedSignal>, PollError>;
}
