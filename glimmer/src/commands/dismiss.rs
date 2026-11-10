//! This module runs `dismiss` and `restore`, which take a review off Ctrl+b
//! e's list and put it back. Each works on the worktree with branch NAME
//! checked out, or without a name on the worktree it runs in. Read the
//! `dismiss_review(dir, branch)` and `restore_review(dir, branch)` functions
//! to understand the module.

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::review_screen::find_worktrees_by_branch;
use crate::shared::review_session::{self, ReviewStatus};
use crate::shared::{Error, ErrorCode, git};
use crate::steps::{NextStep, StepOutput};

/// Dismisses the review of the worktree with `branch` checked out, or of the
/// one that holds `dir` without a branch.
pub fn dismiss_review(
    dir: &Path,
    branch: Option<&str>,
) -> Result<StepOutput<ChangedReview>, Error> {
    let worktree = find_named_worktree(dir, branch)?;
    review_session::dismiss(&worktree)?;
    Ok(StepOutput {
        status: Some(ReviewStatus::Dismissed),
        result: ChangedReview { worktree },
        next: NextStep::FinishWithoutFeedbackRequests,
    })
}

/// Restores the dismissed review of the worktree with `branch` checked out, or
/// of the one that holds `dir` without a branch.
pub fn restore_review(
    dir: &Path,
    branch: Option<&str>,
) -> Result<StepOutput<ChangedReview>, Error> {
    let worktree = find_named_worktree(dir, branch)?;
    review_session::restore(&worktree)?;
    Ok(StepOutput {
        status: Some(ReviewStatus::WaitingForUserFeedback),
        result: ChangedReview { worktree },
        next: NextStep::TellUserAndWait,
    })
}

/// Names the worktree whose review changed.
#[derive(Debug, Serialize)]
pub struct ChangedReview {
    pub worktree: PathBuf,
}

/// Returns the one worktree with `branch` checked out, found as `open NAME`
/// finds it, or the worktree that holds `dir` without a branch.
fn find_named_worktree(dir: &Path, branch: Option<&str>) -> Result<PathBuf, Error> {
    let Some(branch) = branch else {
        return git::find_root(dir);
    };
    // A damaged waiting list must not stop Git from finding the branch.
    let waiting = review_session::read_waiting_list().unwrap_or_default();
    let mut worktrees = find_worktrees_by_branch(branch, &waiting, dir);
    match worktrees.len() {
        0 => Err(Error {
            code: ErrorCode::BadInput,
            message: format!("No worktree has branch {branch} checked out."),
        }),
        1 => Ok(worktrees.remove(0)),
        count => Err(Error {
            code: ErrorCode::BadInput,
            message: format!(
                "{count} worktrees have branch {branch} checked out. Run the command with no name inside the one you mean."
            ),
        }),
    }
}
