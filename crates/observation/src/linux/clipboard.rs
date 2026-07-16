use std::time::{Duration, Instant};

use hiddensteps_domain::{CapturedPayload, CapturedSignal, PrivacyLevel};
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{AtomEnum, ConnectionExt, CreateWindowAux, EventMask, WindowClass};
use x11rb::protocol::Event;
use x11rb::rust_connection::RustConnection;

use crate::{ObservationSource, PollError};

/// Reports clipboard *metadata* only — content type and byte size — never
/// clipboard content, per `docs/design/05-privacy-model.md` §1's Level 2 signal
/// definition.
///
/// X11's selection protocol has no "how big is the selection, without sending it"
/// query — the only way to learn a selection's size is to actually request it
/// (`ConvertSelection`) and read the resulting property's length. This source does
/// exactly that, in-process, and then discards the received buffer immediately:
/// the byte length is measured and kept; the bytes themselves are dropped at the
/// end of the function that received them and never touch a `CapturedPayload`,
/// a log, or storage. This is the one Linux source in this crate where "briefly
/// holds bytes in memory to derive a number, then discards them" is unavoidable
/// given the underlying protocol — the same discipline the Event Pipeline applies
/// to Deep-mode captures (ADR-0006) is applied here at the observation layer
/// itself, before a `CapturedPayload` is even constructed.
pub struct ClipboardMetadataSource {
    conn: RustConnection,
    requestor: u32,
    clipboard: u32,
    targets: u32,
    incoming: u32,
    last_owner: Option<u32>,
}

impl ClipboardMetadataSource {
    pub fn connect() -> Result<Self, PollError> {
        let (conn, screen_num) =
            x11rb::connect(None).map_err(|e| PollError::Backend(e.to_string()))?;
        let root = conn.setup().roots[screen_num].root;

        let requestor = conn
            .generate_id()
            .map_err(|e| PollError::Backend(e.to_string()))?;
        conn.create_window(
            0,
            requestor,
            root,
            0,
            0,
            1,
            1,
            0,
            WindowClass::INPUT_OUTPUT,
            0,
            &CreateWindowAux::new().event_mask(EventMask::PROPERTY_CHANGE),
        )
        .map_err(|e| PollError::Backend(e.to_string()))?
        .check()
        .map_err(|e| PollError::Backend(e.to_string()))?;

        let clipboard = intern_atom(&conn, b"CLIPBOARD")?;
        let targets = intern_atom(&conn, b"TARGETS")?;
        let incoming = intern_atom(&conn, b"HIDDENSTEPS_CLIPBOARD_PROBE")?;

        Ok(Self {
            conn,
            requestor,
            clipboard,
            targets,
            incoming,
            last_owner: None,
        })
    }

    fn wait_for_selection_notify(&self, timeout: Duration) -> Result<Option<u32>, PollError> {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            let event = self
                .conn
                .poll_for_event()
                .map_err(|e| PollError::Backend(e.to_string()))?;
            if let Some(Event::SelectionNotify(notify)) = event {
                return Ok(Some(notify.property));
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        Ok(None)
    }
}

fn intern_atom(conn: &RustConnection, name: &[u8]) -> Result<u32, PollError> {
    Ok(conn
        .intern_atom(false, name)
        .map_err(|e| PollError::Backend(e.to_string()))?
        .reply()
        .map_err(|e| PollError::Backend(e.to_string()))?
        .atom)
}

fn atom_name(conn: &RustConnection, atom: u32) -> Result<String, PollError> {
    let reply = conn
        .get_atom_name(atom)
        .map_err(|e| PollError::Backend(e.to_string()))?
        .reply()
        .map_err(|e| PollError::Backend(e.to_string()))?;
    Ok(String::from_utf8_lossy(&reply.name).into_owned())
}

impl ObservationSource for ClipboardMetadataSource {
    fn id(&self) -> &str {
        "linux.clipboard_metadata"
    }

    fn min_privacy_level(&self) -> PrivacyLevel {
        PrivacyLevel::WorkflowMetadata
    }

