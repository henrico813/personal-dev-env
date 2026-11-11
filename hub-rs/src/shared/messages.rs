//! Holds, delivers and moves messages to sessions. Read
//! `choose_delivery` first. It is pure, and every rule about when a
//! message goes is in it. `deliver_message` holds what happens when
//! Herdr takes or turns down the text.
//!
//! Each message is one file in the run folder's `sends/`, named
//! `NAME.STAMP.N` and a state, such as
//! `docs-fix.20261110-221502.1.held`. A message leaves its file only by
//! a rename, or once Herdr or `hub inbox` took it, so a crash leaves
//! the text on disk.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use jiff::{Timestamp, Zoned};
use regex::Regex;
use serde::{Deserialize, Serialize};

use super::events::{self, Event, EventKind, Via};
use super::run_folder::{self, RunFolder};
use super::{HubError, Tools};
use crate::external::cards::{CardKey, Cards};
use crate::external::herdr::{Agent, AgentKind, AgentStatus, Herdr, HerdrCode, HerdrError};

/// OpenCode's input line when it holds a long message it has not sent,
/// such as `┃  [Pasted ~1 lines]`.
static PASTED_TEXT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^\s*┃\s+\[Pasted ~\d+ lines?\]\s*$").unwrap());

/// A message's file name without its state, such as
/// `docs-fix.20261110-221502.1`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct MessageId(String);

/// One message, as its file holds it.
#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Message {
    pub name: String,
    /// The saved Herdr machine it goes through, or `None` for this one.
    pub machine: Option<String>,
    pub text: String,
    pub created: Timestamp,
    /// Why it was not sent, when Herdr turned it down or the session is
    /// gone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Which file a message sits in, from its suffix.
#[derive(Clone, Copy, Debug)]
pub(crate) enum SendState {
    Held,
    Sending,
    NotSent,
}

impl SendState {
    /// Returns the file name's last part, such as `held`.
    fn suffix(self) -> &'static str {
        match self {
            SendState::Held => "held",
            SendState::Sending => "sending",
            SendState::NotSent => "notsent",
        }
    }
}

/// A message and its file.
#[derive(Debug)]
pub(crate) struct StoredMessage {
    pub id: MessageId,
    pub message: Message,
    pub path: PathBuf,
}

/// Where a message to a name can go.
#[derive(Debug)]
pub(crate) enum Target {
    Live(Agent),
    /// The newest live session of the same card, after a handoff.
    HandedOff(String),
    Gone,
}

/// What to do with a message now.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Delivery {
    Now,
    Hold(HoldReason),
    MoveTo(String),
    Gone,
}

/// Why a message waits.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum HoldReason {
    Blocked,
    UserInPane,
    OlderWaits(usize),
    /// A kind such as Codex, which takes messages only when idle.
    BusyOtherKind(String),
    StateUnknown,
}

impl HoldReason {
    /// Returns the words after `held NAME: `, such as `it is blocked`.
    pub(crate) fn words(&self) -> String {
        match self {
            HoldReason::Blocked => "it is blocked".to_owned(),
            HoldReason::UserInPane => "the user is in its pane".to_owned(),
            HoldReason::OlderWaits(1) => "1 older message waits".to_owned(),
            HoldReason::OlderWaits(count) => format!("{count} older messages wait"),
            HoldReason::BusyOtherKind(kind) => format!("{kind} takes messages when idle"),
            HoldReason::StateUnknown => "its state is unknown".to_owned(),
        }
    }
}

/// How one delivery went.
#[derive(Debug)]
pub(crate) enum SendOutcome {
    Delivered,
    NotSent {
        name: String,
        reason: String,
    },
    Unsure {
        name: String,
        reason: String,
        file: PathBuf,
    },
}

/// How many of a session's messages wait, may have arrived, or were
/// not sent, from their files in `sends/`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct MessageCounts {
    pub held: usize,
    /// `.sending` files. A process killed while sending leaves one too.
    pub unsure: usize,
    pub not_sent: usize,
}

impl MessageCounts {
    /// Tells whether any message waits, is unsure or was not sent.
    pub(crate) fn any(&self) -> bool {
        self.held + self.unsure + self.not_sent > 0
    }
}

