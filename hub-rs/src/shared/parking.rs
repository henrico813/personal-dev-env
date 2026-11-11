//! Parks and reopens sessions, and keeps their parked records, one
//! JSON file per parked session. Read `park_session` and
//! `reopen_session` first. A parked session's tab is closed, and its
//! record is what lets it reopen. Each step writes or checks the record
//! at a point where a crash still leaves a session the next tidy pass
//! can handle.

use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::PathBuf;
use std::thread;

use jiff::{Timestamp, Zoned};
use serde::{Deserialize, Serialize};

use super::events::{self, Event, EventKind};
use super::facts::match_open_request;
use super::lifecycle::{Lifecycle, SessionFacts, Verdict, decide_lifecycle};
use super::run_folder::{self, RunFolder};
use super::{HubError, Tools, messages, tidy};
use crate::external::cards::CardKey;
use crate::external::herdr::{Agent, AgentStart, NewPlace, Pane, Workspace};

/// The most times reopening reads the new agent for its session ID.
const SESSION_ID_READS: u32 = 10;

/// What reopening a parked session needs. `hub status` shows its name,
/// card and time.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ParkedRecord {
    pub name: String,
    pub session_id: String,
    pub card: CardKey,
    /// The label of the workspace it left, for a new workspace when that
    /// one is gone.
    pub workspace_label: String,
    pub parked_at: Timestamp,
}

/// Whether `park_session` parked the session.
#[derive(Debug)]
pub(crate) enum ParkOutcome {
    /// The tab closed, and the record names this card.
    Parked(CardKey),
    /// Its state, worked out again, was no longer `to-park`.
    Stays(Verdict),
    /// Its tab gained a pane, a focus or a busy shell since the panes
    /// were read.
    TabInUse,
}

/// Holds `.tidy.lock` in the parked folder while one tidy pass or one
/// `hub park` runs. Dropping it, or the process ending, lets the next
/// one in.
pub(crate) struct TidyLock {
    _file: File,
}

/// The folder of parked records, `~/.local/state/pde/hub/parked/`,
/// shared by every hub on the machine.
pub struct ParkedFolder {
    folder: PathBuf,
}

impl ParkedFolder {
    pub fn new(folder: PathBuf) -> ParkedFolder {
        ParkedFolder { folder }
    }

    /// Returns every record, sorted by name, skipping files that do not
    /// parse. A missing folder is no records.
    pub(crate) fn list_parked_records(&self) -> Result<Vec<ParkedRecord>, HubError> {
        let entries = match fs::read_dir(&self.folder) {
            Ok(entries) => entries,
            Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(source) => return Err(self.make_file_error(source)),
        };
        let mut records = Vec::new();
        for entry in entries {
            let path = entry.map_err(|source| self.make_file_error(source))?.path();
            if path.extension().is_none_or(|extension| extension != "json") {
                continue;
            }
            let record = fs::read(&path)
                .ok()
                .and_then(|bytes| serde_json::from_slice::<ParkedRecord>(&bytes).ok());
            records.extend(record);
        }
        records.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(records)
    }

    /// Returns NAME's record, or `None` when it has none or its file
    /// does not parse.
    pub(crate) fn find_parked_record(&self, name: &str) -> Result<Option<ParkedRecord>, HubError> {
        let path = self.make_record_path(name);
        match fs::read(&path) {
            Ok(bytes) => Ok(serde_json::from_slice(&bytes).ok()),
            Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(source) => Err(HubError::File { path, source }),
        }
    }

    /// Writes a record whole, making the folder first.
    pub(crate) fn save_parked_record(&self, record: &ParkedRecord) -> Result<(), HubError> {
        fs::create_dir_all(&self.folder).map_err(|source| self.make_file_error(source))?;
        let path = self.make_record_path(&record.name);
        let text = serde_json::to_string_pretty(record).map_err(|error| HubError::File {
            path: path.clone(),
            source: error.into(),
        })?;
        run_folder::write_file_whole(&path, &text)
    }

    /// Removes NAME's record. A missing record is not an error.
    pub(crate) fn delete_parked_record(&self, name: &str) -> Result<(), HubError> {
        let path = self.make_record_path(name);
        match fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(source) => Err(HubError::File { path, source }),
        }
    }

    /// Waits for the lock on `.tidy.lock` and holds it, so two hubs, or
    /// `hub clean` beside `hub next`, never save or delete records at
    /// once.
    pub(crate) fn take_tidy_lock(&self) -> Result<TidyLock, HubError> {
        fs::create_dir_all(&self.folder).map_err(|source| self.make_file_error(source))?;
        let path = self.folder.join(".tidy.lock");
        let locked = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(false)
            .open(&path)
            .and_then(|file| file.lock().map(|()| file));
        match locked {
            Ok(file) => Ok(TidyLock { _file: file }),
            Err(source) => Err(HubError::File { path, source }),
        }
    }

    /// Returns `NAME.json` in the folder. A checked agent name holds no
    /// slash or dot.
    fn make_record_path(&self, name: &str) -> PathBuf {
        self.folder.join(format!("{name}.json"))
    }

    /// Returns the failure to read or make the folder itself.
    fn make_file_error(&self, source: io::Error) -> HubError {
        HubError::File {
            path: self.folder.clone(),
            source,
        }
    }
}

