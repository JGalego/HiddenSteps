use std::sync::Arc;

use hiddensteps_event_store::SqlCipherEventStore;
use tauri::AppHandle;

// Everything below is used only inside the `#[cfg(target_os = "linux")]`
// branch of `run` — gating the imports too avoids "unused import" warnings on
// macOS/Windows, where that branch compiles out entirely (real warnings CI's
// first run surfaced, not hypothetical ones).
#[cfg(target_os = "linux")]
use std::time::Duration;
#[cfg(target_os = "linux")]
use hiddensteps_domain::PrivacyLevel;
#[cfg(target_os = "linux")]
use hiddensteps_pipeline::{EventPipeline, PipelineOutcome};
#[cfg(target_os = "linux")]
use tauri::Emitter;
#[cfg(target_os = "linux")]
use time::OffsetDateTime;

/// The real capture → pipeline → store → UI-event loop
/// (`docs/design/03-data-flow-diagrams.md` §1), polled on an interval rather than
/// driven by OS callbacks — matches the polling contract
/// `hiddensteps_observation::ObservationSource::poll` defines, and keeps this
/// loop's own logic trivial: ask each active source what's new, run it through
/// the pipeline, persist what survives, tell the UI.
///
/// Only wired for Linux here (`hiddensteps_observation::linux::ActiveWindowSource`,
/// the one real, tested-against-a-live-display source in that crate). Wiring the
/// macOS/Windows sources is the same shape once those modules are compiled and
/// verified on their respective platforms — see
/// `../../../crates/observation/src/lib.rs`'s doc comment.
pub async fn run(app: AppHandle, store: Arc<SqlCipherEventStore>) {
    #[cfg(target_os = "linux")]
    {
        let mut source = match hiddensteps_observation::linux::ActiveWindowSource::connect() {
            Ok(source) => source,
            Err(e) => {
                let _ = app.emit(
                    "security::key_or_vault_error",
                    format!("failed to start observation source: {e}"),
                );
                return;
            }
        };
        let pipeline = EventPipeline::new();

        loop {
            let privacy_state = match store.get_privacy_state() {
                Ok(state) => state,
                Err(_) => break,
            };
            if !privacy_state.observation_active || privacy_state.current_level == PrivacyLevel::Manual {
                tokio::time::sleep(Duration::from_secs(2)).await;
                continue;
            }

            match hiddensteps_observation::ObservationSource::poll(&mut source) {
                Ok(signals) => {
                    for signal in signals {
                        match pipeline.process(signal, privacy_state.current_level, OffsetDateTime::now_utc()) {
                            PipelineOutcome::Summarized(event) => {
                                if let Ok(id) = store.insert_event_summary(&event) {
                                    let mut with_id = event;
                                    with_id.id = Some(id);
                                    let _ = app.emit("observation::event_captured", &with_id);
                                }
                            }
                            PipelineOutcome::Dropped(_reason) => {
                                // Per ADR-0006: a dropped event is discarded, not
                                // logged with content — there is nothing to emit.
                            }
                        }
                    }
                }
                Err(_) => {
                    // A transient backend error (e.g. the X11 connection hiccuped)
                    // is not fatal to the loop — try again next tick.
                }
            }

            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    }

    #[cfg(not(target_os = "linux"))]
    {
        let _ = (app, store);
        // See this module's doc comment — macOS/Windows sources exist as source
        // in `hiddensteps-observation` but are unverified on this platform, so
        // this loop deliberately does nothing rather than call code that has
        // never been compiled.
    }
}