/// Decides whether a message goes now, waits or moves. OpenCode and
/// Claude Code take a message while working, idle or done; other kinds
/// only when idle or done. A blocked, focused or unknown session
/// waits, and so does any message behind an older one.
pub(crate) fn choose_delivery(target: &Target, older_waiting: usize) -> Delivery {
    let agent = match target {
        Target::Live(agent) => agent,
        Target::HandedOff(successor) => return Delivery::MoveTo(successor.clone()),
        Target::Gone => return Delivery::Gone,
    };
    let hold = match (&agent.kind, agent.status) {
        (_, AgentStatus::Blocked) => HoldReason::Blocked,
        _ if agent.focused => HoldReason::UserInPane,
        (_, AgentStatus::Unknown) => HoldReason::StateUnknown,
        _ if older_waiting > 0 => HoldReason::OlderWaits(older_waiting),
        (AgentKind::Other(kind), AgentStatus::Working) => HoldReason::BusyOtherKind(kind.clone()),
        _ => return Delivery::Now,
    };
    Delivery::Hold(hold)
}

/// Sends each held message whose session can take it, oldest first,
/// and moves the messages of sessions that handed off. Each session
/// gets at most one message per check, since the first may block it
/// and Herdr's prompt returns once it blocks. Stops at the first
/// message that was not sent or is unsure, and returns it. A later
/// message with the same problem waits for the next `hub next` check,
/// so each one gets its own line.
pub(crate) fn deliver_held_messages(
    tools: &Tools<'_>,
    run: &RunFolder,
    agents: &[Agent],
    now: &Zoned,
) -> Result<Option<SendOutcome>, HubError> {
    // Names that got a message or have one left waiting in this check.
    let mut waiting_names = BTreeSet::new();
    // Each saved machine's agents, or `None` when its Herdr failed.
    let mut machine_agents: BTreeMap<String, Option<Vec<Agent>>> = BTreeMap::new();
    for stored in list_messages(run, None)? {
        let name = stored.message.name.clone();
        if waiting_names.contains(&name) {
            continue;
        }
        let machine_herdr;
        let (herdr, known): (&dyn Herdr, &[Agent]) = match &stored.message.machine {
            None => (tools.herdr, agents),
            Some(label) => {
                machine_herdr = tools.herdr.on_machine(label);
                let listed = machine_agents
                    .entry(label.clone())
                    .or_insert_with(|| machine_herdr.list_agents().ok());
                // An unreachable machine keeps its messages for a later check.
                let Some(listed) = listed else {
                    waiting_names.insert(name);
                    continue;
                };
                (machine_herdr.as_ref(), listed.as_slice())
            }
        };
        let target = match known.iter().find(|agent| agent.name == name) {
            Some(agent) => Target::Live(agent.clone()),
            None => find_card_successor(tools.cards, &name, known),
        };
        // Each session gets one message per check, so none of its older
        // ones waits here.
        let mut delivery = choose_delivery(&target, 0);
        // An earlier prompt in this check can take a minute, so the
        // session is read again.
        if delivery == Delivery::Now {
            delivery = match herdr.get_agent(&name) {
                Ok(agent) => choose_delivery(&Target::Live(agent), 0),
                Err(_) => Delivery::Hold(HoldReason::StateUnknown),
            };
        }
        match delivery {
            Delivery::Now => {
                let Some(claimed) = claim_message(stored)? else {
                    continue;
                };
                let outcome = deliver_message(herdr, run, claimed, Via::Next, now)?;
                if !matches!(outcome, SendOutcome::Delivered) {
                    return Ok(Some(outcome));
                }
                waiting_names.insert(name);
            }
            Delivery::Hold(_) => {
                waiting_names.insert(name);
            }
            Delivery::MoveTo(successor) => {
                if let Some(claimed) = claim_message(stored)? {
                    move_message(run, claimed, &successor, now)?;
                }
            }
            Delivery::Gone => {
                if let Some(claimed) = claim_message(stored)? {
                    let outcome = mark_not_sent(run, claimed, "the session is gone", now)?;
                    return Ok(Some(outcome));
                }
            }
        }
    }
    Ok(None)
}

