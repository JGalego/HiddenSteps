use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread::JoinHandle;

use hiddensteps_domain::{CapturedPayload, CapturedSignal, PrivacyLevel};
use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    RegisterHotKey, UnregisterHotKey, HOT_KEY_MODIFIERS,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetMessageW, PostThreadMessageW, MSG, WM_HOTKEY, WM_QUIT,
};

use crate::{ObservationSource, PollError};

/// One shortcut to watch for: a Win32 modifier mask (`MOD_ALT`, `MOD_CONTROL`,
/// ... OR'd together, from `HOT_KEY_MODIFIERS`), a virtual-key code, and the
/// human-readable label to report when it fires. Mirrors
/// `linux::ShortcutBinding`'s "work in raw codes, not names" choice for the
/// same reason: resolving a name like `"Ctrl+Shift+4"` to a virtual-key code
/// is real keyboard-layout-aware work that's a legitimate follow-up, not
/// something worth faking with a hardcoded lookup table here.
pub struct ShortcutBinding {
    pub modifiers: u32,
    pub vk: u32,
    pub label: String,
}

/// Watches for specific global keyboard shortcuts via `RegisterHotKey`.
///
/// **This is invasive by construction**, same as `linux::GlobalShortcutSource`:
/// `RegisterHotKey` intercepts the given key combo session-wide, for every
/// application, until explicitly unregistered. Per the caveats in this crate's
/// `lib.rs` doc comment, nothing in HiddenSteps auto-starts this source with a
/// default binding list — a caller must explicitly choose which combos to
/// register, same as a user would explicitly configure which shortcuts they
/// want tracked.
///
/// `RegisterHotKey`/`WM_HOTKEY` delivery is thread-affine: only the thread
/// that registered a hotkey can retrieve its `WM_HOTKEY` message from that
/// thread's own message queue. Because `ObservationSource::poll` may be
/// called from whatever thread an async runtime happens to resume its task
/// on, this source runs its own dedicated OS thread to register the bindings
/// and run the `GetMessageW` loop, handing fired shortcuts back to `poll`
/// over a channel — the same worker-thread-plus-channel shape
/// `windows::FileOperationSource` uses for its own, different, backend-
/// imposed reason.
pub struct GlobalShortcutSource {
    receiver: Receiver<String>,
    worker: Option<JoinHandle<()>>,
    worker_thread_id: Option<u32>,
}

impl GlobalShortcutSource {
    pub fn register(bindings: Vec<ShortcutBinding>) -> Result<Self, PollError> {
        let (fired_tx, fired_rx) = channel();
        let (ready_tx, ready_rx) = channel();

        let worker = std::thread::spawn(move || Self::run_worker(bindings, fired_tx, ready_tx));

        match ready_rx.recv() {
            Ok(Ok(thread_id)) => Ok(Self {
                receiver: fired_rx,
                worker: Some(worker),
                worker_thread_id: Some(thread_id),
            }),
            Ok(Err(e)) => {
                let _ = worker.join();
                Err(PollError::Backend(e))
            }
            Err(_) => {
                let _ = worker.join();
                Err(PollError::Backend(
                    "shortcut worker thread exited before registering any binding".into(),
                ))
            }
        }
    }

    /// Runs entirely on its own thread: registers every binding against
    /// *this* thread's message queue, reports back success (with this
    /// thread's id, needed for `Drop` to post it a shutdown message) or the
    /// first registration failure, then blocks in `GetMessageW` until told to
    /// stop.
    fn run_worker(
        bindings: Vec<ShortcutBinding>,
        fired_tx: Sender<String>,
        ready_tx: Sender<Result<u32, String>>,
    ) {
        // SAFETY: takes no arguments; reads the calling thread's own id.
        let thread_id = unsafe { GetCurrentThreadId() };

        let mut registered_ids = Vec::new();
        for (index, binding) in bindings.iter().enumerate() {
            let id = index as i32;
            // SAFETY: `None` (no window) registers the hotkey against this
            // thread's message queue instead of a specific window's, which is
            // exactly the delivery mechanism this worker's `GetMessageW` loop
            // below expects; `id` is unique per binding within this call.
            let result = unsafe {
                RegisterHotKey(None, id, HOT_KEY_MODIFIERS(binding.modifiers), binding.vk)
            };
            if let Err(e) = result {
                for registered_id in &registered_ids {
                    // SAFETY: each id here was successfully registered above
                    // and has not yet been unregistered.
                    let _ = unsafe { UnregisterHotKey(None, *registered_id) };
                }
                let _ = ready_tx.send(Err(e.to_string()));
                return;
            }
            registered_ids.push(id);
        }

        if ready_tx.send(Ok(thread_id)).is_err() {
            // The constructor gave up waiting (its own receiver dropped) —
            // nothing left to hand fired shortcuts to, so tear down and exit
            // rather than run an unobserved message loop forever.
            for registered_id in &registered_ids {
                let _ = unsafe { UnregisterHotKey(None, *registered_id) };
            }
            return;
        }

        let mut msg = MSG::default();
        loop {
            // SAFETY: `None` retrieves messages posted to this thread (rather
            // than to a specific window), which is where `RegisterHotKey`
            // above delivers `WM_HOTKEY` and where `Drop` posts `WM_QUIT` to
            // end this loop; `msg` is a valid out-pointer for the duration of
            // the call. The return value is checked below rather than via
            // `.as_bool()`, since `GetMessageW` can return -1 on error, which
            // `.as_bool()` would treat as "got a message" — `<= 0` treats
            // both `WM_QUIT` (0) and an error (-1) as "stop".
            let got_message = unsafe { GetMessageW(&mut msg, None, 0, 0) };
            if got_message.0 <= 0 {
                break;
            }
            if msg.message == WM_HOTKEY {
                let index = msg.wParam.0 as usize;
                if let Some(binding) = bindings.get(index) {
                    let _ = fired_tx.send(binding.label.clone());
                }
            }
        }

        for registered_id in &registered_ids {
            // SAFETY: each id here was successfully registered above.
            let _ = unsafe { UnregisterHotKey(None, *registered_id) };
        }
    }
}

impl Drop for GlobalShortcutSource {
    fn drop(&mut self) {
        if let Some(thread_id) = self.worker_thread_id {
            // SAFETY: posts the well-known, argument-less `WM_QUIT` message
            // to the worker's own thread queue to unblock its `GetMessageW`
            // call; `thread_id` was returned by that same thread at startup,
            // and posting to a thread that has already exited is documented
            // to fail harmlessly rather than being unsafe.
            let _ = unsafe { PostThreadMessageW(thread_id, WM_QUIT, WPARAM(0), LPARAM(0)) };
        }
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl ObservationSource for GlobalShortcutSource {
    fn id(&self) -> &str {
        "windows.global_shortcuts"
    }

    fn min_privacy_level(&self) -> PrivacyLevel {
        PrivacyLevel::ApplicationMetadata
    }

    fn poll(&mut self) -> Result<Vec<CapturedSignal>, PollError> {
        let mut signals = Vec::new();
        while let Ok(label) = self.receiver.try_recv() {
            signals.push(CapturedSignal::new(
                self.id(),
                CapturedPayload::ShortcutInvoked { shortcut: label },
            ));
        }
        Ok(signals)
    }
}

// No #[cfg(test)] module here — see this module's parent doc comment for why:
// without a Windows machine to run them on, a test that actually registers a
// real global hotkey would assert behavior of code nobody has executed, and
// would carry the same shared-session interference risk
// `linux::shortcuts`'s `#[ignore]`d test discloses for `XGrabKey`.
