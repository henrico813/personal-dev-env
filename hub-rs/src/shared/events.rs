//! Keeps the hub's memory as one append-only log, `events.jsonl`.
//! Read `append_event` first. Each line is one JSON object, such as
//! `{"at":"2026-11-11T03:14:40Z","event":"read","name":"docs-fix","archive":"report-docs-fix.2214.read"}`.

use std::fs::{self, OpenOptions};
use std::io::{self, Write};

use jiff::Timestamp;
use serde::Serialize;

use super::HubError;
use super::run_folder::RunFolder;

/// One line of the event log.
#[derive(Serialize)]
pub(crate) struct Event {
    pub at: Timestamp,
    // Writes the fields of `kind` beside `at` in the same object.
    #[serde(flatten)]
    pub kind: EventKind,
}

/// What happened, written as the line's `event` field in snake case.
#[derive(Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub(crate) enum EventKind {
    /// `hub read` archived NAME's report as `archive`.
    Read { name: String, archive: String },
    /// `hub handoff` started `new` after `old`, whose tab is `tab`.
    Handoff {
        old: String,
        tab: String,
        new: String,
    },
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
