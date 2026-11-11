//! Finds a card's linked worktrees and what unsaved work they hold.
//! No fetch runs, so a branch pushed from elsewhere looks unpushed and
//! its session stays.

use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::cards::CardKey;

/// What a session's worktrees hold that only they hold. From worst to
/// best, a git failure, then files, then commits, then clean.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Unsaved {
    /// Nothing; every file is committed and every commit pushed.
    Clean,
    /// Uncommitted files exist in this worktree.
    Files(PathBuf),
    /// No remote branch holds these commits, in this worktree.
    Commits(PathBuf),
    /// git's own message.
    GitFailed(String),
    /// The session cannot be parked or closed, so git did not look.
    NotChecked,
}

/// A git call that failed.
#[derive(Debug)]
pub enum GitError {
    NotInstalled,
    /// git exited nonzero, with its standard error.
    Failed {
        folder: PathBuf,
        message: String,
    },
}

/// Writes git's own message and the folder it ran in.
impl fmt::Display for GitError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GitError::NotInstalled => formatter.write_str("git is not installed"),
            GitError::Failed { folder, message } => {
                write!(formatter, "{message} in {}", folder.display())
            }
        }
    }
}

/// Returns the linked worktrees that hold the agent's folders or, with
/// a key, whose branch names it. Never returns the main checkout,
/// which holds the user's own files, or a worktree whose folder is
/// gone. A folder that is gone or not in a repository is skipped.
pub fn find_card_worktrees(
    folders: &[PathBuf],
    key: Option<&CardKey>,
) -> Result<Vec<PathBuf>, GitError> {
    let lower_key = key.map(CardKey::to_lowercase);
    let mut found: Vec<PathBuf> = Vec::new();
    for folder in folders {
        // Herdr adds this mark to a removed folder, such as a merged
        // worktree.
        let text = folder.to_string_lossy();
        let folder = Path::new(text.strip_suffix(" (deleted)").unwrap_or(&text));
        if !folder.is_dir() || !is_in_repository(folder) {
            continue;
        }
        // git prints each worktree's real path, without symbolic links.
        let real_folder = folder.canonicalize().unwrap_or_else(|_| folder.to_owned());
        let listing = run_git(folder, &["worktree", "list", "--porcelain"])?;
        for (worktree, branch) in parse_worktree_list(&listing) {
            let in_use = real_folder.starts_with(&worktree);
            let names_key = lower_key.as_deref().is_some_and(|key| branch.contains(key));
            if (in_use || names_key) && worktree.is_dir() && !found.contains(&worktree) {
                found.push(worktree);
            }
        }
    }
    Ok(found)
}

/// Returns files, commits or clean for one worktree.
pub fn check_unsaved(worktree: &Path) -> Result<Unsaved, GitError> {
    // A repository that sets `status.showUntrackedFiles` to `no` would
    // otherwise hide a worker's new files.
    let status_args = ["status", "--porcelain", "--untracked-files=normal"];
    if !run_git(worktree, &status_args)?.is_empty() {
        return Ok(Unsaved::Files(worktree.to_owned()));
    }
    let unpushed = run_git(worktree, &["rev-list", "HEAD", "--not", "--remotes"])?;
    if !unpushed.is_empty() {
        return Ok(Unsaved::Commits(worktree.to_owned()));
    }
    Ok(Unsaved::Clean)
}

/// Tells whether a folder or one of its parents holds `.git`.
fn is_in_repository(folder: &Path) -> bool {
    folder.ancestors().any(|path| path.join(".git").exists())
}

/// Runs git in `folder` and returns what it printed.
fn run_git(folder: &Path, args: &[&str]) -> Result<String, GitError> {
    let output = Command::new("git")
        .arg("-C")
        .arg(folder)
        .args(args)
        .output()
        .map_err(|error| match error.kind() {
            io::ErrorKind::NotFound => GitError::NotInstalled,
            _ => GitError::Failed {
                folder: folder.to_owned(),
                message: format!("cannot run git: {error}"),
            },
        })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        return Err(GitError::Failed {
            folder: folder.to_owned(),
            message: if stderr.is_empty() {
                format!("git ended with {}", output.status)
            } else {
                stderr
            },
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

/// Returns each linked worktree and its branch from
/// `git worktree list --porcelain`, whose blocks are split by a blank
/// line. git lists the main checkout first, and it is left out. A
/// detached worktree has an empty branch.
fn parse_worktree_list(text: &str) -> Vec<(PathBuf, String)> {
    text.split("\n\n")
        .skip(1)
        .filter_map(|block| {
            let mut worktree = None;
            let mut branch = String::new();
            for line in block.lines() {
                if let Some(path) = line.strip_prefix("worktree ") {
                    worktree = Some(PathBuf::from(path));
                } else if let Some(name) = line.strip_prefix("branch ") {
                    branch = name.to_owned();
                }
            }
            worktree.map(|worktree| (worktree, branch))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The main checkout holds the user's own files, which are never a
    /// card's unsaved work. Counting it would keep every session open
    /// forever, since the user always has something uncommitted there.
    #[test]
    fn parse_worktree_list_skips_main_checkout() {
        let listing = "worktree /home/u/repo\n\
                       HEAD 1111111111111111111111111111111111111111\n\
                       branch refs/heads/main\n\
                       \n\
                       worktree /home/u/wt/demo-kb-007\n\
                       HEAD 2222222222222222222222222222222222222222\n\
                       branch refs/heads/feat/demo-kb-007-retries\n";

        let worktrees = parse_worktree_list(listing);

        assert_eq!(
            worktrees,
            [(
                PathBuf::from("/home/u/wt/demo-kb-007"),
                "refs/heads/feat/demo-kb-007-retries".to_owned(),
            )]
        );
    }
}
