//! Runs `hub read`, `hub report` and `hub note`, the commands that
//! move reports and notes through the run folder.

use std::fs;

use jiff::Zoned;

use super::Outcome;
use crate::external::cards::CardKey;
use crate::shared::events::{self, Event, EventKind};
use crate::shared::run_folder::RunFolder;
use crate::shared::{AgentName, HubError, Tools};

/// What `hub read` reads, and where its note goes.
pub struct ReadRequest {
    pub name: AgentName,
    /// A line for `notes.md`, stamped with the read time.
    pub note: Option<String>,
    pub card: CardChoice,
}

/// Which card, if any, gets the note as a Log line.
pub enum CardChoice {
    NoCard,
    /// The key in the session's name.
    FromName,
    Given(CardKey),
}

/// Prints a report, archives it, logs the note and lists the other
/// unread reports. The report is renamed before it is read, so one
/// written meanwhile stays unread.
///
/// # Errors
///
/// `HubError::NotDone` when NAME has no unread report, and
/// `HubError::Usage` when `--card` is given with no key and the
/// session's name has no key either. Both change nothing. If a step
/// after the archive fails, the error is `HubError::PartlyDone` and
/// names the archive. A second `hub read` no longer finds the report.
/// A failed `card note` comes last, after the `notes.md` line.
pub fn run_read(
    tools: &Tools<'_>,
    run: &RunFolder,
    request: &ReadRequest,
    now: &Zoned,
) -> Result<Outcome, HubError> {
    let name = request.name.as_str();
    let card_key = match &request.card {
        CardChoice::NoCard => None,
        CardChoice::FromName => Some(CardKey::find_in_name(name).ok_or_else(|| {
            HubError::Usage(format!("{name} holds no card key; pass --card KEY"))
        })?),
        CardChoice::Given(key) => Some(key.clone()),
    };
    let Some(archive) = run.archive_report(&request.name, now)? else {
        return Err(HubError::NotDone(format!("no unread report from {name}")));
    };
    let archive_name = archive
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    let archived_but = |error: HubError| HubError::PartlyDone {
        done: format!("archived {archive_name}"),
        error: Box::new(error),
    };
    let read = Event {
        at: now.timestamp(),
        kind: EventKind::Read {
            name: name.to_owned(),
            archive: archive_name.clone(),
        },
    };
    events::append_event(run, &read).map_err(archived_but)?;
    if let Some(note) = &request.note {
        run.append_note_line(note, now).map_err(archived_but)?;
    }
    let text = fs::read_to_string(&archive).map_err(|source| {
        archived_but(HubError::File {
            path: archive.clone(),
            source,
        })
    })?;
    let others = run.list_reports().map_err(archived_but)?;
    let mut lines: Vec<String> = text.lines().map(str::to_owned).collect();
    lines.push(if others.is_empty() {
        "unread: -".to_owned()
    } else {
        format!("unread: {}", others.join(" "))
    });
    // clap lets `--card` through only with `--note`.
    if let (Some(key), Some(note)) = (card_key, &request.note) {
        tools
            .cards
            .add_note(&key, note)
            .map_err(|error| archived_but(error.into()))?;
    }
    Ok(Outcome { lines })
}

/// Writes a worker's report whole, so the hub never reads half of it.
/// Empty text writes nothing and returns `HubError::NotDone`, so an
/// empty standard input never blanks a real report.
pub fn run_report(run: &RunFolder, name: &AgentName, text: &str) -> Result<Outcome, HubError> {
    if text.trim().is_empty() {
        return Err(HubError::NotDone(
            "empty report; nothing written".to_owned(),
        ));
    }
    run.write_report(name, text)?;
    Ok(Outcome::line(format!(
        "report {} written, {} lines",
        name.as_str(),
        text.lines().count()
    )))
}

/// Adds a line stamped with the real local time to `notes.md`. When
/// the day changed, it adds a date heading first.
pub fn run_note(run: &RunFolder, text: &str, now: &Zoned) -> Result<Outcome, HubError> {
    let stamp = run.append_note_line(text, now)?;
    Ok(Outcome::line(format!("noted {stamp}")))
}
