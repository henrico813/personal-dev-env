//! Reads the command line into a `Command`. Read `parse_command`
//! first.

use std::error::Error as _;

use clap::Parser;
use clap::error::ErrorKind;

use crate::shared::{AgentName, HubError};

// clap turns each `///` line on `Cli`, `Command` and their fields into
// the help text users see, so those lines are written for the user.
// Notes for maintainers use `//`.

/// Watch, message and tidy a hub's sessions.
#[derive(Parser)]
#[command(
    name = "hub",
    version,
    override_usage = "hub [--machine LABEL] COMMAND ...",
    after_help = "Exit: 0 done, 1 a tool failed,\n2 wrong usage, 3 not done."
)]
pub struct Cli {
    /// Send Herdr calls to the SSH machine saved in Herdr as LABEL
    #[arg(long, value_name = "LABEL")]
    pub machine: Option<String>,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(clap::Subcommand)]
pub enum Command {
    /// Mark a pane "needs you"
    #[command(after_help = "Example: hub flag docs-fix")]
    Flag {
        /// The session's agent name
        name: AgentName,
    },
    /// Clear the mark
    #[command(after_help = "Example: hub unflag docs-fix")]
    Unflag {
        /// The session's agent name
        name: AgentName,
    },
    /// Show the next hub
    #[command(after_help = "Example: hub go")]
    Go,
    /// Put hubs at the top
    #[command(after_help = "Example: hub gather")]
    Gather,
}

impl Command {
    /// Returns the name the user typed, such as `go`.
    pub fn name(&self) -> &'static str {
        match self {
            Command::Flag { .. } => "flag",
            Command::Unflag { .. } => "unflag",
            Command::Go => "go",
            Command::Gather => "gather",
        }
    }
}

/// Parses the arguments after `hub`. clap prints its own message and
/// exits for help, the version and bad flags.
///
/// # Errors
///
/// `HubError::Usage` for a bad agent name, with the message
/// `AgentName` gave, such as `Docs/Fix is not an agent name`.
pub fn parse_command() -> Result<Cli, HubError> {
    Cli::try_parse().map_err(|error| match error.kind() {
        // clap keeps the `from_str` error as the source.
        ErrorKind::ValueValidation => HubError::Usage(
            error
                .source()
                .map_or_else(|| error.to_string(), ToString::to_string),
        ),
        _ => error.exit(),
    })
}

/// Tells whether a command works with `--machine`. `go` and `gather`
/// do not, because each Herdr server keeps its own sidebar.
pub fn takes_machine(command: &Command) -> bool {
    match command {
        Command::Flag { .. } | Command::Unflag { .. } => true,
        Command::Go | Command::Gather => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A remote hub flags sessions on another machine with `--machine`,
    /// so refusing it there leaves those sessions unmarked. Allowing it
    /// on `go` would make `go` jump, in this machine's sidebar, to a hub
    /// found on another machine.
    #[test]
    fn takes_machine_refuses_local_only_commands() {
        let name: AgentName = "docs-fix".parse().unwrap();
        let cases = [
            ("flag", Command::Flag { name: name.clone() }, true),
            ("unflag", Command::Unflag { name: name.clone() }, true),
            ("go", Command::Go, false),
            ("gather", Command::Gather, false),
        ];

        for (case_name, command, expected) in cases {
            assert_eq!(takes_machine(&command), expected, "{case_name}");
        }
    }
}