/// Parks a session whose state is `to-park`. The steps run in this order
/// so a crash at any step leaves a session the next pass handles.
/// 1. Work out the state again with a fresh agent and message count.
/// 2. Save the record whole.
/// 3. Link the session to its card while the agent still runs.
/// 4. Work out the state again, since a message may have come in, and
///    read the tab's panes again, to compare with `planned`.
/// 5. Close the tab, then log `parked`, write one `notes.md` line, and
///    add `parked NAME` to the card's Log.
///
/// A state other than `to-park` at step 1 or 4 returns
/// `ParkOutcome::Stays`, and a changed tab `ParkOutcome::TabInUse`,
/// and neither closes anything. A record left by step 4, or by a card
/// failure, belongs to a live session, and the next tidy pass deletes
/// it.
///
/// # Errors
///
/// A Herdr, card or file failure. `HubError::PartlyDone` when the tab
/// closed and the log or note failed.
pub(crate) fn park_session(
    tools: &Tools<'_>,
    run: &RunFolder,
    facts: &SessionFacts,
    planned: &[Pane],
    now: &Zoned,
) -> Result<ParkOutcome, HubError> {
    let name = facts.name.as_str();
    let (agent, verdict) = decide_fresh_lifecycle(tools, run, facts)?;
    if verdict.state != Lifecycle::ToPark {
        return Ok(ParkOutcome::Stays(verdict));
    }
    // `decide_lifecycle` parks only an OpenCode agent with a session ID
    // and a card.
    let (Some(session_id), Some(card)) = (agent.session_id.clone(), facts.card.as_ref()) else {
        return Err(HubError::NotDone(format!(
            "{name} has no session ID or card to park with"
        )));
    };
    let workspace_label = tools
        .herdr
        .list_workspaces()?
        .into_iter()
        .find(|workspace| workspace.workspace_id == agent.workspace_id)
        .map_or_else(|| card.key.as_str().to_owned(), |workspace| workspace.label);
    let record = ParkedRecord {
        name: name.to_owned(),
        session_id: session_id.clone(),
        card: card.key.clone(),
        workspace_label,
        parked_at: now.timestamp(),
    };
    tools.parked.save_parked_record(&record)?;
    tools.cards.link_session(&card.key, name)?;
    let (agent, recheck) = decide_fresh_lifecycle(tools, run, facts)?;
    if recheck.state != Lifecycle::ToPark {
        return Ok(ParkOutcome::Stays(recheck));
    }
    if !tidy::is_tab_still_free(tools, &agent.tab_id, planned)? {
        return Ok(ParkOutcome::TabInUse);
    }
    tools.herdr.close_tab(&agent.tab_id)?;

    let parked_but = |error: HubError| HubError::PartlyDone {
        done: format!("parked {name}"),
        error: Box::new(error),
    };
    let parked = Event {
        at: now.timestamp(),
        kind: EventKind::Parked {
            name: name.to_owned(),
            session: session_id,
            tab: agent.tab_id,
        },
    };
    events::append_event(run, &parked).map_err(parked_but)?;
    let note = format!("parked {name} ({} review)", card.key.as_str());
    run.append_note_line(&note, now).map_err(parked_but)?;
    tools
        .cards
        .add_note(&card.key, &format!("parked {name}"))
        .map_err(|error| parked_but(error.into()))?;
    Ok(ParkOutcome::Parked(record.card))
}

