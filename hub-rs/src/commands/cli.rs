//! Reads the command line into a `Command`, and finds the run folder.
//! Read `parse_command` first.

use std::error::Error as _;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use clap::Parser;
use clap::error::ErrorKind;

use crate::external::cards::CardKey;
use crate::shared::run_folder::RunFolder;
use crate::shared::{AgentName, HubError};

// clap turns each `///` line on `Cli`, `Command`, `RunArg` and their
// fields into the help text users see, so those lines are written for
// the user. Notes for maintainers use `//`.

/// Watch, message and tidy a hub's sessions.
#[derive(Parser)]
#[command(
    name = "hub",
    // `build.rs` sets this from `HUB_VERSION` or the newest `hub-v` tag.
    version = env!("HUB_BUILD_VERSION"),
    override_usage = "hub [--machine LABEL] COMMAND ...",
    // clap cannot mark the commands that take a run folder, so this
    // list is written by hand. Keep it in step with `Command`.
    help_template = "\
usage: {usage}

{about-with-newline}Commands marked * take --run DIR, else
$HUB_RUN.

  status*  each session and its state
  next*    wait for news, print one line
  send*    send now, or hold until free
  read*    print and archive a report
  report*  write your report (workers)
  inbox*   print held messages (workers)
  note*    add a timed line to notes.md
  brief*   what waits on the user
  handoff* start the next hub
  clean*   close and park finished tabs
  park*    park one finished session
  auto     the steps of an auto run
  flag     mark a pane \"needs you\"
  unflag   clear the mark
  alive    is an OpenCode session alive
  go       show the next hub
  gather   put hubs at the top

{after-help}",
    after_help = "Exit: 0 done, 1 a tool failed,\n2 wrong usage, 3 not done."
)]
pub struct Cli {
    /// Send Herdr calls to the SSH machine saved in Herdr as LABEL
    #[arg(long, value_name = "LABEL")]
    pub machine: Option<String>,
    #[command(subcommand)]
    pub command: Command,
}

// The run folder, shared by every command that takes one. `None`
// falls back to `HUB_RUN` in `find_run_folder`.
#[derive(clap::Args)]
pub struct RunArg {
    /// The hub's run folder, else $HUB_RUN
    #[arg(long, value_name = "DIR")]
    pub run: Option<PathBuf>,
}

#[derive(clap::Subcommand)]
pub enum Command {
    /// Each session and its state
    #[command(after_help = "Example: hub status --json")]
    Status {
        #[command(flatten)]
        run: RunArg,
        /// Print the same facts as JSON
        #[arg(long)]
        json: bool,
    },
    /// Wait for news, print one line
    #[command(after_help = "Example: hub next --timeout 600")]
    Next {
        #[command(flatten)]
        run: RunArg,
        /// Give up after this many seconds
        #[arg(long, value_name = "SECONDS")]
        timeout: Option<f64>,
        /// Print the news as JSON
        #[arg(long)]
        json: bool,
    },
    /// Deliver now, or hold until free
    #[command(after_help = "Example: hub send docs-fix --file /tmp/msg.md")]
    Send {
        #[command(flatten)]
        run: RunArg,
        /// The session's agent name
        name: AgentName,
        /// The message text
        #[arg(required_unless_present = "file", conflicts_with = "file")]
        message: Option<String>,
        /// Read the message from this file
        #[arg(long, value_name = "PATH")]
        file: Option<PathBuf>,
    },
    /// Print and archive a report
    #[command(after_help = "Example: hub read docs-fix --note \"merged #9\"")]
    Read {
        #[command(flatten)]
        run: RunArg,
        /// The session's agent name
        name: AgentName,
        /// Add this line to notes.md
        #[arg(long)]
        note: Option<String>,
        /// Also add the note to the card, from the name or this key
        // `None` without `--card`, `Some(None)` for `--card` alone.
        #[arg(long, value_name = "KEY", requires = "note")]
        card: Option<Option<CardKey>>,
    },
    /// Write your report (workers)
    #[command(after_help = "Example: hub report docs-fix /tmp/report.md")]
    Report {
        #[command(flatten)]
        run: RunArg,
        /// Your agent name
        name: AgentName,
        /// The report file, or - for standard input
        // `None` reads standard input too.
        file: Option<PathBuf>,
    },
    /// Print held messages (workers)
    #[command(after_help = "Example: hub inbox docs-fix")]
    Inbox {
        #[command(flatten)]
        run: RunArg,
        /// Your agent name
        name: AgentName,
    },
    /// Add a timed line to notes.md
    #[command(after_help = "Example: hub note \"merged #9\"")]
    Note {
        #[command(flatten)]
        run: RunArg,
        /// The line to add
        text: String,
    },
    /// What waits on the user
    #[command(after_help = "Example: hub brief")]
    Brief {
        #[command(flatten)]
        run: RunArg,
    },
    /// Start the next hub
    #[command(after_help = "Example: hub handoff kanban-hub-14 /tmp/hub-14.md")]
    Handoff {
        #[command(flatten)]
        run: RunArg,
        /// The next hub's agent name
        name: AgentName,
        /// The handoff file for the next hub
        handoff: PathBuf,
    },
    /// Close and park finished tabs
    #[command(after_help = "Example: hub clean --dry-run")]
    Clean {
        #[command(flatten)]
        run: RunArg,
        /// Print what it would do, and change nothing
        #[arg(long)]
        dry_run: bool,
    },
    /// Close a finished session, keeping its ID
    #[command(after_help = "Example: hub park docs-fix")]
    Park {
        #[command(flatten)]
        run: RunArg,
        /// The session's agent name
        name: AgentName,
    },
    /// The steps of an auto run
    #[command(after_help = "Example: hub auto check --run /tmp/auto-run-2")]
    Auto {
        #[command(subcommand)]
        step: AutoStep,
    },
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
    /// Is an OpenCode session alive
    #[command(after_help = "Example: hub alive ses_0a1b2c3d4e5f")]
    Alive {
        /// The OpenCode session ID
        session: String,
    },
    /// Show the next hub
    #[command(after_help = "Example: hub go")]
    Go,
    /// Put hubs at the top
    #[command(after_help = "Example: hub gather")]
    Gather,
}