    fn poll(&mut self) -> Result<Vec<CapturedSignal>, PollError> {
        let owner_reply = self
            .conn
            .get_selection_owner(self.clipboard)
            .map_err(|e| PollError::Backend(e.to_string()))?
            .reply()
            .map_err(|e| PollError::Backend(e.to_string()))?;
        let owner = owner_reply.owner;

        if owner == 0 || Some(owner) == self.last_owner {
            self.last_owner = Some(owner);
            return Ok(Vec::new());
        }
        self.last_owner = Some(owner);

        // Ask the current owner what target formats it can provide.
        self.conn
            .convert_selection(
                self.requestor,
                self.clipboard,
                self.targets,
                self.incoming,
                x11rb::CURRENT_TIME,
            )
            .map_err(|e| PollError::Backend(e.to_string()))?
            .check()
            .map_err(|e| PollError::Backend(e.to_string()))?;
        self.conn
            .flush()
            .map_err(|e| PollError::Backend(e.to_string()))?;

        let Some(property) = self.wait_for_selection_notify(Duration::from_millis(200))? else {
            return Ok(Vec::new());
        };

        let targets_reply = self
            .conn
            .get_property(false, self.requestor, property, AtomEnum::ATOM, 0, 64)
            .map_err(|e| PollError::Backend(e.to_string()))?
            .reply()
            .map_err(|e| PollError::Backend(e.to_string()))?;
        let target_atoms: Vec<u32> = targets_reply
            .value
            .chunks_exact(4)
            .map(|c| u32::from_ne_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        // Prefer a plain-text-ish target if the owner offers one; otherwise fall
        // back to whatever the first offered target is. Either way we only ever
        // report *which* target was used, never the bytes behind it.
        let chosen_target = target_atoms
            .iter()
            .find(|&&a| {
                atom_name(&self.conn, a)
                    .map(|n| n.contains("UTF8") || n.contains("STRING"))
                    .unwrap_or(false)
            })
            .or_else(|| target_atoms.first())
            .copied();

        let Some(target) = chosen_target else {
            return Ok(Vec::new());
        };
        let content_type = atom_name(&self.conn, target)?;

        self.conn
            .convert_selection(
                self.requestor,
                self.clipboard,
                target,
                self.incoming,
                x11rb::CURRENT_TIME,
            )
            .map_err(|e| PollError::Backend(e.to_string()))?
            .check()
            .map_err(|e| PollError::Backend(e.to_string()))?;
        self.conn
            .flush()
            .map_err(|e| PollError::Backend(e.to_string()))?;

        let Some(property) = self.wait_for_selection_notify(Duration::from_millis(200))? else {
            return Ok(Vec::new());
        };
        let data_reply = self
            .conn
            .get_property(false, self.requestor, property, AtomEnum::ANY, 0, u32::MAX)
            .map_err(|e| PollError::Backend(e.to_string()))?
            .reply()
            .map_err(|e| PollError::Backend(e.to_string()))?;
        // `size_bytes` is the only thing derived from `data_reply.value` — the
        // buffer itself is dropped at the end of this scope and never placed into
        // a `CapturedPayload`.
        let size_bytes = data_reply.value.len();

        Ok(vec![CapturedSignal::new(
            self.id(),
            CapturedPayload::ClipboardMetadata {
                content_type,
                size_bytes,
            },
        )])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_correct_minimum_privacy_level() {
        let source = ClipboardMetadataSource::connect().expect("connect to X11 display");
        assert_eq!(source.min_privacy_level(), PrivacyLevel::WorkflowMetadata);
    }

    #[test]
    fn does_not_error_when_polled_against_the_real_display() {
        // Whatever the clipboard's current state is in this session, polling
        // must succeed without erroring — the specific content_type/size
        // returned (if any) depends on live desktop state this test doesn't
        // control, so it deliberately doesn't assert on those values.
        let mut source = ClipboardMetadataSource::connect().expect("connect to X11 display");
        let result = source.poll();
        assert!(result.is_ok(), "poll failed: {:?}", result.err());
    }

    #[test]
    fn second_immediate_poll_with_no_new_copy_yields_nothing() {
        let mut source = ClipboardMetadataSource::connect().expect("connect to X11 display");
        let _first = source.poll().unwrap();
        let second = source.poll().unwrap();
        assert!(second.is_empty());
    }
}