/// Prompts a claimed message's session and records how it went. A
/// stall gets one Enter press when the text sits pasted and the pane
/// is free. The file stays as `.sending` when the text may have
/// arrived, and becomes `.notsent` with the reason when it did not. A
/// step that fails after Herdr took the text returns
/// `HubError::PartlyDone`, whose line starts with `sent NAME`.
pub(crate) fn deliver_message(
    herdr: &dyn Herdr,
    run: &RunFolder,
    stored: StoredMessage,
    via: Via,
    now: &Zoned,
) -> Result<SendOutcome, HubError> {
    let name = stored.message.name.clone();
    let mut result = herdr.prompt(&name, &stored.message.text);
    if let Err(HerdrError::Refused {
        code: HerdrCode::AgentPromptStalled,
        ..
    }) = &result
        && submit_pasted_text(herdr, &name)
    {
        result = Ok(());
    }
    match result {
        Ok(()) => {
            let done = format!("sent {name}");
            let sent_but = |error: HubError| HubError::PartlyDone {
                done: done.clone(),
                error: Box::new(error),
            };
            fs::remove_file(&stored.path).map_err(|source| {
                sent_but(HubError::File {
                    path: stored.path.clone(),
                    source,
                })
            })?;
            let delivered = EventKind::Delivered {
                name,
                id: stored.id,
                via,
            };
            append_message_event(run, delivered, now).map_err(sent_but)?;
            Ok(SendOutcome::Delivered)
        }
        Err(error) if error.may_have_arrived() => {
            let reason = error.to_string();
            let unsure = EventKind::Unsure {
                name: name.clone(),
                id: stored.id,
                reason: events::cut_reason(&reason),
            };
            append_message_event(run, unsure, now)?;
            Ok(SendOutcome::Unsure {
                name,
                reason,
                file: stored.path,
            })
        }
        Err(error) => mark_not_sent(run, stored, &error.to_string(), now),
    }
}

/// Presses Enter once when a stall left the text pasted and the user
/// is not in the pane. Returns true when the session then started.
/// Any Herdr failure returns false.
pub(crate) fn submit_pasted_text(herdr: &dyn Herdr, name: &str) -> bool {
    let Ok(screen) = herdr.read_screen(name) else {
        return false;
    };
    if !PASTED_TEXT.is_match(&screen) {
        return false;
    }
    // Enter in a pane the user typed in meanwhile would send their text.
    let Ok(agent) = herdr.get_agent(name) else {
        return false;
    };
    if agent.focused || agent.status == AgentStatus::Blocked {
        return false;
    }
    herdr.press_enter(name).is_ok() && herdr.wait_until_started(name).is_ok()
}

/// Returns where a message to a name that is not live goes. That is
/// the newest live session on the card in its name, which took over
/// after a handoff, else nowhere. A card `card` cannot read counts as
/// having no live session.
pub(crate) fn find_card_successor(cards: &dyn Cards, name: &str, agents: &[Agent]) -> Target {
    let Some(note) = CardKey::find_in_name(name).and_then(|key| cards.read_card(&key).ok()) else {
        return Target::Gone;
    };
    let successor = note.sessions.iter().rev().find_map(|session| {
        agents
            .iter()
            .find(|agent| agent.name != name && agent.session_id.as_deref() == Some(session))
    });
    match successor {
        Some(agent) => Target::HandedOff(agent.name.clone()),
        None => Target::Gone,
    }
}

/// Renames a held file to sending. Returns `None` when another
/// process took it first, so no two processes send one message.
pub(crate) fn claim_message(stored: StoredMessage) -> Result<Option<StoredMessage>, HubError> {
    let sending = state_path(&stored.path, &stored.id, SendState::Sending);
    match fs::rename(&stored.path, &sending) {
        Ok(()) => Ok(Some(StoredMessage {
            path: sending,
            ..stored
        })),
        Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(HubError::File {
            path: stored.path,
            source,
        }),
    }
}

/// Writes a new message whole under its state's suffix, with the next
/// free number in its second. `name` is a checked agent name or one
/// Herdr gave, so it holds no slash or dot.
pub(crate) fn write_message(
    run: &RunFolder,
    name: &str,
    machine: Option<&str>,
    text: &str,
    state: SendState,
    now: &Zoned,
) -> Result<StoredMessage, HubError> {
    let message = Message {
        name: name.to_owned(),
        machine: machine.map(str::to_owned),
        text: text.to_owned(),
        created: now.timestamp(),
        reason: None,
    };
    let stamp = now.strftime("%Y%m%d-%H%M%S").to_string();
    place_message(run, message, &stamp, state)
}

