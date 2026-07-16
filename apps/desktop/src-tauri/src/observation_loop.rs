use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use hiddensteps_domain::PrivacyLevel;
use hiddensteps_event_store::SqlCipherEventStore;
use hiddensteps_observation::ObservationSource;
use hiddensteps_pipeline::{EventPipeline, PipelineOutcome};
use tauri::{AppHandle, Emitter};
use time::OffsetDateTime;

/// The real capture → pipeline → store → UI-event loop
/// (`docs/design/03-data-flow-diagrams.md` §1), polled on an interval rather than
/// driven by OS callbacks — matches the polling contract
/// `hiddensteps_observation::ObservationSource::poll` defines, and keeps this
/// loop's own logic trivial: ask each active source what's new, run it through
/// the pipeline, persist what survives, tell the UI.
///
/// Which sources exist is the only platform-specific part (`build_sources`
/// below); everything after that — the poll/pipeline/store/emit cycle — is one
/// shared implementation, so Linux, macOS, and Windows all get the same
/// capture → store → UI-event behavior once each has a real source wired in.
///
/// Global-shortcut capture is deliberately excluded from `build_sources` even
/// where it's implemented
/// (`hiddensteps_observation::{linux,windows}::GlobalShortcutSource`): grabbing
/// a key combo session-wide is something a user must opt into explicitly, not
/// something this loop starts by default — see
/// `../../../crates/observation/src/lib.rs`'s doc comment.
pub async fn run(app: AppHandle, store: Arc<SqlCipherEventStore>) {
    let mut sources = build_sources(&app);
    if sources.is_empty() {
        return;
    }
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

        for source in &mut sources {
            match source.poll() {
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
                    // A transient backend error (e.g. the X11/Win32 connection
                    // hiccuped) is not fatal to the loop — try again next tick.
                }
            }
        }

        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}

/// User-facing "places people keep files they're actively working with" —
/// deliberately narrower than the whole home directory, which on a developer's
/// machine can contain source trees, `node_modules`, and build output deep
/// enough to exhaust the OS's file-watch limits (`inotify` watch descriptors on
/// Linux, the equivalent internal buffer behind `ReadDirectoryChangesW` on
/// Windows) long before it captures anything meaningful about *workflow*. Any
/// of these that don't exist are silently skipped.
fn watched_directories() -> Vec<PathBuf> {
    #[cfg(target_os = "windows")]
    let home = std::env::var("USERPROFILE").ok().map(PathBuf::from);
    #[cfg(not(target_os = "windows"))]
    let home = std::env::var("HOME").ok().map(PathBuf::from);

    let Some(home) = home else {
        return Vec::new();
    };
    ["Desktop", "Documents", "Downloads"]
        .into_iter()
        .map(|dir| home.join(dir))
        .filter(|path| path.is_dir())
        .collect()
}

/// Reports a source that failed to construct via the same UI-visible channel
/// other observation errors use — a failure here is not fatal to the other
/// sources this platform builds, so the loop still starts with whatever did
/// construct successfully.
fn report_source_error(app: &AppHandle, source_id: &str, error: impl std::fmt::Display) {
    let _ = app.emit(
        "observation::source_error",
        format!("failed to start {source_id}: {error}"),
    );
}

#[cfg(target_os = "linux")]
fn build_sources(app: &AppHandle) -> Vec<Box<dyn ObservationSource>> {
    use hiddensteps_observation::linux::{
        ActiveWindowSource, ClipboardMetadataSource, FileOperationSource,
    };

    let mut sources: Vec<Box<dyn ObservationSource>> = Vec::new();

    match ActiveWindowSource::connect() {
        Ok(source) => sources.push(Box::new(source)),
        Err(e) => report_source_error(app, "linux.active_window", e),
    }
    match ClipboardMetadataSource::connect() {
        Ok(source) => sources.push(Box::new(source)),
        Err(e) => report_source_error(app, "linux.clipboard_metadata", e),
    }
    for dir in watched_directories() {
        match FileOperationSource::watch(&dir) {
            Ok(source) => sources.push(Box::new(source)),
            Err(e) => report_source_error(app, "linux.file_operations", e),
        }
    }

    sources
}

#[cfg(target_os = "windows")]
fn build_sources(app: &AppHandle) -> Vec<Box<dyn ObservationSource>> {
    use hiddensteps_observation::windows::{ActiveWindowSource, ClipboardMetadataSource, FileOperationSource};

    let mut sources: Vec<Box<dyn ObservationSource>> =
        vec![Box::new(ActiveWindowSource::new()), Box::new(ClipboardMetadataSource::new())];

    for dir in watched_directories() {
        match FileOperationSource::watch(&dir) {
            Ok(source) => sources.push(Box::new(source)),
            Err(e) => report_source_error(app, "windows.file_operations", e),
        }
    }

    sources
}

#[cfg(target_os = "macos")]
fn build_sources(_app: &AppHandle) -> Vec<Box<dyn ObservationSource>> {
    use hiddensteps_observation::macos::ActiveWindowSource;

    vec![Box::new(ActiveWindowSource::new())]
}

#[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
fn build_sources(_app: &AppHandle) -> Vec<Box<dyn ObservationSource>> {
    // No compiled `ObservationSource` backend exists for this target — see
    // `../../../crates/observation/src/lib.rs`'s doc comment for the three
    // that do.
    Vec::new()
}
