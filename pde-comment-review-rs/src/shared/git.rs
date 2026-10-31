//! Runs every Git command used by the review program.

use std::collections::BTreeSet;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use super::{Error, ErrorCode};

/// Holds a resolved commit hash rather than a movable branch name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitId(String);

impl CommitId {
    /// Borrows the full hash for reports and later saved lists.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Resolves an explicit task start to a commit before reading files.
pub fn resolve_commit(dir: &Path, revision: &str) -> Result<CommitId, Error> {
    let revision = format!("{revision}^{{commit}}");
    let output = run_git(
        dir,
        &["rev-parse", "--verify", "--end-of-options", &revision],
    )?;
    Ok(CommitId(read_text(output)?.trim().to_owned()))
}

/// Finds the split from local main, or remote main when local main is absent.
pub fn find_branch_start(dir: &Path) -> Result<CommitId, Error> {
    let local_main = git_command(dir)
        .args(["show-ref", "--verify", "--quiet", "refs/heads/main"])
        .output()
        .map_err(command_error)?;
    let main = if local_main.status.success() {
        "refs/heads/main"
    } else if local_main.status.code() == Some(1) {
        "refs/remotes/origin/main"
    } else {
        return Err(git_failure(dir, local_main));
    };
    let output = run_git(dir, &["merge-base", "HEAD", main])?;
    Ok(CommitId(read_text(output)?.trim().to_owned()))
}

/// Lists changed paths relative to the repository root, including untracked files.
/// Deleted, staged and unstaged paths count too.
pub fn list_changed_files(dir: &Path, base: &CommitId) -> Result<Vec<PathBuf>, Error> {
    let root = find_root(dir)?;
    // Touched but unchanged files may be listed to keep the index read-only.
    let tracked = run_git(
        &root,
        &[
            "diff",
            "--name-only",
            "-z",
            "--no-renames",
            base.as_str(),
            "--",
        ],
    )?;
    let untracked = run_git(
        &root,
        &[
            "ls-files",
            "--others",
            "--exclude-standard",
            "--full-name",
            "-z",
        ],
    )?;
    let mut files = BTreeSet::new();
    for output in [tracked, untracked] {
        for name in output
            .split(|byte| *byte == 0)
            .filter(|name| !name.is_empty())
        {
            files.insert(PathBuf::from(OsString::from_vec(name.to_vec())));
        }
    }
    Ok(files.into_iter().collect())
}

/// Holds changed line numbers in the current file, counted from one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangedLines(BTreeSet<usize>);

impl ChangedLines {
    /// Lets style rules test a current line without exposing mutation.
    pub fn contains(&self, line: usize) -> bool {
        self.0.contains(&line)
    }

    /// Borrows the ordered set for callers that visit changed lines.
    pub fn lines(&self) -> &BTreeSet<usize> {
        &self.0
    }
}

/// Marks additions and replacements in a root-relative current file.
/// Deletions have no current lines and every new file line counts.
pub fn list_changed_lines(dir: &Path, base: &CommitId, file: &Path) -> Result<ChangedLines, Error> {
    let root = find_root(dir)?;
    validate_file(file)?;
    let current = match fs::read_to_string(root.join(file)) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(ChangedLines(BTreeSet::new()));
        }
        Err(error) => return Err(file_error(error)),
    };
    if read_file_at_commit(&root, base, file)?.is_none() {
        return Ok(ChangedLines((1..=current.lines().count()).collect()));
    }
    let output = run_git(
        &root,
        &[
            OsStr::new("diff"),
            OsStr::new("--no-ext-diff"),
            OsStr::new("--no-textconv"),
            OsStr::new("--no-renames"),
            OsStr::new("--text"),
            OsStr::new("--unified=0"),
            OsStr::new("--inter-hunk-context=0"),
            OsStr::new("--no-color"),
            OsStr::new(base.as_str()),
            OsStr::new("--"),
            file.as_os_str(),
        ],
    )?;
    let diff = read_text(output)?;
    let mut lines = BTreeSet::new();
    for header in diff.lines().filter(|line| line.starts_with("@@ ")) {
        let range = header
            .split_whitespace()
            .nth(2)
            .and_then(|range| range.strip_prefix('+'))
            .ok_or_else(invalid_diff)?;
        let (start, count) = range.split_once(',').unwrap_or((range, "1"));
        let start: usize = start.parse().map_err(|_| invalid_diff())?;
        let count: usize = count.parse().map_err(|_| invalid_diff())?;
        let end = start.checked_add(count).ok_or_else(invalid_diff)?;
        lines.extend(start..end);
    }
    Ok(ChangedLines(lines))
}