/// Returns held messages oldest first, for one session or for all.
pub(crate) fn list_messages(
    run: &RunFolder,
    name: Option<&str>,
) -> Result<Vec<StoredMessage>, HubError> {
    let sends = run.path().join("sends");
    let entries = match fs::read_dir(&sends) {
        Ok(entries) => entries,
        Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(source) => {
            return Err(HubError::File {
                path: sends,
                source,
            });
        }
    };
    let mut found = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|source| HubError::File {
            path: sends.clone(),
            source,
        })?;
        let file_name = entry.file_name().to_string_lossy().into_owned();
        let Some(id) = file_name.strip_suffix(".held") else {
            continue;
        };
        let Some((owner, stamp, number)) = split_message_id(id) else {
            continue;
        };
        if name.is_some_and(|name| name != owner) {
            continue;
        }
        let path = entry.path();
        let message = read_message_file(&path)?;
        let order = (stamp.to_owned(), number, owner.to_owned());
        let stored = StoredMessage {
            id: MessageId(id.to_owned()),
            message,
            path,
        };
        found.push((order, stored));
    }
    found.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(found.into_iter().map(|(_, stored)| stored).collect())
}

/// Counts each name's message files by state.
pub(crate) fn count_messages(run: &RunFolder) -> Result<BTreeMap<String, MessageCounts>, HubError> {
    let sends = run.path().join("sends");
    let entries = match fs::read_dir(&sends) {
        Ok(entries) => entries,
        Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(BTreeMap::new()),
        Err(source) => {
            return Err(HubError::File {
                path: sends,
                source,
            });
        }
    };
    let mut counts: BTreeMap<String, MessageCounts> = BTreeMap::new();
    for entry in entries {
        let entry = entry.map_err(|source| HubError::File {
            path: sends.clone(),
            source,
        })?;
        let file_name = entry.file_name().to_string_lossy().into_owned();
        let Some((id, suffix)) = file_name.rsplit_once('.') else {
            continue;
        };
        // A name starting with a dot is a temporary file, mid-write.
        let Some((name, _, _)) = split_message_id(id).filter(|_| !id.starts_with('.')) else {
            continue;
        };
        let count = counts.entry(name.to_owned()).or_default();
        match suffix {
            "held" => count.held += 1,
            "sending" => count.unsure += 1,
            "notsent" => count.not_sent += 1,
            _ => {}
        }
    }
    Ok(counts)
}

/// Claims every message held for NAME, records it as delivered through
/// `hub inbox`, then deletes its file. A failed record renames the file
/// back to held. Returns the messages oldest first.
pub(crate) fn take_held_messages(
    run: &RunFolder,
    name: &str,
    now: &Zoned,
) -> Result<Vec<Message>, HubError> {
    let mut taken = Vec::new();
    for stored in list_messages(run, Some(name))? {
        // A message `hub next` claimed meanwhile has nothing left here to take.
        let Some(claimed) = claim_message(stored)? else {
            continue;
        };
        let delivered = EventKind::Delivered {
            name: name.to_owned(),
            id: claimed.id.clone(),
            via: Via::Inbox,
        };
        // Deleting the file first would lose the text if the event write failed.
        if let Err(error) = append_message_event(run, delivered, now) {
            let held = state_path(&claimed.path, &claimed.id, SendState::Held);
            let _ = fs::rename(&claimed.path, held);
            return Err(error);
        }
        fs::remove_file(&claimed.path).map_err(|source| HubError::File {
            path: claimed.path.clone(),
            source,
        })?;
        taken.push(claimed.message);
    }
    Ok(taken)
}

/// Gives a claimed message to the session that took over its card, as
/// a held message under that name. The old file goes last, so a crash
/// leaves it as `.sending` and the text stays on disk.
fn move_message(
    run: &RunFolder,
    stored: StoredMessage,
    to: &str,
    now: &Zoned,
) -> Result<(), HubError> {
    let stamp = split_message_id(&stored.id.0)
        .map(|(_, stamp, _)| stamp.to_owned())
        .unwrap_or_else(|| now.strftime("%Y%m%d-%H%M%S").to_string());
    let old_name = stored.message.name.clone();
    let message = Message {
        name: to.to_owned(),
        ..stored.message
    };
    place_message(run, message, &stamp, SendState::Held)?;
    let moved = EventKind::Moved {
        name: old_name,
        id: stored.id,
        to: to.to_owned(),
    };
    append_message_event(run, moved, now)?;
    fs::remove_file(&stored.path).map_err(|source| HubError::File {
        path: stored.path,
        source,
    })
}

