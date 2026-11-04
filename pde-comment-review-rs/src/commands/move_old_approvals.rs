//! This module runs `move-old-approvals`, which copies the old checker's
//! approvals and the old review tool's open reviews into this program once,
//! and leaves the old files as they were. Read the `move_old_approvals(dir)`
//! and `Reply::from_move_outcome(outcome)` functions to understand the
//! module.

use std::path::{Path, PathBuf};

use serde::Serialize;

use super::Reply;
use super::old_reviews::{self, OldReviewMove, OldReviewOutcome};
use crate::shared::approvals::{self, OldApprovalsAdded};
use crate::shared::git;
use crate::shared::{Error, ErrorCode};

/// Names the old checker's approvals file at the top of each worktree.
const OLD_APPROVALS_FILE: &str = ".code-comments.json";

/// Moves main's and every worktree's old approvals into the clone's approvals
/// file, then each open review of a worktree into its Review Session. Reads
/// every old file before it writes, so an unreadable one stops the move with
/// nothing changed.
pub fn move_old_approvals(dir: &Path) -> Result<MoveReport, Error> {
    let mut old_files = Vec::new();
    let mut old_copies = Vec::new();
    let mut missing_worktrees = Vec::new();
    let mut worktrees = Vec::new();
    for worktree in git::list_worktrees(dir)? {
        if !worktree.is_dir() {
            missing_worktrees.push(worktree);
            continue;
        }
        let path = worktree.join(OLD_APPROVALS_FILE);
        let copy = approvals::load_file(&path)?;
        if path.exists() {
            old_files.push(OldApprovalsFile {
                path,
                entries: copy.count_entries(),
                avoided_words: copy.avoided_words().len(),
            });
        }
        old_copies.push(copy);
        worktrees.push(worktree);
    }
    let open_reviews = old_reviews::read_open_reviews(&worktrees)?;
    let matched_reviews = open_reviews
        .in_this_clone
        .into_iter()
        .map(old_reviews::match_open_review)
        .collect::<Result<Vec<_>, _>>()?;

    let (approvals_added, approvals) = approvals::add_old_approvals(dir, &old_copies)?;
    let reviews = matched_reviews
        .into_iter()
        .map(|matched| old_reviews::save_matched_review(matched, &approvals))
        .collect();
    Ok(MoveReport {
        old_files,
        missing_worktrees,
        approvals: approvals_added,
        reviews,
        reviews_in_other_clones: open_reviews.in_other_clones,
    })
}

// The move's reply sits here rather than in `commands/mod.rs`, so deleting
// the move deletes it too.
impl Reply {
    /// Returns the reply to `move-old-approvals`, which has no status and names
    /// no next step, since it is not a step of the Review Process. A review
    /// that did not move sets `error` beside the result, so the CLI
    /// exits 1.
    pub fn from_move_outcome(outcome: Result<MoveReport, Error>) -> Reply {
        let report = match outcome {
            Ok(report) => report,
            Err(error) => return Reply::from_error(error),
        };
        let error = report.find_review_error();
        match serde_json::to_value(report) {
            Ok(result) => Reply {
                status: None,
                result: Some(result),
                error,
                next: None,
            },
            Err(error) => Reply::from_error(Error {
                code: ErrorCode::FileUnreadable,
                message: format!("Cannot write the result as JSON. {error}"),
            }),
        }
    }
}

/// Says what the move read and did, so the user can compare counts.
#[derive(Debug, Serialize)]
pub struct MoveReport {
    /// Lists each old approvals file read, main's first.
    pub old_files: Vec<OldApprovalsFile>,
    /// Lists each worktree Git knows whose folder is gone, so no file of it
    /// was read.
    pub missing_worktrees: Vec<PathBuf>,
    pub approvals: OldApprovalsAdded,
    pub reviews: Vec<OldReviewMove>,
    /// Lists the worktree of each open review for another clone, which moves
    /// when the command runs there.
    pub reviews_in_other_clones: Vec<PathBuf>,
}

impl MoveReport {
    /// Returns an error with the first review's code when some open reviews
    /// did not move, or `None` when none failed.
    fn find_review_error(&self) -> Option<Error> {
        let mut errors = self
            .reviews
            .iter()
            .filter_map(|review| match &review.outcome {
                OldReviewOutcome::NotSaved { error } => Some(error),
                OldReviewOutcome::Moved { .. }
                | OldReviewOutcome::AlreadyHasReviewSession
                | OldReviewOutcome::NothingLeftToMove { .. } => None,
            });
        let first = errors.next()?;
        Some(Error {
            code: first.code,
            message: format!(
                "Some open reviews did not move, and each one's outcome in `reviews` says why. {}",
                first.message
            ),
        })
    }
}

/// Counts one old approvals file's entries and avoided words.
#[derive(Debug, Serialize)]
pub struct OldApprovalsFile {
    pub path: PathBuf,
    pub entries: usize,
    pub avoided_words: usize,
}
