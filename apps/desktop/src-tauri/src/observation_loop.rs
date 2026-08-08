use std::path::PathBuf;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use std::time::Duration;

use hiddensteps_domain::PrivacyLevel;
use hiddensteps_event_store::SqlCipherEventStore;
use hiddensteps_observation::{BrowserBridgeSource, ObservationSource};
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
///
/// `browser_bridge_token` is resolved once, synchronously, in `main.rs` (see
/// `resolve_browser_bridge_token`) — before this async loop even starts, so
/// `commands::get_browser_bridge_status` never races this loop for "does a
/// token exist yet."
pub async fn run(app: AppHandle, store: Arc<SqlCipherEventStore>, browser_bridge_token: String) {
    // Shared with every `BrowserBridgeSource` `build_sources` constructs
    // (there's at most one, but the mechanism doesn't assume that) so this
    // loop can keep the bridge's notion of "the currently active privacy
    // level" current every tick, without needing to downcast out of the
    // type-erased `Box<dyn ObservationSource>` values in `sources` below to
    // reach a bridge-specific setter.
    let bridge_level = Arc::new(AtomicU8::new(PrivacyLevel::Manual.as_u8()));
    let mut sources = build_sources(&app, &browser_bridge_token, &bridge_level);
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
        let observation_gated_off = !privacy_state.observation_active
            || privacy_state.current_level == PrivacyLevel::Manual
            || reconsent_required;
        // Tell the bridge's `/v1/status`/`/v1/report` endpoints the level
        // that's actually in effect this tick — `Manual` (rejecting every
        // report) whenever observation as a whole is gated off, not just
        // `privacy_state.current_level` verbatim, so a paused app or one
        // pending re-consent stops accepting browser-activity reports the
        // same tick it stops polling every other source, not just once the
        // level itself is lowered.
        let effective_bridge_level = if observation_gated_off {
            PrivacyLevel::Manual
        } else {
            privacy_state.current_level
        };
        bridge_level.store(effective_bridge_level.as_u8(), Ordering::Relaxed);
        if observation_gated_off {
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

/// Reads the persisted browser-extension pairing token
/// (`BrowserBridgeSource`'s bearer-token check — see that type's doc comment
/// for why this token, not the loopback binding or CORS, is the bridge's
/// actual security boundary), generating and persisting a fresh one on first
/// run. Called synchronously from `main.rs`, before this module's async `run`
/// loop starts, so `commands::get_browser_bridge_status` never has to handle
/// "the token doesn't exist yet" as a distinct state.
///
/// A 256-bit random token, hex-encoded — reusing
/// `hiddensteps_security::generate_master_key`'s CSPRNG rather than pulling
/// in a separate randomness dependency for one string; a pairing token has
/// materially lower stakes than the database master key that function was
/// written for, but there's no reason to reach for weaker randomness when a
/// cryptographically strong generator is already a dependency.
pub(crate) fn resolve_browser_bridge_token(store: &SqlCipherEventStore) -> String {
    if let Ok(Some(serde_json::Value::String(existing))) =
        store.get_setting(crate::commands::BROWSER_BRIDGE_TOKEN_SETTING_KEY)
    {
        if !existing.is_empty() {
            return existing;
        }
    }
    let key = hiddensteps_security::generate_master_key();
    let token: String = key.iter().map(|b| format!("{b:02x}")).collect();
    let _ = store.set_setting(
        crate::commands::BROWSER_BRIDGE_TOKEN_SETTING_KEY,
        &serde_json::Value::String(token.clone()),
    );
    token
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

/// Constructs the browser-extension bridge (cross-platform — see
/// `BrowserBridgeSource`'s doc comment) and pushes it into `sources` on
/// success, exactly like `ScreenshotSource` is added by every platform's
/// `build_sources` below. Factored out once rather than duplicated four
/// times, since — unlike `ScreenshotSource::new()`, which is infallible —
/// this construction takes the token/level-cell parameters `build_sources`
/// itself was handed and can fail (the fixed port already in use), which
/// needs the same `report_source_error` handling every fallible source here
/// already gets.
fn push_browser_bridge_source(
    app: &AppHandle,
    sources: &mut Vec<Box<dyn ObservationSource>>,
    bridge_token: &str,
    bridge_level: &Arc<AtomicU8>,
) {
    match BrowserBridgeSource::start(
        bridge_token.to_string(),
        BrowserBridgeSource::DEFAULT_PORT,
        Arc::clone(bridge_level),
    ) {
        Ok(source) => sources.push(Box::new(source)),
        Err(e) => report_source_error(app, BrowserBridgeSource::SOURCE_ID, e),
    }
}

#[cfg(target_os = "linux")]
fn build_sources(
    app: &AppHandle,
    bridge_token: &str,
    bridge_level: &Arc<AtomicU8>,
) -> Vec<Box<dyn ObservationSource>> {
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
    // Also cross-platform (see `BrowserBridgeSource`'s doc comment) — its own
    // per-tick level updates (`bridge_level`, refreshed every tick in `run`)
    // are what make its `/v1/report` endpoint stop accepting domain/title
    // reports below the level each requires, the same belt-and-suspenders
    // discipline `ScreenshotSource` relies on the run loop's gate for.
    push_browser_bridge_source(app, &mut sources, bridge_token, bridge_level);

    sources
}

#[cfg(target_os = "windows")]
fn build_sources(
    app: &AppHandle,
    bridge_token: &str,
    bridge_level: &Arc<AtomicU8>,
) -> Vec<Box<dyn ObservationSource>> {
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
    push_browser_bridge_source(app, &mut sources, bridge_token, bridge_level);

    sources
}

#[cfg(target_os = "macos")]
fn build_sources(
    app: &AppHandle,
    bridge_token: &str,
    bridge_level: &Arc<AtomicU8>,
) -> Vec<Box<dyn ObservationSource>> {
    use hiddensteps_observation::macos::ActiveWindowSource;
    use hiddensteps_observation::ScreenshotSource;

    let mut sources: Vec<Box<dyn ObservationSource>> = vec![
        Box::new(ActiveWindowSource::new()),
        Box::new(ScreenshotSource::new()),
    ];
    push_browser_bridge_source(app, &mut sources, bridge_token, bridge_level);

    sources
}

#[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
fn build_sources(
    _app: &AppHandle,
    _bridge_token: &str,
    _bridge_level: &Arc<AtomicU8>,
) -> Vec<Box<dyn ObservationSource>> {
    // No compiled `ObservationSource` backend exists for this target — see
    // `../../../crates/observation/src/lib.rs`'s doc comment for the three
    // that do.
    Vec::new()
}
