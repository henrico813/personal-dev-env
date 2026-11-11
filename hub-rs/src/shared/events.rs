//! Keeps the hub's memory as one append-only log, `events.jsonl`, and
//! folds it into a `Memory` value. Read `append_event` and
//! `load_memory` first. Each line is one JSON object, such as
//! `{"at":"2026-11-11T03:14:40Z","event":"read","name":"docs-fix","archive":"report-docs-fix.2214.read"}`.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::PathBuf;

use jiff::Timestamp;
use serde::{Deserialize, Serialize};

use super::HubError;
use super::messages::MessageId;
use super::run_folder::RunFolder;

/// The longest reason a line keeps, so each line stays one small write.
const REASON_LIMIT: usize = 300;

/// One line of the event log.
#[derive(Serialize, Deserialize)]
pub(crate) struct Event {
    pub at: Timestamp,
    // Writes the fields of `kind` beside `at` in the same object.
    #[serde(flatten)]
    pub kind: EventKind,
}

/// What happened, written as the line's `event` field in snake case.
#[derive(Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub(crate) enum EventKind {
    /// `hub next` printed the version of NAME's report modified at
    /// `mtime_ns`.
    Announced { name: String, mtime_ns: u64 },
    /// `hub read` archived NAME's report as `archive`.
    Read { name: String, archive: String },
    /// A message waits in `sends/` until NAME can take it.
    Held { name: String, id: MessageId },
    /// The message reached NAME through `via`.
    Delivered {
        name: String,
        id: MessageId,
        via: Via,
    },
    /// Herdr failed after it may have taken the text.
    Unsure {
        name: String,
        id: MessageId,
        reason: String,
    },
    /// Herdr turned the message down before taking it.
    NotSent {
        name: String,
        id: MessageId,
        reason: String,
    },
    /// NAME handed off, so its message now waits for `to`.
    Moved {
        name: String,
        id: MessageId,
        to: String,
    },
    /// `hub next` printed NAME's blocked state number `state_seq`.
    BlockedSeen { name: String, state_seq: u64 },
    /// `hub next` asked worker NAME, in session `session`, to hand off.
    HandoffAsked { name: String, session: String },
    /// `hub next` told the hub with this session ID to hand off.
    HubContextTold { session: String },
    /// `hub next` printed that the board went over its limit, or the
    /// run's first minute pass found it over and printed nothing.
    BoardOver { busy: u32, limit: u32 },
    /// The minute pass saw the board back within its limit, or within
    /// it at the run's first pass.
    BoardUnder { busy: u32, limit: u32 },
    /// `hub next` printed that NAMES became `state`, `to-close` or
    /// `to-park`.
    Listed { state: String, names: Vec<String> },
    /// The minute pass saw NAMES leave the state they were listed in.
    Unlisted { names: Vec<String> },
    /// `hub next` printed that the old hub's message file `path` waits.
    OldSendSeen { path: PathBuf },
    /// `hub handoff` started `new` after `old`, whose tab is `tab`.
    Handoff {
        old: String,
        tab: String,
        new: String,
    },
    /// NAME's tab `tab` closed after its parked record was saved.
    Parked {
        name: String,
        session: String,
        tab: String,
    },
    /// A parked NAME started again in a new tab.
    Reopened { name: String, session: String },
    /// The tidy pass closed NAME's tab `tab`, for `why`.
    Closed {
        name: String,
        tab: String,
        why: String,
    },
}

/// Which command delivered a message.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Via {
    Send,
    Next,
    Inbox,
}

