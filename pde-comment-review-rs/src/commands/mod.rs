//! This module builds the `{status, result, error, next}` reply that the
//! command line and MCP both give, from a step's plain Rust result. Read the
//! `Reply::from_outcome(outcome)` and `list_commands()` functions to
//! understand the module.

use std::path::Path;

use serde::Serialize;
use serde_json::Value;

use crate::shared::git::{self, CommitId};
use crate::shared::review_session::ReviewStatus;
use crate::shared::{Error, ErrorCode};
use crate::steps::{NextStep, StepOutput};

pub mod cli;
pub mod mcp;
pub mod move_old_approvals;
mod old_reviews;

/// Holds the reply every MCP tool and CLI command gives.
#[derive(Debug, Serialize)]
pub struct Reply {
    /// Holds `None` when no Review Session exists or can be read, or the call
    /// was refused.
    pub status: Option<ReviewStatus>,
    pub result: Option<Value>,
    pub error: Option<Error>,
    /// Holds `None` after a refusal that only its message helps with.
    pub next: Option<NextStep>,
}

impl Reply {
    /// Returns the reply to a step's outcome, for MCP and the CLI. A refusal
    /// takes its `next` from its error code.
    pub fn from_outcome<T: Serialize>(outcome: Result<StepOutput<T>, Error>) -> Reply {
        let output = match outcome {
            Ok(output) => output,
            Err(error) => return Reply::from_error(error),
        };
        match serde_json::to_value(output.result) {
            Ok(result) => Reply {
                status: output.status,
                result: Some(result),
                error: None,
                next: Some(output.next),
            },
            // JSON holds only UTF-8, so a file name that is not UTF-8 cannot be sent.
            Err(error) => Reply::from_error(Error {
                code: ErrorCode::FileUnreadable,
                message: format!("Cannot write the result as JSON. {error}"),
            }),
        }
    }

    fn from_error(error: Error) -> Reply {
        Reply {
            status: None,
            result: None,
            next: error.code.next_step(),
            error: Some(error),
        }
    }
}

/// Holds one command's names and its description for `help` and MCP.
#[derive(Debug)]
pub struct CommandInfo {
    /// Shows the command with its arguments, such as `send-agent-suggestions FILE`.
    pub cli_usage: &'static str,
    /// Holds `None` for a command with no MCP tool, such as `help`.
    pub mcp_name: Option<&'static str>,
    pub description: &'static str,
}

/// Lists every command in the order `help` shows them. `help` and the MCP
/// tool list both read it, so their words cannot disagree.
pub fn list_commands() -> &'static [CommandInfo] {
    &[
        CommandInfo {
            cli_usage: "check-comments [--base C]",
            mcp_name: Some("check_comments"),
            description: "Checks the agent's comments (step 2)",
        },
        CommandInfo {
            cli_usage: "make-feedback-request-list [--base C] [--input FILE]",
            mcp_name: Some("make_feedback_request_list"),
            description: "Shows the list, saves nothing (step 3)",
        },
        CommandInfo {
            cli_usage: "send-agent-suggestions FILE",
            mcp_name: Some("send_agent_suggestions"),
            description: "Saves suggestions, sends the list (step 5)",
        },
        CommandInfo {
            cli_usage: "show-status",
            mcp_name: Some("show_status"),
            description: "Shows the status and next step (any time)",
        },
        CommandInfo {
            cli_usage: "screen --popup|--tab",
            mcp_name: None,
            description: "Shows the Review Screen in Herdr",
        },
        CommandInfo {
            cli_usage: "open [NAME]",
            mcp_name: None,
            description: "Shows branch NAME's review in a plain terminal",
        },
        CommandInfo {
            cli_usage: "mcp",
            mcp_name: None,
            description: "Serves the four MCP tools",
        },
        CommandInfo {
            cli_usage: "move-old-approvals",
            mcp_name: None,
            description: "Moves today's approvals and open reviews in, once",
        },
        CommandInfo {
            cli_usage: "help",
            mcp_name: None,
            description: "Prints the commands; runs anywhere",
        },
    ]
}

/// Returns the commit that `base`, such as a branch name or a short hash,
/// names in `dir`'s repository, or `None` when no base is given.
pub fn resolve_base(dir: &Path, base: Option<&str>) -> Result<Option<CommitId>, Error> {
    base.map(|base| git::resolve_commit(dir, base)).transpose()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `Reply::from_outcome` must give a refusal the `next` its error code names.
    /// Otherwise an agent told the code changed would not know to build the list again.
    #[test]
    fn from_outcome_takes_refusal_next_from_code() {
        let refusal = Error {
            code: ErrorCode::ListChanged,
            message: "The code changed. Build the list again.".to_owned(),
        };

        let reply = Reply::from_outcome::<()>(Err(refusal));

        assert_eq!(reply.next, Some(NextStep::MakeFeedbackRequestList));
    }

    /// `Reply::from_outcome` must put the step's status in the reply.
    /// Otherwise an agent that checks the status before removing a worktree would see a waiting review as absent.
    #[test]
    fn from_outcome_keeps_step_status() {
        let output = StepOutput {
            status: Some(ReviewStatus::WaitingForUserFeedback),
            result: (),
            next: NextStep::TellUserAndWait,
        };

        let reply = Reply::from_outcome(Ok(output));

        assert_eq!(reply.status, Some(ReviewStatus::WaitingForUserFeedback));
    }
}