/// Starts a parked session again in a new tab of its card's workspace,
/// or in a new workspace when that one is gone, with `HUB_RUN` set to
/// this run folder. The record goes only once Herdr shows the parked
/// session ID, so a failed start keeps the way back.
///
/// The caller holds the tidy lock, and the record is read again under
/// it. A record that is gone, or a session that is live again, means
/// another process already reopened it. This then returns with nothing
/// done, and the message stays held.
///
/// # Errors
///
/// `HubError::NotSent` when the OpenCode database lacks the session,
/// since `--session` with an unknown ID starts an empty session. Also
/// `HubError::NotSent` when Herdr shows another session ID after the
/// start, or none after ten reads, after closing the tab it made. A
/// Herdr or file failure otherwise.
pub(crate) fn reopen_session(
    tools: &Tools<'_>,
    run: &RunFolder,
    record: &ParkedRecord,
    _lock: &TidyLock,
    now: &Zoned,
) -> Result<(), HubError> {
    let name = record.name.as_str();
    let not_sent = |reason: &str| HubError::NotSent {
        name: name.to_owned(),
        reason: reason.to_owned(),
    };
    let Some(record) = tools.parked.find_parked_record(name)? else {
        return Ok(());
    };
    match tools.herdr.get_agent(name) {
        Ok(_) => return Ok(()),
        Err(error) if error.is_agent_not_found() => {}
        Err(error) => return Err(error.into()),
    }
    let Some(folder) = tools.opencode.find_session(&record.session_id)? else {
        return Err(not_sent("the session is not in the database"));
    };
    let place = NewPlace {
        label: name.to_owned(),
        cwd: folder,
        env: vec![("HUB_RUN".to_owned(), run.path().display().to_string())],
    };
    let workspaces = tools.herdr.list_workspaces()?;
    let ids = match find_card_workspace(&workspaces, &record.card) {
        Some(workspace) => tools.herdr.create_tab(&workspace.workspace_id, &place)?,
        None => tools.herdr.create_workspace(&NewPlace {
            label: record.workspace_label.clone(),
            ..place
        })?,
    };
    tools.herdr.start_agent(&AgentStart {
        name: name.to_owned(),
        pane_id: ids.root_pane_id.clone(),
        args: build_resume_args(&record),
    })?;
    let started = wait_for_session_id(tools, name)?;
    let failure = match started.session_id.as_deref() {
        Some(id) if id == record.session_id => None,
        Some(_) => Some("another session started"),
        None => Some("Herdr showed no session ID"),
    };
    if let Some(reason) = failure {
        // The new tab holds nothing yet, and a held message must not
        // reach a session that knows nothing of the work. Only the tab
        // this reopen made closes.
        if started.pane_id == ids.root_pane_id {
            let _ = tools.herdr.close_tab(&started.tab_id);
        }
        return Err(not_sent(reason));
    }
    tools.parked.delete_parked_record(name)?;
    let reopened = Event {
        at: now.timestamp(),
        kind: EventKind::Reopened {
            name: name.to_owned(),
            session: record.session_id.clone(),
        },
    };
    events::append_event(run, &reopened)
}

/// Works out a session's state again from a fresh Herdr agent, message
/// count and request folder. The card and git facts are kept, since
/// reading them again costs a network-drive call and a git run.
pub(crate) fn decide_fresh_lifecycle(
    tools: &Tools<'_>,
    run: &RunFolder,
    facts: &SessionFacts,
) -> Result<(Agent, Verdict), HubError> {
    let agent = tools.herdr.get_agent(&facts.name)?;
    let counts = messages::count_messages(run)?;
    let open = tools
        .requests
        .list_open_requests()
        .map_err(|source| HubError::File {
            path: tools.requests.folder().to_owned(),
            source,
        })?;
    let fresh = SessionFacts {
        messages: counts.get(&facts.name).copied().unwrap_or_default(),
        approval_pending: facts.approval_pending
            || open.bad_files > 0
            || match_open_request(&agent, &open.requests),
        agent: Some(agent.clone()),
        ..facts.clone()
    };
    Ok((agent, decide_lifecycle(&fresh)))
}

/// Reads a just-started agent until Herdr shows a session ID, at most
/// `SESSION_ID_READS` times, `tools.reopen_poll` apart.
fn wait_for_session_id(tools: &Tools<'_>, name: &str) -> Result<Agent, HubError> {
    let mut agent = tools.herdr.get_agent(name)?;
    for _ in 1..SESSION_ID_READS {
        if agent.session_id.is_some() {
            break;
        }
        thread::sleep(tools.reopen_poll);
        agent = tools.herdr.get_agent(name)?;
    }
    Ok(agent)
}

/// Returns the agent arguments that resume the session,
/// `--session ses_x --auto`.
fn build_resume_args(record: &ParkedRecord) -> Vec<String> {
    vec![
        "--session".to_owned(),
        record.session_id.clone(),
        "--auto".to_owned(),
    ]
}

/// Returns the workspace whose label starts with the card key as its
/// own word, such as `DEMO-KB-007 impl`, if one is left.
fn find_card_workspace<'a>(workspaces: &'a [Workspace], card: &CardKey) -> Option<&'a Workspace> {
    workspaces.iter().find(|workspace| {
        workspace
            .label
            .split_whitespace()
            .next()
            .is_some_and(|first| first.eq_ignore_ascii_case(card.as_str()))
    })
}