/// What the hub remembers, folded from the log in file order, later
/// events winning.
#[derive(Debug, Default)]
pub(crate) struct Memory {
    /// The `mtime_ns` of each name's last announced report.
    pub announced: BTreeMap<String, u64>,
    /// The `state_seq` of each name's last printed blocked state.
    pub blocked_seen: BTreeMap<String, u64>,
    /// Hub session IDs told to hand off.
    pub hub_context_told: BTreeSet<String>,
    /// Worker session IDs that got a handoff ask.
    pub handoff_asked: BTreeSet<String>,
    /// The board was over its limit at the last crossing.
    pub board_over: bool,
    /// A minute pass saw the board, so a later pass can tell a
    /// crossing from the board as it was when the run started.
    pub board_known: bool,
    /// The state, `to-close` or `to-park`, each name was last listed in.
    pub listed: BTreeMap<String, String>,
    /// Old hub message files `hub next` already printed.
    pub old_send_seen: BTreeSet<PathBuf>,
    /// How many `read` events the log holds, for the minute pass.
    pub read_count: usize,
    /// When `hub read` last archived each name's report.
    pub last_read: BTreeMap<String, Timestamp>,
    /// When a message last reached each name.
    pub last_delivered: BTreeMap<String, Timestamp>,
    /// The old hub named in the last `handoff` event.
    pub predecessor: Option<String>,
}

/// Adds one line to the log in a single write. Writes a newline first
/// when the file does not end in one, so a cut-off line never joins
/// the next.
pub(crate) fn append_event(run: &RunFolder, event: &Event) -> Result<(), HubError> {
    let log = run.path().join("events.jsonl");
    let file_error = |source: io::Error| HubError::File {
        path: log.clone(),
        source,
    };
    let mut line = serde_json::to_string(event).map_err(|error| file_error(error.into()))?;
    line.push('\n');
    let ends_cut_off = match fs::read(&log) {
        Ok(existing) => existing.last().is_some_and(|last| *last != b'\n'),
        Err(source) if source.kind() == io::ErrorKind::NotFound => false,
        Err(source) => return Err(file_error(source)),
    };
    if ends_cut_off {
        line.insert(0, '\n');
    }
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log)
        .map_err(file_error)?;
    file.write_all(line.as_bytes()).map_err(file_error)
}

/// Returns the first 300 characters of a reason for an event.
pub(crate) fn cut_reason(reason: &str) -> String {
    reason.chars().take(REASON_LIMIT).collect()
}

/// Folds every line of the log that parses into the memory. A missing
/// log is an empty memory.
pub(crate) fn load_memory(run: &RunFolder) -> Result<Memory, HubError> {
    let log = run.read_optional_file("events.jsonl")?;
    Ok(fold_events(log.lines().filter_map(parse_event_line)))
}

/// Builds the memory from events in file order.
fn fold_events(events: impl IntoIterator<Item = Event>) -> Memory {
    let mut memory = Memory::default();
    for event in events {
        match event.kind {
            EventKind::Announced { name, mtime_ns } => {
                memory.announced.insert(name, mtime_ns);
            }
            EventKind::BlockedSeen { name, state_seq } => {
                memory.blocked_seen.insert(name, state_seq);
            }
            EventKind::HubContextTold { session } => {
                memory.hub_context_told.insert(session);
            }
            EventKind::Read { name, .. } => {
                memory.last_read.insert(name, event.at);
                memory.read_count += 1;
            }
            EventKind::HandoffAsked { session, .. } => {
                memory.handoff_asked.insert(session);
            }
            EventKind::BoardOver { .. } => {
                memory.board_over = true;
                memory.board_known = true;
            }
            EventKind::BoardUnder { .. } => {
                memory.board_over = false;
                memory.board_known = true;
            }
            EventKind::OldSendSeen { path } => {
                memory.old_send_seen.insert(path);
            }
            EventKind::Listed { state, names } => {
                for name in names {
                    memory.listed.insert(name, state.clone());
                }
            }
            EventKind::Unlisted { names } => {
                for name in names {
                    memory.listed.remove(&name);
                }
            }
            EventKind::Delivered { name, .. } => {
                memory.last_delivered.insert(name, event.at);
            }
            EventKind::Handoff { old, .. } => memory.predecessor = Some(old),
            EventKind::Held { .. }
            | EventKind::Unsure { .. }
            | EventKind::NotSent { .. }
            | EventKind::Moved { .. }
            | EventKind::Parked { .. }
            | EventKind::Reopened { .. }
            | EventKind::Closed { .. } => {}
        }
    }
    memory
}

/// Turns one log line into an event, or `None` for a cut-off line or
/// an event this version does not know, so an older hub reads a newer
/// hub's log.
fn parse_event_line(line: &str) -> Option<Event> {
    serde_json::from_str(line).ok()
}
