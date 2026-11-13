//! Holds what every command shares. `HubError` is the one error type
//! a command returns, `Tools` is the bundle of external systems, and
//! `AgentName` is a checked session name.

pub(crate) mod context;
pub(crate) mod decisions;
pub(crate) mod events;
pub(crate) mod facts;
pub(crate) mod lifecycle;
pub(crate) mod messages;
pub mod parking;
pub mod run_folder;
#[cfg(test)]
pub(crate) mod test_values;
pub(crate) mod tidy;

use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::Duration;

use crate::external::cards::{CardCli, CardError, Cards};
use crate::external::herdr::{Herdr, HerdrCli, HerdrError};
use crate::external::opencode::{OpenCodeDb, OpenCodeError};
use crate::external::requests::RequestFolder;
use parking::ParkedFolder;

/// A command that stopped, with the exit code `main` gives it. Tests
/// match the variant, never the message.
#[derive(Debug)]
pub enum HubError {
    /// Wrong usage clap does not report itself, such as a bad agent name,
    /// no run folder, or `--machine` on a command that works on this
    /// machine only. Exit 2.
    Usage(String),
    /// The command did not do its work, for the reason given. Exit 3.
    NotDone(String),
    /// A message was not sent, because Herdr turned it down, the
    /// session is gone, or a reopen of it failed. Exit 1.
    NotSent { name: String, reason: String },
    /// A message may have arrived. `file` is its `.sending` file.
    /// Exit 1.
    Unsure {
        name: String,
        reason: String,
        file: PathBuf,
    },
    /// Exit 1.
    Herdr(HerdrError),
    /// Exit 1.
    Card(CardError),
    /// Exit 1.
    OpenCode(OpenCodeError),
    /// A run folder or state file the hub could not read or write.
    /// Exit 1.
    File { path: PathBuf, source: io::Error },
    /// A step that failed after `done` already happened, such as
    /// `archived report-docs-fix.2214.read`. It exits with the failed
    /// step's code.
    PartlyDone { done: String, error: Box<HubError> },
}

impl HubError {
    /// Returns 1 for an external failure, 2 for wrong usage and 3 for
    /// work not done.
    pub fn exit_code(&self) -> u8 {
        match self {
            HubError::NotSent { .. }
            | HubError::Unsure { .. }
            | HubError::Herdr(_)
            | HubError::Card(_)
            | HubError::OpenCode(_)
            | HubError::File { .. } => 1,
            HubError::Usage(_) => 2,
            HubError::NotDone(_) => 3,
            HubError::PartlyDone { error, .. } => error.exit_code(),
        }
    }
}

/// Writes the message `main` prints after `hub: `.
impl fmt::Display for HubError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HubError::Usage(message) | HubError::NotDone(message) => formatter.write_str(message),
            HubError::NotSent { name, reason } => write!(formatter, "not sent {name}: {reason}"),
            HubError::Unsure { name, reason, file } => {
                let file_name = file.file_name().unwrap_or_default().to_string_lossy();
                write!(formatter, "unsure {name}: {reason}; see sends/{file_name}")
            }
            HubError::Herdr(error) => write!(formatter, "{error}"),
            HubError::Card(error) => write!(formatter, "{error}"),
            HubError::OpenCode(error) => write!(formatter, "{error}"),
            HubError::File { path, source } => write!(formatter, "{}: {source}", path.display()),
            HubError::PartlyDone { done, error } => write!(formatter, "{done}, but {error}"),
        }
    }
}

/// Returns the error behind a failure, when there is one.
impl std::error::Error for HubError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            HubError::Herdr(error) => Some(error),
            HubError::Card(error) => Some(error),
            HubError::OpenCode(error) => Some(error),
            HubError::File { source, .. } => Some(source),
            HubError::PartlyDone { error, .. } => Some(error.as_ref()),
            HubError::Usage(_)
            | HubError::NotDone(_)
            | HubError::NotSent { .. }
            | HubError::Unsure { .. } => None,
        }
    }
}