/// Renames a claimed message to `.notsent`, writes the reason into it
/// and records it. The text stays so the hub can send it again.
fn mark_not_sent(
    run: &RunFolder,
    stored: StoredMessage,
    reason: &str,
    now: &Zoned,
) -> Result<SendOutcome, HubError> {
    let not_sent = state_path(&stored.path, &stored.id, SendState::NotSent);
    fs::rename(&stored.path, &not_sent).map_err(|source| HubError::File {
        path: stored.path.clone(),
        source,
    })?;
    let message = Message {
        reason: Some(reason.to_owned()),
        ..stored.message
    };
    run_folder::write_file_whole(&not_sent, &to_json(&message, &not_sent)?)?;
    let name = message.name;
    let event = EventKind::NotSent {
        name: name.clone(),
        id: stored.id,
        reason: events::cut_reason(reason),
    };
    append_message_event(run, event, now)?;
    Ok(SendOutcome::NotSent {
        name,
        reason: reason.to_owned(),
    })
}

/// Writes a message under the first free number in its stamp's second
/// and returns it.
fn place_message(
    run: &RunFolder,
    message: Message,
    stamp: &str,
    state: SendState,
) -> Result<StoredMessage, HubError> {
    let sends = run.path().join("sends");
    fs::create_dir_all(&sends).map_err(|source| HubError::File {
        path: sends.clone(),
        source,
    })?;
    // The dot keeps the temporary file out of `list_messages`.
    let temporary = sends.join(format!(".{}.{}.tmp", message.name, std::process::id()));
    fs::write(&temporary, to_json(&message, &temporary)?).map_err(|source| HubError::File {
        path: temporary.clone(),
        source,
    })?;
    let mut number = 1;
    let placed = loop {
        let id = MessageId(format!("{}.{stamp}.{number}", message.name));
        let path = sends.join(format!("{}.{}", id.0, state.suffix()));
        let taken = [SendState::Held, SendState::Sending, SendState::NotSent]
            .into_iter()
            .any(|other| state_path(&path, &id, other).exists());
        if taken {
            number += 1;
            continue;
        }
        // A hard link fails when the name exists, so another message
        // written in the same second is never replaced.
        match fs::hard_link(&temporary, &path) {
            Ok(()) => break Ok(StoredMessage { id, message, path }),
            Err(source) if source.kind() == io::ErrorKind::AlreadyExists => number += 1,
            Err(source) => break Err(HubError::File { path, source }),
        }
    };
    let _ = fs::remove_file(&temporary);
    placed
}

/// Returns the path of the same message in another state.
fn state_path(path: &Path, id: &MessageId, state: SendState) -> PathBuf {
    path.with_file_name(format!("{}.{}", id.0, state.suffix()))
}

/// Splits `docs-fix.20261110-221502.1` into the name, the stamp and
/// the number, or returns `None` for a file this hub did not write.
fn split_message_id(id: &str) -> Option<(&str, &str, u32)> {
    let mut parts = id.splitn(3, '.');
    let name = parts.next()?;
    let stamp = parts.next()?;
    let number = parts.next()?.parse().ok()?;
    Some((name, stamp, number))
}

/// Reads one message file.
fn read_message_file(path: &Path) -> Result<Message, HubError> {
    let file_error = |source: io::Error| HubError::File {
        path: path.to_owned(),
        source,
    };
    let text = fs::read_to_string(path).map_err(file_error)?;
    serde_json::from_str(&text).map_err(|error| file_error(error.into()))
}

/// Returns a message as the JSON its file holds.
fn to_json(message: &Message, path: &Path) -> Result<String, HubError> {
    serde_json::to_string(message).map_err(|error| HubError::File {
        path: path.to_owned(),
        source: error.into(),
    })
}