/// Reads historical UTF-8 text at a root-relative path, or None when absent.
pub fn read_file_at_commit(
    dir: &Path,
    commit: &CommitId,
    file: &Path,
) -> Result<Option<String>, Error> {
    let root = find_root(dir)?;
    validate_file(file)?;
    let entry = run_git(
        &root,
        &[
            OsStr::new("ls-tree"),
            OsStr::new("-z"),
            OsStr::new(commit.as_str()),
            OsStr::new("--"),
            file.as_os_str(),
        ],
    )?;
    if entry.is_empty() {
        return Ok(None);
    }
    let mut object = OsString::from(format!("{}:", commit.as_str()));
    object.push(file);
    let output = run_git(&root, &[OsStr::new("show"), &object])?;
    read_text(output).map(Some)
}

/// Finds the absolute Git folder that belongs to this worktree alone.
pub fn find_worktree_git_folder(dir: &Path) -> Result<PathBuf, Error> {
    find_git_folder(dir, "--git-dir")
}

/// Finds the absolute Git folder shared by every worktree of the clone.
pub fn find_shared_git_folder(dir: &Path) -> Result<PathBuf, Error> {
    find_git_folder(dir, "--git-common-dir")
}

/// Returns the branch name, or HEAD when the checkout is detached.
pub fn read_current_branch(dir: &Path) -> Result<String, Error> {
    let output = run_git(dir, &["rev-parse", "--abbrev-ref", "HEAD"])?;
    Ok(read_text(output)?.trim_end_matches('\n').to_owned())
}

fn find_root(dir: &Path) -> Result<PathBuf, Error> {
    let output = run_git(dir, &["rev-parse", "--show-toplevel"])?;
    Ok(read_path(output))
}

fn find_git_folder(dir: &Path, option: &str) -> Result<PathBuf, Error> {
    // Reject a folder that is not a working checkout.
    find_root(dir)?;
    let output = run_git(dir, &["rev-parse", "--path-format=absolute", option])?;
    Ok(read_path(output))
}

fn read_path(mut output: Vec<u8>) -> PathBuf {
    if output.last() == Some(&b'\n') {
        output.pop();
    }
    PathBuf::from(OsString::from_vec(output))
}

fn validate_file(file: &Path) -> Result<(), Error> {
    // Reject paths that climb outside the repository.
    if file.as_os_str().as_bytes().is_empty()
        || file
            .components()
            .any(|part| !matches!(part, std::path::Component::Normal(_)))
    {
        return Err(Error::new(
            ErrorCode::BadInput,
            "Use a path relative to the repository root.".to_owned(),
        ));
    }
    Ok(())
}

fn git_command(dir: &Path) -> Command {
    let mut command = Command::new("git");
    command.args([
        OsStr::new("-c"),
        OsStr::new("diff.autoRefreshIndex=false"),
        OsStr::new("--literal-pathspecs"),
        OsStr::new("-C"),
        dir.as_os_str(),
    ]);
    command.env("LC_ALL", "C").env("GIT_OPTIONAL_LOCKS", "0");
    for name in [
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_INDEX_FILE",
        "GIT_COMMON_DIR",
        "GIT_OBJECT_DIRECTORY",
        "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    ] {
        command.env_remove(name);
    }
    command
}

fn run_git<S: AsRef<OsStr>>(dir: &Path, args: &[S]) -> Result<Vec<u8>, Error> {
    let output = git_command(dir)
        .args(args)
        .output()
        .map_err(command_error)?;
    if output.status.success() {
        Ok(output.stdout)
    } else {
        Err(git_failure(dir, output))
    }
}

fn git_failure(dir: &Path, output: Output) -> Error {
    let checkout = git_command(dir)
        .args(["rev-parse", "--is-inside-work-tree"])
        .output();
    let code = match checkout {
        Ok(checkout) if checkout.status.success() && checkout.stdout == b"true\n" => {
            ErrorCode::BadInput
        }
        _ => ErrorCode::NotAGitCheckout,
    };
    let detail = String::from_utf8_lossy(&output.stderr);
    Error::new(
        code,
        format!("Git failed with {}. {}", output.status, detail.trim()),
    )
}

fn read_text(output: Vec<u8>) -> Result<String, Error> {
    String::from_utf8(output).map_err(|error| {
        Error::with_source(
            ErrorCode::FileUnreadable,
            "Git returned text that is not UTF-8.".to_owned(),
            error,
        )
    })
}

fn command_error(error: io::Error) -> Error {
    Error::with_source(
        ErrorCode::FileUnreadable,
        "Cannot run Git.".to_owned(),
        error,
    )
}

fn file_error(error: io::Error) -> Error {
    Error::with_source(
        ErrorCode::FileUnreadable,
        "Cannot read the current file.".to_owned(),
        error,
    )
}

fn invalid_diff() -> Error {
    Error::new(
        ErrorCode::BadInput,
        "Cannot read Git's line diff.".to_owned(),
    )
}
