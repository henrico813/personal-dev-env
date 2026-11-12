//! Keeps what the watch did as its own kinds of line in the run's
//! `events.jsonl`, and folds them into a `WatchMemory`. Read
//! `load_watch_memory` and `append_watch_event` first. The hub's
//! `load_memory` skips these kinds. This fold skips the lines the hub's
//! own commands write, except `handoff`, which names the run's next
//! hub. The one line the watch writes for `hub next`, `report_missed`,
//! is of the hub's own kinds.

use std::collections::BTreeMap;

use jiff::{Timestamp, Zoned};
use serde::{Deserialize, Serialize};

use crate::shared::HubError;
use crate::shared::events;
use crate::shared::run_folder::RunFolder;

/// One line of the log that the watch reads or writes.
#[derive(Serialize, Deserialize)]
struct WatchLine {
    at: Timestamp,
    // Writes the fields of `event` beside `at` in the same object.
    #[serde(flatten)]
    event: WatchEvent,
}

/// What happened, written as the line's `event` field in snake case.
#[derive(Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub(crate) enum WatchEvent {
    /// `hub handoff` or `hub adopt` made `new` the run's hub. The watch
    /// only reads this kind, and reads only `new`.
    Handoff { new: String },
    /// `hub watch start` named `hub` as the run's hub.
    WatchStarted { hub: String },
    /// The watch prompted worker NAME while Herdr's state number was
    /// `state_seq`, for `why`, such as a stall or an ended wait.
    WorkerProdded {
        name: String,
        state_seq: u64,
        why: String,
    },
    /// The watch sent a phone notification, because NAME's report
    /// modified at `mtime_ns` waited 10 minutes while the run's hub was
    /// gone from Herdr.
    PhoneAlerted { name: String, mtime_ns: u64 },
}

/// What the watch remembers of a run, folded from the log in file
/// order, later events winning.
#[derive(Debug, Default)]
pub(crate) struct WatchMemory {
    /// The run's hub, from the last `handoff` or `watch_started` event.
    /// The watch checks a run's workers only while this hub runs.
    pub hub: Option<String>,
    /// The Herdr state number of each worker's last prod.
    pub prodded: BTreeMap<String, u64>,
    /// The `mtime_ns` of each name's last report that sent a phone
    /// notification.
    pub alerted: BTreeMap<String, u64>,
}

/// Adds one watch event at `now` to the run's log.
pub(crate) fn append_watch_event(
    run: &RunFolder,
    event: WatchEvent,
    now: &Zoned,
) -> Result<(), HubError> {
    let line = WatchLine {
        at: now.timestamp(),
        event,
    };
    events::append_json_line(run, &line)
}

/// Folds every watch line of the run's log into the memory. A line of
/// another kind, or one cut off, does not parse and is skipped.
pub(crate) fn load_watch_memory(run: &RunFolder) -> Result<WatchMemory, HubError> {
    let log = run.read_optional_file("events.jsonl")?;
    let mut memory = WatchMemory::default();
    let lines = log
        .lines()
        .filter_map(|line| serde_json::from_str::<WatchLine>(line).ok());
    for line in lines {
        match line.event {
            WatchEvent::Handoff { new } => memory.hub = Some(new),
            WatchEvent::WatchStarted { hub } => memory.hub = Some(hub),
            WatchEvent::WorkerProdded {
                name, state_seq, ..
            } => {
                memory.prodded.insert(name, state_seq);
            }
            WatchEvent::PhoneAlerted { name, mtime_ns } => {
                memory.alerted.insert(name, mtime_ns);
            }
        }
    }
    Ok(memory)
}
