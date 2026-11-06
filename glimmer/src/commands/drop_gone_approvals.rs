//! The user runs `drop-gone-approvals` after a pull request merges. It
//! removes approvals whose code was deleted, and backs up the approvals file
//! first.

use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::shared::Error;
use crate::shared::approvals::{self, Approval, ApprovalsBackup};
use crate::shared::comments::{self, FileComments};
use crate::shared::git;

/// Caps the dropped approvals a reply lists, since one run can drop hundreds.
const LISTED_DROPS: usize = 5;

/// Says how many approvals and cuts `drop-gone-approvals` removed. It lists
/// the first few and names the backup that holds them all.
#[derive(Debug, Serialize)]
pub struct DroppedApprovals {
    pub dropped: usize,
    pub first_dropped: Vec<Approval>,
    pub backup: ApprovalsBackup,
}

/// Removes approvals and cuts for functions or files that no worktree has any
/// more, since they pile up after code is deleted. A branch still in a
/// worktree keeps its approvals, even before it merges. Refuses and removes
/// nothing when a file does not parse.
pub fn drop_gone_approvals(dir: &Path) -> Result<DroppedApprovals, Error> {
    let worktrees = git::list_worktrees(dir)?;
    let mut copies: HashMap<PathBuf, Vec<FileComments>> = HashMap::new();
    let (mut dropped, backup) = approvals::remove(dir, |approval| {
        let copies = match copies.entry(approval.file.clone()) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => entry.insert(read_every_copy(&worktrees, &approval.file)?),
        };
        Ok(copies
            .iter()
            .all(|copy| copy.find_code_line(&approval.name).is_none()))
    })?;
    let count = dropped.len();
    dropped.truncate(LISTED_DROPS);
    Ok(DroppedApprovals {
        dropped: count,
        first_dropped: dropped,
        backup,
    })
}

/// Reads `file` in each worktree, as an absent file where a worktree lacks it.
fn read_every_copy(worktrees: &[PathBuf], file: &Path) -> Result<Vec<FileComments>, Error> {
    worktrees
        .iter()
        .map(|worktree| comments::read_comments_on_disk(worktree, file))
        .collect()
}
