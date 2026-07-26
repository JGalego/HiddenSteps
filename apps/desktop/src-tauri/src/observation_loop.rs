use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use hiddensteps_domain::PrivacyLevel;
use hiddensteps_event_store::SqlCipherEventStore;
use hiddensteps_observation::ObservationSource;
use hiddensteps_pipeline::{
    EventPipeline, NoTextExtraction, OcrsTextExtractor, PipelineOutcome, TextExtractor,
    DEFAULT_DEEP_MODE_TTL,
};
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
    let text_extractor = build_text_extractor(&app, &store).await;
    let pipeline = EventPipeline::with_text_extractor(text_extractor, Some(DEFAULT_DEEP_MODE_TTL));

    loop {
        let privacy_state = match store.get_privacy_state() {
            Ok(state) => state,
            Err(_) => break,
        };
        // requires_reconsent gates observation the same way observation_active
        // does: consented_manifest_version was persisted since v0.1.0, but
        // nothing ever compared it against the current build's manifest
        // version, so a future manifest bump (a release that changes what a
        // level captures) would never actually pause observation until the
        // user re-consents — it would just keep observing under the old,
        // superseded consent.
        let reconsent_required = hiddensteps_privacy_engine::requires_reconsent(
            privacy_state.consented_manifest_version,
            hiddensteps_privacy_engine::CURRENT_MANIFEST_VERSION,
        );
        if !privacy_state.observation_active
            || privacy_state.current_level == PrivacyLevel::Manual
            || reconsent_required
        {
            tokio::time::sleep(Duration::from_secs(2)).await;
            continue;
        }

        // Re-read every tick (not just once at startup): this is what makes
        // toggling either the privacy level or the Level-4 screenshot+OCR
        // sub-capability (`commands::DEEP_MODE_SCREENSHOT_OCR_SETTING_KEY`)
        // take effect on the very next poll, not just on the next app
        // restart.
        let deep_mode_screenshot_ocr_enabled = deep_mode_screenshot_ocr_enabled(&store);

        for source in &mut sources {
            // Defensive, belt-and-suspenders gate (the pipeline enforces this
            // too, in `minimum_level_for` — see its doc comment): never even
            // call `poll` on a source above the currently active privacy
            // level. Before this, every source's `poll` ran every tick
            // regardless of level, relying entirely on the pipeline to drop
            // the resulting signal afterward — harmless for a metadata-only
            // source, but wrong for a Level-4 source, since it means an
            // OS-level screenshot would actually be captured every tick even
            // at Level 0-3, just to be discarded a moment later.
            if source.min_privacy_level() > privacy_state.current_level {
                continue;
            }
            // A source's `min_privacy_level` being `MaximumAssistance` marks
            // it as Deep-mode content (screenshot/OCR/accessibility-tree,
            // per `docs/design/05-privacy-model.md` §1) — which needs its own
            // sub-capability opt-in on top of the coarse level-4 selection,
            // not just "the user is somewhere at or above Level 4."
            if source.min_privacy_level() == PrivacyLevel::MaximumAssistance
                && !deep_mode_screenshot_ocr_enabled
            {
                continue;
            }

            match source.poll() {
                Ok(signals) => {
                    for signal in signals {
                        match pipeline.process(
                            signal,
                            privacy_state.current_level,
                            OffsetDateTime::now_utc(),
                        ) {
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

fn deep_mode_screenshot_ocr_enabled(store: &SqlCipherEventStore) -> bool {
    matches!(
        store.get_setting(crate::commands::DEEP_MODE_SCREENSHOT_OCR_SETTING_KEY),
        Ok(Some(serde_json::Value::Bool(true)))
    )
}

/// Picks the pipeline's `TextExtractor` once, at loop startup: a real
/// `OcrsTextExtractor` if the user is already at Level 4 with screenshot+OCR
/// enabled *and* its model files can be provisioned, or `NoTextExtraction`
/// otherwise (in which case a Deep-mode screenshot signal drops with
/// `DropReason::OcrUnavailable`, per that type's own doc comment — never
/// silently stored unread).
///
/// Disclosed limitation: this is a one-time, startup-only decision, not a
/// live-reloading one, unlike the per-tick privacy-level/sub-capability gate
/// above. A user who enables Level 4 + screenshot/OCR mid-session (without
/// restarting the app) will have screenshots captured (the per-tick gate
/// allows that immediately) but dropped as `OcrUnavailable` until the next
/// restart re-runs this function. Making OCR-extractor selection itself fully
/// dynamic (lazy-init with retry/backoff on model-download failure) is a
/// reasonable follow-up, not attempted here to keep this change to what a
/// working screenshot+OCR producer needs.
async fn build_text_extractor(
    app: &AppHandle,
    store: &SqlCipherEventStore,
) -> Box<dyn TextExtractor> {
    let privacy_state = match store.get_privacy_state() {
        Ok(state) => state,
        Err(_) => return Box::new(NoTextExtraction),
    };
    if privacy_state.current_level != PrivacyLevel::MaximumAssistance
        || !deep_mode_screenshot_ocr_enabled(store)
    {
        return Box::new(NoTextExtraction);
    }

    let cache_dir = crate::data_dir()
        .parent()
        .map(|p| p.join("ocr-models"))
        .unwrap_or_else(|| PathBuf::from("ocr-models"));
    match crate::ocr_models::ensure_ocr_models(&cache_dir).await {
        Ok((detection_path, recognition_path)) => {
            match OcrsTextExtractor::from_model_files(&detection_path, &recognition_path) {
                Ok(extractor) => Box::new(extractor),
                Err(e) => {
                    report_source_error(app, "deep_mode.ocr_engine_init", e);
                    Box::new(NoTextExtraction)
                }
            }
        }
        Err(e) => {
            report_source_error(app, "deep_mode.ocr_model_download", e);
            Box::new(NoTextExtraction)
        }
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
    use hiddensteps_observation::ScreenshotSource;

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
    // Cross-platform (see `ScreenshotSource`'s doc comment) — always
    // constructed here regardless of the active privacy level, matching every
    // other source above; the run loop's per-tick gate is what actually keeps
    // it from being polled below Level 4 or without the screenshot+OCR
    // sub-capability turned on.
    sources.push(Box::new(ScreenshotSource::new()));

    sources
}

#[cfg(target_os = "windows")]
fn build_sources(app: &AppHandle) -> Vec<Box<dyn ObservationSource>> {
    use hiddensteps_observation::windows::{
        ActiveWindowSource, ClipboardMetadataSource, FileOperationSource,
    };
    use hiddensteps_observation::ScreenshotSource;

    let mut sources: Vec<Box<dyn ObservationSource>> = vec![
        Box::new(ActiveWindowSource::new()),
        Box::new(ClipboardMetadataSource::new()),
    ];

    for dir in watched_directories() {
        match FileOperationSource::watch(&dir) {
            Ok(source) => sources.push(Box::new(source)),
            Err(e) => report_source_error(app, "windows.file_operations", e),
        }
    }
    sources.push(Box::new(ScreenshotSource::new()));

    sources
}

#[cfg(target_os = "macos")]
fn build_sources(_app: &AppHandle) -> Vec<Box<dyn ObservationSource>> {
    use hiddensteps_observation::macos::ActiveWindowSource;
    use hiddensteps_observation::ScreenshotSource;

    vec![
        Box::new(ActiveWindowSource::new()),
        Box::new(ScreenshotSource::new()),
    ]
}

#[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
fn build_sources(_app: &AppHandle) -> Vec<Box<dyn ObservationSource>> {
    // No compiled `ObservationSource` backend exists for this target — see
    // `../../../crates/observation/src/lib.rs`'s doc comment for the three
    // that do.
    Vec::new()
}
