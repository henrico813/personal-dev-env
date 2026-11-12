//! Runs `hub at`, which stores a message to send at a time today, and
//! holds each stored message once its time comes, for `hub next`. Read
//! `run_at`, then `hold_due_messages`.
//!
//! Each timed message is one file in the run folder's `timed/`, named
//! by its time and a number, such as `20261111-2000.1.json`. Deleting
//! the file before its time cancels the message.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use jiff::civil::Time;
use jiff::{Timestamp, Zoned};
use serde::{Deserialize, Serialize};

use super::Outcome;
use crate::external::cards::CardKey;
use crate::external::herdr::Agent;
use crate::shared::HubError;
use crate::shared::events::{self, Event, EventKind};
use crate::shared::lifecycle::is_hub_name;
use crate::shared::messages;
use crate::shared::run_folder::RunFolder;

/// One timed message to store, with the text already read from
/// `--file`.
pub struct AtRequest {
    /// The time today, in this machine's time zone.
    pub time: Time,
    pub to: Recipients,
    /// The file the text came from, which `hub next` names in its line.
    pub file: PathBuf,
    pub text: String,
}

/// Who a timed message goes to.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Recipients {
    /// These agent names.
    Names(Vec<String>),
    /// Every live worker, found when the time comes.
    AllWorkers,
}

/// One timed message, as its file holds it.
#[derive(Serialize, Deserialize)]
struct TimedMessage {
    at: Timestamp,
    to: Recipients,
    file: PathBuf,
    text: String,
}

/// Stores the message for its time today and prints where it went.
/// Refuses a time that has passed today, rather than taking it as
/// tomorrow.
///
/// # Errors
///
/// `HubError::Usage` when the time today is not after now.
pub fn run_at(run: &RunFolder, request: AtRequest, now: &Zoned) -> Result<Outcome, HubError> {
    let due = now
        .with()
        .time(request.time)
        .build()
        .map_err(|error| HubError::Usage(error.to_string()))?;
    let clock = due.strftime("%H:%M").to_string();
    if due <= *now {
        return Err(HubError::Usage(format!(
            "{clock} has passed today; give a later time"
        )));
    }
    let to = match &request.to {
        Recipients::Names(names) => names.join(", "),
        Recipients::AllWorkers => "every live worker".to_owned(),
    };
    let message = TimedMessage {
        at: due.timestamp(),
        to: request.to,
        file: request.file,
        text: request.text,
    };
    let stamp = due.strftime("%Y%m%d-%H%M").to_string();
    let path = store_timed_message(run, &message, &stamp)?;
    Ok(Outcome::line(format!(
        "at {clock} to {to}: {}",
        path.display()
    )))
}

/// Holds each timed message whose time has come for each of its
/// sessions, logs `timed_held` and deletes its file. The held messages
/// then go out as `hub send`'s held ones do. A crash or an error after
/// some holds holds the message again on the next check.
pub(crate) fn hold_due_messages(
    run: &RunFolder,
    agents: &[Agent],
    now: &Zoned,
) -> Result<(), HubError> {
    for (id, path) in list_timed_files(run)? {
        // A file deleted since the listing was cancelled.
        let Some(message) = read_timed_message(&path)? else {
            continue;
        };
        if message.at > now.timestamp() {
            continue;
        }
        let names: Vec<String> = match message.to {
            Recipients::Names(names) => names,
            Recipients::AllWorkers => list_live_workers(run, agents)?,
        };
        for name in &names {
            messages::hold_message(run, name, None, &message.text, now)?;
        }
        let held = Event {
            at: now.timestamp(),
            kind: EventKind::TimedHeld {
                id,
                file: message.file,
                names,
            },
        };
        events::append_event(run, &held)?;
        fs::remove_file(&path).map_err(|source| HubError::File { path, source })?;
    }
    Ok(())
}

/// Returns the names of the live workers. A worker's name holds a card
/// key, or it has reported in this run, and it is never a hub. Other
/// sessions are the user's own.
fn list_live_workers(run: &RunFolder, agents: &[Agent]) -> Result<Vec<String>, HubError> {
    let reported = run.list_reported_names()?;
    Ok(agents
        .iter()
        .filter(|agent| !is_hub_name(&agent.name))
        .filter(|agent| {
            CardKey::find_in_name(&agent.name).is_some() || reported.any.contains(&agent.name)
        })
        .map(|agent| agent.name.clone())
        .collect())
}

/// Writes the message whole under its stamp and the first free number,
/// and returns its path.
fn store_timed_message(
    run: &RunFolder,
    message: &TimedMessage,
    stamp: &str,
) -> Result<PathBuf, HubError> {
    let folder = run.path().join("timed");
    fs::create_dir_all(&folder).map_err(|source| HubError::File {
        path: folder.clone(),
        source,
    })?;
    // The `.tmp` ending keeps the file out of `list_timed_files`.
    let temporary = folder.join(format!("{}.tmp", std::process::id()));
    let text = serde_json::to_string(message).map_err(|error| HubError::File {
        path: temporary.clone(),
        source: error.into(),
    })?;
    fs::write(&temporary, text).map_err(|source| HubError::File {
        path: temporary.clone(),
        source,
    })?;
    let mut number = 1;
    let stored = loop {
        let path = folder.join(format!("{stamp}.{number}.json"));
        // A hard link fails when the name exists, so a message stored
        // for the same minute is never replaced.
        match fs::hard_link(&temporary, &path) {
            Ok(()) => break Ok(path),
            Err(source) if source.kind() == io::ErrorKind::AlreadyExists => number += 1,
            Err(source) => break Err(HubError::File { path, source }),
        }
    };
    let _ = fs::remove_file(&temporary);
    stored
}

/// Returns each timed message's ID and file, by ID, such as
/// `20261111-2000.1`.
fn list_timed_files(run: &RunFolder) -> Result<Vec<(String, PathBuf)>, HubError> {
    let folder = run.path().join("timed");
    let entries = match fs::read_dir(&folder) {
        Ok(entries) => entries,
        Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(source) => {
            return Err(HubError::File {
                path: folder,
                source,
            });
        }
    };
    let mut found = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|source| HubError::File {
            path: folder.clone(),
            source,
        })?;
        let file_name = entry.file_name().to_string_lossy().into_owned();
        if let Some(id) = file_name.strip_suffix(".json") {
            found.push((id.to_owned(), entry.path()));
        }
    }
    found.sort();
    Ok(found)
}

/// Reads one timed message file, or returns `None` when it is gone.
fn read_timed_message(path: &Path) -> Result<Option<TimedMessage>, HubError> {
    let file_error = |source: io::Error| HubError::File {
        path: path.to_owned(),
        source,
    };
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(source) => return Err(file_error(source)),
    };
    serde_json::from_str(&text)
        .map(Some)
        .map_err(|error| file_error(error.into()))
}