// The steps run in the order the `auto-run` skill gives.
#[derive(clap::Subcommand)]
pub enum AutoStep {
    /// Write the ready cards to ready.md
    #[command(after_help = "Example: hub auto ready")]
    Ready {
        #[command(flatten)]
        run: RunArg,
    },
    /// Compare run.md against the ready cards
    #[command(after_help = "Example: hub auto check")]
    Check {
        #[command(flatten)]
        run: RunArg,
    },
    /// Save run.md with the user's reply
    #[command(after_help = "Example: hub auto approve 2 \"approve run 2\"")]
    Approve {
        #[command(flatten)]
        run: RunArg,
        /// The reply is the user's own words naming exactly the run's
        /// cards, not `approve run N`
        #[arg(long)]
        given: bool,
        /// The run's number
        number: u32,
        /// The user's reply, word for word
        reply: String,
    },
    /// Write and print brief-now.md
    #[command(after_help = "Example: hub auto now")]
    Now {
        #[command(flatten)]
        run: RunArg,
    },
}

impl Command {
    /// Returns the name the user typed, such as `go`.
    pub fn name(&self) -> &'static str {
        match self {
            Command::Status { .. } => "status",
            Command::Next { .. } => "next",
            Command::Send { .. } => "send",
            Command::Read { .. } => "read",
            Command::Report { .. } => "report",
            Command::Inbox { .. } => "inbox",
            Command::Note { .. } => "note",
            Command::Brief { .. } => "brief",
            Command::Handoff { .. } => "handoff",
            Command::Clean { .. } => "clean",
            Command::Park { .. } => "park",
            Command::Auto { .. } => "auto",
            Command::Flag { .. } => "flag",
            Command::Unflag { .. } => "unflag",
            Command::Alive { .. } => "alive",
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
/// `HubError::Usage` for a bad agent name or card key, with the
/// message its parser gave, such as `Docs/Fix is not an agent name`.
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

/// Returns the run folder from `--run`, else from `HUB_RUN`.
///
/// # Errors
///
/// `HubError::Usage` when neither is set, or the folder does not
/// exist.
pub fn find_run_folder(
    given: Option<&Path>,
    hub_run: Option<&OsStr>,
) -> Result<RunFolder, HubError> {
    let path = given
        .map(Path::to_path_buf)
        .or_else(|| hub_run.filter(|value| !value.is_empty()).map(PathBuf::from))
        .ok_or_else(|| {
            HubError::Usage("no run folder; pass --run DIR or set HUB_RUN".to_owned())
        })?;
    RunFolder::open(&path)
}

/// Tells whether a command works with `--machine`, which only
/// `status`, `send`, `flag` and `unflag` do. The rest read this
/// machine's files and database, or its own Herdr sidebar.
pub fn takes_machine(command: &Command) -> bool {
    match command {
        Command::Status { .. }
        | Command::Send { .. }
        | Command::Flag { .. }
        | Command::Unflag { .. } => true,
        Command::Next { .. }
        | Command::Read { .. }
        | Command::Report { .. }
        | Command::Inbox { .. }
        | Command::Note { .. }
        | Command::Brief { .. }
        | Command::Handoff { .. }
        | Command::Clean { .. }
        | Command::Park { .. }
        | Command::Auto { .. }
        | Command::Alive { .. }
        | Command::Go
        | Command::Gather => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A remote hub lists, flags and messages sessions on another
    /// machine with `--machine`, so refusing it there leaves those
    /// sessions unseen, unmarked or without their message. Allowing it
    /// on `go` would make `go` jump, in this machine's sidebar, to a hub
    /// found on another machine, and on `alive` would read this
    /// machine's database for another machine's session. `clean` and
    /// `park` check this machine's worktrees with git. With `--machine`,
    /// they would read this machine's git facts but close tabs on
    /// another machine.
    #[test]
    fn takes_machine_refuses_local_only_commands() {
        let name: AgentName = "docs-fix".parse().unwrap();
        let cases = [
            (
                "status",
                Command::Status {
                    run: RunArg { run: None },
                    json: false,
                },
                true,
            ),
            (
                "send",
                Command::Send {
                    run: RunArg { run: None },
                    name: name.clone(),
                    message: Some("Rebase on main.".to_owned()),
                    file: None,
                },
                true,
            ),
            ("flag", Command::Flag { name: name.clone() }, true),
            ("unflag", Command::Unflag { name: name.clone() }, true),
            (
                "alive",
                Command::Alive {
                    session: "ses_0a1b2c3d4e5f".to_owned(),
                },
                false,
            ),
            (
                "clean",
                Command::Clean {
                    run: RunArg { run: None },
                    dry_run: false,
                },
                false,
            ),
            (
                "park",
                Command::Park {
                    run: RunArg { run: None },
                    name: name.clone(),
                },
                false,
            ),
            ("go", Command::Go, false),
            ("gather", Command::Gather, false),
        ];

        for (case_name, command, expected) in cases {
            assert_eq!(takes_machine(&command), expected, "{case_name}");
        }
    }
}
