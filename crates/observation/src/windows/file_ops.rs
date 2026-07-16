use std::path::Path;
use std::sync::mpsc::{channel, Receiver};

use hiddensteps_domain::{CapturedPayload, CapturedSignal, PrivacyLevel};
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};

use crate::{ObservationSource, PollError};

/// Watches a directory tree for file create/modify/delete operations via
/// `ReadDirectoryChangesW` (through the cross-platform `notify` crate — the
/// same dependency `linux::FileOperationSource` uses, just resolving to its
/// Windows backend here rather than inotify), reporting path + operation type
/// only — per `docs/design/05-privacy-model.md` §1's Level 2 file-operation-
/// metadata signal, never file *content*. There is no code path in this
/// source that reads a watched file's bytes at all.
///
/// Unlike `windows::ActiveWindowSource` and `windows::ClipboardMetadataSource`,
/// this source has no hand-written Win32 FFI of its own — `notify` owns the
/// platform-specific unsafe code — so it carries meaningfully lower risk of a
/// signature mismatch surfacing only once this compiles on a real Windows
/// machine. It is still unverified against a real NTFS volume in this
/// environment, same disclosure as everything else in this module.
pub struct FileOperationSource {
    // Held only to keep the underlying watch alive for this source's
    // lifetime; never read directly.
    _watcher: RecommendedWatcher,
    receiver: Receiver<notify::Result<Event>>,
}

impl FileOperationSource {
    pub fn watch(path: &Path) -> Result<Self, PollError> {
        let (tx, rx) = channel();
        let mut watcher = notify::recommended_watcher(move |res| {
            // The receiver may already be gone if the source was dropped; a
            // send failure here is not this closure's problem to handle.
            let _ = tx.send(res);
        })
        .map_err(|e| PollError::Backend(e.to_string()))?;
        watcher
            .watch(path, RecursiveMode::Recursive)
            .map_err(|e| PollError::Backend(e.to_string()))?;
        Ok(Self {
            _watcher: watcher,
            receiver: rx,
        })
    }
}

impl ObservationSource for FileOperationSource {
    fn id(&self) -> &str {
        "windows.file_operations"
    }

    fn min_privacy_level(&self) -> PrivacyLevel {
        PrivacyLevel::WorkflowMetadata
    }

    fn poll(&mut self) -> Result<Vec<CapturedSignal>, PollError> {
        let mut signals = Vec::new();
        while let Ok(result) = self.receiver.try_recv() {
            let event = match result {
                Ok(event) => event,
                // A watch-backend error (e.g. a watched path was removed out
                // from under us) is not fatal to the source as a whole — skip
                // it and keep draining whatever else is queued.
                Err(_) => continue,
            };
            let operation = match event.kind {
                EventKind::Create(_) => "create",
                EventKind::Modify(_) => "modify",
                EventKind::Remove(_) => "delete",
                // Access events (a file being merely read/opened) and
                // anything else `notify` reports are not "operations" in the
                // sense this signal type means to capture — deliberately
                // filtered, not forgotten.
                _ => continue,
            };
            for path in event.paths {
                signals.push(CapturedSignal::new(
                    self.id(),
                    CapturedPayload::FileOperation {
                        path: path.to_string_lossy().into_owned(),
                        operation: operation.to_string(),
                    },
                ));
            }
        }
        Ok(signals)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::thread::sleep;
    use std::time::Duration;

    // Same shape as `linux::file_ops`'s tests — real filesystem operations
    // against `notify`'s actual Windows backend, not a mock. `ReadDirectoryChangesW`
    // delivery is asynchronous relative to the syscall that triggers it, same as
    // inotify, hence the short generous sleep before polling.

    #[test]
    fn reports_a_real_file_creation() {
        let dir = tempfile::tempdir().unwrap();
        let mut source = FileOperationSource::watch(dir.path()).unwrap();

        let file_path = dir.path().join("report.xlsx");
        fs::write(&file_path, b"contents-never-inspected").unwrap();
        sleep(Duration::from_millis(300));

        let signals = source.poll().unwrap();
        assert!(signals.iter().any(|s| matches!(
            &s.payload,
            CapturedPayload::FileOperation { path, operation }
                if path.ends_with("report.xlsx") && operation == "create"
        )));
    }

    #[test]
    fn reports_a_real_file_deletion() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("scratch.tmp");
        fs::write(&file_path, b"x").unwrap();

        let mut source = FileOperationSource::watch(dir.path()).unwrap();
        fs::remove_file(&file_path).unwrap();
        sleep(Duration::from_millis(300));

        let signals = source.poll().unwrap();
        assert!(signals.iter().any(|s| matches!(
            &s.payload,
            CapturedPayload::FileOperation { path, operation }
                if path.ends_with("scratch.tmp") && operation == "delete"
        )));
    }

    #[test]
    fn never_reads_file_contents_into_a_signal() {
        // Structural check: FileOperation only ever carries `path` and
        // `operation` — there is no field it could put file bytes into even
        // if a future change tried to.
        let dir = tempfile::tempdir().unwrap();
        let mut source = FileOperationSource::watch(dir.path()).unwrap();
        fs::write(dir.path().join("secret.txt"), b"AKIAIOSFODNN7EXAMPLE").unwrap();
        sleep(Duration::from_millis(300));

        for signal in source.poll().unwrap() {
            match signal.payload {
                CapturedPayload::FileOperation { path, operation } => {
                    assert!(!path.contains("AKIAIOSFODNN7EXAMPLE"));
                    assert!(!operation.contains("AKIAIOSFODNN7EXAMPLE"));
                }
                other => panic!("expected FileOperation, got {other:?}"),
            }
        }
    }

    #[test]
    fn reports_correct_minimum_privacy_level() {
        let dir = tempfile::tempdir().unwrap();
        let source = FileOperationSource::watch(dir.path()).unwrap();
        assert_eq!(source.min_privacy_level(), PrivacyLevel::WorkflowMetadata);
    }
}
