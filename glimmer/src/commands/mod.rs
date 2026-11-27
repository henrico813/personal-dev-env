//! This module builds the `{status, result, error, next}` reply that the
//! command line and MCP both give, from a step's plain Rust result. Read the
//! `Reply::from_outcome(outcome)` function to understand the module.

use std::path::Path;

use serde::Serialize;
use serde_json::Value;

use crate::shared::git::{self, CommitId};
use crate::shared::review_session::ReviewStatus;
use crate::shared::{Error, ErrorCode};
use crate::steps::{NextStep, StepOutput};

pub mod cli;
pub mod dismiss;
pub mod drop_gone_approvals;
mod help_text;
pub mod mcp;

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

    /// Returns the reply to a command outside the Review Process. The reply
    /// has no status and names no next step.
    pub fn from_drop_outcome<T: Serialize>(outcome: Result<T, Error>) -> Reply {
        let result = outcome.and_then(|result| {
            serde_json::to_value(result).map_err(|error| Error {
                code: ErrorCode::FileUnreadable,
                message: format!("Cannot write the result as JSON. {error}"),
            })
        });
        match result {
            Ok(result) => Reply {
                status: None,
                result: Some(result),
                error: None,
                next: None,
            },
            Err(error) => Reply::from_error(error),
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
