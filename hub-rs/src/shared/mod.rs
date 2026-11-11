//! Holds what every command shares. `HubError` is the one error type
//! a command returns, `Tools` is the bundle of outside systems, and
//! `AgentName` is a checked session name.

use std::fmt;
use std::str::FromStr;

use crate::outside::herdr::{Herdr, HerdrError};

/// A command that stopped, with the exit code `main` gives it. Tests
/// match the variant, never the message.
#[derive(Debug)]
pub enum HubError {
    /// Wrong usage clap does not report itself, such as a bad agent name
    /// or `--machine` on a command that works on this machine only.
    /// Exit 2.
    Usage(String),
    /// The command did not do its work, for the reason given. Exit 3.
    NotDone(String),
    /// Exit 1.
    Herdr(HerdrError),
}

impl HubError {
    /// Returns 1 for an outside failure, 2 for wrong usage and 3 for
    /// work not done.
    pub fn exit_code(&self) -> u8 {
        match self {
            HubError::Herdr(_) => 1,
            HubError::Usage(_) => 2,
            HubError::NotDone(_) => 3,
        }
    }
}

/// Writes the message `main` prints after `hub: `.
impl fmt::Display for HubError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HubError::Usage(message) | HubError::NotDone(message) => formatter.write_str(message),
            HubError::Herdr(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for HubError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            HubError::Herdr(error) => Some(error),
            HubError::Usage(_) | HubError::NotDone(_) => None,
        }
    }
}

impl From<HerdrError> for HubError {
    fn from(error: HerdrError) -> HubError {
        HubError::Herdr(error)
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

/// The outside systems one command may call, built once by `main`.
/// Tests build the doubles first and lend them here, so they can read
/// what each double recorded after the call.
pub struct Tools<'a> {
    pub herdr: &'a dyn Herdr,
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
