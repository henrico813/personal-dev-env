//! This module reads the command line and prints each reply as JSON. The exit
//! code is 0 for no error, 1 for an error code, and 2 for bad arguments,
//! because scripts read success from it. Read the `parse_command(args)` and
//! `print_reply(reply)` functions to understand the module.

use std::ffi::OsString;
use std::fmt;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::Parser;
use serde::Deserialize;
use serde::de::DeserializeOwned;

use super::{Reply, list_commands};
use crate::shared::review_session::FeedbackRequestList;
use crate::shared::{Error, ErrorCode};
use crate::steps::make_feedback_request_list::ChangeRequest;

/// Names what the command line asks for, with its arguments. The step commands
/// work on the current folder.
#[derive(Debug, Parser)]
#[command(
    name = "pde-comment-review-next",
    disable_help_flag = true,
    disable_help_subcommand = true
)]
pub enum Command {
    CheckComments {
        /// Names the task start, a commit or branch.
        #[arg(long)]
        base: Option<String>,
    },
    MakeFeedbackRequestList {
        #[arg(long)]
        base: Option<String>,
        /// Holds the JSON file of change requests, because a name can hold
        /// `::` or spaces.
        #[arg(long = "input")]
        input_file: Option<PathBuf>,
    },
    SendAgentSuggestions {
        /// Holds the Feedback Request List with its Agent Suggestions.
        file: PathBuf,
    },
    ShowStatus,
    Mcp,
    /// Needs no Git checkout, Herdr or saved review, because the installer
    /// runs it to check each install.
    Help,
}

/// Holds clap's message for bad arguments, with the usage text.
#[derive(Debug)]
pub struct UsageError(String);

impl fmt::Display for UsageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Reads the command line, whose first item is the program's name. On a
/// `UsageError`, `main` exits with 2.
pub fn parse_command(args: &[OsString]) -> Result<Command, UsageError> {
    Command::try_parse_from(args).map_err(|error| UsageError(error.render().to_string()))
}

/// Prints the reply as JSON. Returns exit code 1 when the reply holds an error
/// or cannot be printed, and 0 otherwise.
pub fn print_reply(reply: &Reply) -> ExitCode {
    let printed = serde_json::to_string_pretty(reply)
        .map_err(io::Error::from)
        .and_then(|text| writeln!(io::stdout().lock(), "{text}"));
    if let Err(error) = printed {
        eprintln!("Cannot print the reply. {error}");
        return ExitCode::FAILURE;
    }
    if reply.error.is_some() {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

/// Prints each command with its description, from `list_commands`.
pub fn print_help() -> ExitCode {
    let mut help = String::from("Usage: pde-comment-review-next COMMAND\n");
    for command in list_commands() {
        help.push_str(&format!(
            "\n{}\n    {}\n",
            command.cli_usage, command.description
        ));
    }
    match io::stdout().lock().write_all(help.as_bytes()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("Cannot print help. {error}");
            ExitCode::FAILURE
        }
    }
}

/// Returns the change requests in the `--input` file, or none without one.
pub fn read_change_requests(input_file: Option<&Path>) -> Result<Vec<ChangeRequest>, Error> {
    /// Holds the input file, which has only change requests.
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct InputFile {
        change_requests: Vec<ChangeRequest>,
    }

    match input_file {
        Some(input_file) => Ok(read_json_file::<InputFile>(input_file)?.change_requests),
        None => Ok(Vec::new()),
    }
}

/// Returns the Feedback Request List in `file`.
pub fn read_list_file(file: &Path) -> Result<FeedbackRequestList, Error> {
    read_json_file(file)
}

/// Reads `file` as JSON, refusing a missing or malformed file with `BadInput`.
fn read_json_file<T: DeserializeOwned>(file: &Path) -> Result<T, Error> {
    let bad_input = |error: &dyn fmt::Display| Error {
        code: ErrorCode::BadInput,
        message: format!("Cannot read {} as the input. {error}", file.display()),
    };
    let text = fs::read(file).map_err(|error| bad_input(&error))?;
    serde_json::from_slice(&text).map_err(|error| bad_input(&error))
}
