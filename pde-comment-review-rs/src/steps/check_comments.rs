//! This module runs step 2, which fails a branch that rewords an approved
//! comment or removes one whose code stays, and writes no file. Read the
//! `check_comments(dir, base)` function to understand the module.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use super::{NextStep, StepOutput};
use crate::shared::approvals::{self, Approvals, DecisionMatch};
use crate::shared::comment_rules::{Finding, Rule};
use crate::shared::comments::{self, FileComments};
use crate::shared::git::{self, CommitId};
use crate::shared::{Error, ErrorCode};

/// Returns step 2's failures for changes since `base`, or since the branch
/// start when it is `None`. Refuses a `base` that HEAD does not build on, and
/// a changed file that does not parse now or at `base`, naming the file.
pub fn check_comments(
    dir: &Path,
    base: Option<CommitId>,
) -> Result<StepOutput<CheckResult>, Error> {
    let base = match base {
        Some(base) => {
            // A commit outside the branch's history could already hold the branch's edits.
            if !git::is_ancestor_of_head(dir, &base)? {
                return Err(Error {
                    code: ErrorCode::BadInput,
                    message: format!(
                        "Pass a base that HEAD builds on. {} is not one.",
                        base.as_str()
                    ),
                });
            }
            base
        }
        None => git::find_branch_start(dir)?,
    };
    let approvals = approvals::load(dir)?;
    let changed: BTreeSet<PathBuf> = git::list_changed_files(dir, &base)?.into_iter().collect();
    let files: BTreeSet<&Path> = changed
        .iter()
        .map(PathBuf::as_path)
        .chain(approvals.files())
        .collect();
    let mut failures = Vec::new();
    let mut unsupported = Vec::new();
    for file in files {
        if !comments::is_supported(file) {
            if changed.contains(file) {
                unsupported.push(file.to_path_buf());
            }
            continue;
        }
        // Commits and the index ship too, so each version that differs from `base` is checked.
        let versions = git::read_changed_versions(dir, &base, file)?;
        if versions.is_empty() {
            continue;
        }
        let before = comments::read_comments_at_commit(dir, &base, file)?;
        for version in versions {
            let now = comments::read_version_comments(file, version)?;
            let found = find_changed_approved_texts(&approvals, &now, &before)
                .into_iter()
                .chain(find_removed_approved_comments(&approvals, &now, &before));
            for failure in found {
                if !failures.contains(&failure) {
                    failures.push(failure);
                }
            }
        }
    }
    let next = if failures.is_empty() {
        NextStep::MakeFeedbackRequestList
    } else {
        NextStep::CheckComments
    };
    Ok(StepOutput {
        result: CheckResult {
            failures,
            unsupported,
        },
        next,
    })
}

/// Holds what step 2 found.
#[derive(Debug)]
pub struct CheckResult {
    pub failures: Vec<Finding>,
    /// Lists changed files no language rule reads, whose comments go unchecked.
    pub unsupported: Vec<PathBuf>,
}

fn find_changed_approved_texts(
    approvals: &Approvals,
    now: &FileComments,
    before: &FileComments,
) -> Vec<Finding> {
    let mut failures = Vec::new();
    for comment in &now.comments {
        // Another worktree may have decided on an unchanged comment, so step 3 asks instead.
        if before.has_same(comment) {
            continue;
        }
        // A comment on a name the user cut passes, so step 3 can ask whether they want it back.
        if approvals.compare(comment) == DecisionMatch::DiffersFromApproved {
            failures.push(Finding {
                file: comment.file.clone(),
                name: comment.name.clone(),
                line: comment.line,
                rule: Rule::Approved,
            });
        }
    }
    failures
}

fn find_removed_approved_comments(
    approvals: &Approvals,
    now: &FileComments,
    before: &FileComments,
) -> Vec<Finding> {
    let mut failures = Vec::new();
    for old in &before.comments {
        let Some(name) = &old.name else {
            continue;
        };
        let still_named = now
            .comments
            .iter()
            .any(|comment| comment.name.as_ref() == Some(name));
        if still_named || approvals.compare(old) != DecisionMatch::Approved {
            continue;
        }
        // An approval whose code was deleted or renamed protects nothing.
        if let Some(line) = now.find_code_line(name) {
            failures.push(Finding {
                file: old.file.clone(),
                name: Some(name.clone()),
                line,
                rule: Rule::ApprovedRemoved,
            });
        }
    }
    failures
}