/// Appends one message event stamped `now`.
fn append_message_event(run: &RunFolder, kind: EventKind, now: &Zoned) -> Result<(), HubError> {
    let event = Event {
        at: now.timestamp(),
        kind,
    };
    events::append_event(run, &event)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::test_values::make_agent;

    /// These rules decide whether a message types into a pane now. Typing
    /// into a blocked pane or under the user's own typing breaks their
    /// work, and holding a working worker's message until after it hands
    /// off loses it, as happened before this rule.
    #[test]
    fn choose_delivery_follows_state_kind_and_queue() {
        struct Case {
            name: &'static str,
            target: Target,
            older_waiting: usize,
            expected: Delivery,
        }
        let codex = |status| Agent {
            kind: AgentKind::Other("codex".to_owned()),
            ..make_agent("docs-fix", status)
        };
        let cases = [
            Case {
                name: "opencode-working",
                target: Target::Live(make_agent("docs-fix", AgentStatus::Working)),
                older_waiting: 0,
                expected: Delivery::Now,
            },
            Case {
                name: "claude-idle",
                target: Target::Live(Agent {
                    kind: AgentKind::Claude,
                    ..make_agent("docs-fix", AgentStatus::Idle)
                }),
                older_waiting: 0,
                expected: Delivery::Now,
            },
            Case {
                name: "opencode-done",
                target: Target::Live(make_agent("docs-fix", AgentStatus::Done)),
                older_waiting: 0,
                expected: Delivery::Now,
            },
            Case {
                name: "blocked",
                target: Target::Live(make_agent("docs-fix", AgentStatus::Blocked)),
                older_waiting: 0,
                expected: Delivery::Hold(HoldReason::Blocked),
            },
            Case {
                name: "focused",
                target: Target::Live(Agent {
                    focused: true,
                    ..make_agent("docs-fix", AgentStatus::Idle)
                }),
                older_waiting: 0,
                expected: Delivery::Hold(HoldReason::UserInPane),
            },
            Case {
                name: "codex-working",
                target: Target::Live(codex(AgentStatus::Working)),
                older_waiting: 0,
                expected: Delivery::Hold(HoldReason::BusyOtherKind("codex".to_owned())),
            },
            Case {
                name: "codex-idle",
                target: Target::Live(codex(AgentStatus::Idle)),
                older_waiting: 0,
                expected: Delivery::Now,
            },
            Case {
                name: "unknown",
                target: Target::Live(make_agent("docs-fix", AgentStatus::Unknown)),
                older_waiting: 0,
                expected: Delivery::Hold(HoldReason::StateUnknown),
            },
            Case {
                name: "older-waiting",
                target: Target::Live(make_agent("docs-fix", AgentStatus::Idle)),
                older_waiting: 1,
                expected: Delivery::Hold(HoldReason::OlderWaits(1)),
            },
            Case {
                name: "handed-off",
                target: Target::HandedOff("docs-fix-2".to_owned()),
                older_waiting: 0,
                expected: Delivery::MoveTo("docs-fix-2".to_owned()),
            },
            Case {
                name: "gone",
                target: Target::Gone,
                older_waiting: 0,
                expected: Delivery::Gone,
            },
        ];

        for case in cases {
            assert_eq!(
                choose_delivery(&case.target, case.older_waiting),
                case.expected,
                "{}",
                case.name
            );
        }
    }

    /// `hub status` holds a session at `hub` while a message to it is
    /// held, unsure or not sent, so the tidy pass never closes a tab a
    /// message still waits for. A suffix counted as another state shows
    /// the wrong word. A file whose name starts with a dot is not a
    /// message, and counting it would hold the session at `hub` for
    /// nothing.
    #[test]
    fn count_messages_counts_states_not_temporary_files() {
        let folder =
            std::env::temp_dir().join(format!("hub-count-messages-{}", std::process::id()));
        let sends = folder.join("sends");
        fs::create_dir_all(&sends).unwrap();
        for file_name in [
            "docs-fix.20261111-221400.1.held",
            "docs-fix.20261111-221400.2.sending",
            "docs-fix.20261111-221400.3.notsent",
            ".docs-fix.20261111-221400.4.held",
        ] {
            fs::write(sends.join(file_name), "{}").unwrap();
        }

        let counts = count_messages(&RunFolder::open(&folder).unwrap());
        // Removed before the asserts, so a failure leaves no folder.
        fs::remove_dir_all(&folder).unwrap();

        let expected = MessageCounts {
            held: 1,
            unsure: 1,
            not_sent: 1,
        };
        assert_eq!(
            counts.unwrap(),
            BTreeMap::from([("docs-fix".to_owned(), expected)])
        );
    }
}