impl From<HerdrError> for HubError {
    fn from(error: HerdrError) -> HubError {
        HubError::Herdr(error)
    }
}

impl From<CardError> for HubError {
    fn from(error: CardError) -> HubError {
        HubError::Card(error)
    }
}

impl From<OpenCodeError> for HubError {
    fn from(error: OpenCodeError) -> HubError {
        HubError::OpenCode(error)
    }
}

/// A session name that matches Herdr's agent name rule,
/// `[a-z][a-z0-9_-]{0,31}`. File names built from it cannot hold a
/// slash or a dot.
#[derive(Clone, Debug)]
pub struct AgentName(String);

/// Checks the rule. `parse_command` turns a bad name into
/// `HubError::Usage` with this message.
impl FromStr for AgentName {
    type Err = String;

    fn from_str(text: &str) -> Result<AgentName, String> {
        let mut chars = text.chars();
        let starts_with_letter = chars.next().is_some_and(|first| first.is_ascii_lowercase());
        let rest_allowed = chars.all(|next| {
            next.is_ascii_lowercase() || next.is_ascii_digit() || next == '_' || next == '-'
        });
        if starts_with_letter && rest_allowed && text.len() <= 32 {
            Ok(AgentName(text.to_owned()))
        } else {
            Err(format!("{text} is not an agent name"))
        }
    }
}

impl AgentName {
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

/// The external systems one command may call, built once by `main`.
/// Tests build the doubles first and lend them here, so they can read
/// what each double recorded after the call.
pub struct Tools<'a> {
    pub herdr: &'a dyn Herdr,
    pub cards: &'a dyn Cards,
    pub opencode: &'a OpenCodeDb,
    pub requests: &'a RequestFolder,
    pub parked: &'a ParkedFolder,
    /// This hub's own pane, from `HERDR_PANE_ID`, or `None` outside
    /// Herdr. Nothing closes it, and it is never a status row.
    pub own_pane: Option<&'a str>,
    /// This project's state folder, `~/.local/state/pde`, which holds
    /// the handoffs and session reviews.
    pub state_folder: &'a Path,
    /// The wait between reads of a reopened agent's session ID, one
    /// second outside tests. Herdr learns the ID only after OpenCode
    /// starts.
    pub reopen_poll: Duration,
}

/// The external systems a `Tools` lends, owned. `hub board` moves them
/// into the thread that reads the hub's state, since a new thread
/// cannot borrow from the one that started it.
pub struct OwnedTools {
    pub herdr: HerdrCli,
    pub cards: CardCli,
    pub opencode: OpenCodeDb,
    pub requests: RequestFolder,
    pub parked: ParkedFolder,
    pub own_pane: Option<String>,
    pub state_folder: PathBuf,
    pub reopen_poll: Duration,
}

impl OwnedTools {
    /// Returns a `Tools` that borrows each system from `self`.
    pub fn lend(&self) -> Tools<'_> {
        Tools {
            herdr: &self.herdr,
            cards: &self.cards,
            opencode: &self.opencode,
            requests: &self.requests,
            parked: &self.parked,
            own_pane: self.own_pane.as_deref(),
            state_folder: &self.state_folder,
            reopen_poll: self.reopen_poll,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Report and message file names are built from session names. A
    /// slash or dot let through would write outside the run folder or
    /// split a message file name in the wrong place.
    #[test]
    fn agent_name_accepts_only_herdr_agent_names() {
        let cases = [
            ("slash", "docs/fix", false),
            ("dot", "docs.fix", false),
            ("empty", "", false),
            ("upper-case", "Docs-fix", false),
            ("33-characters", "a23456789012345678901234567890123", false),
            ("32-characters", "a2345678901234567890123456789012", true),
            ("valid", "docs-fix_2", true),
        ];

        for (case_name, text, expected) in cases {
            assert_eq!(text.parse::<AgentName>().is_ok(), expected, "{case_name}");
        }
    }
}
