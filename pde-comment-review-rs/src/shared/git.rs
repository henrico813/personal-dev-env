//! Runs every Git command used by the review program.

use std::collections::BTreeSet;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde::{Deserialize, Serialize};

use super::{Error, ErrorCode};

/// Holds a resolved commit hash rather than a movable branch name.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

/// Lists paths that differ from `base` on disk, in the index or in HEAD,
/// relative to the repository root, including untracked files.
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
    let staged = run_git(
        &root,
        &[
            "diff",
            "--name-only",
            "-z",
            "--no-renames",
            "--cached",
            base.as_str(),
            "--",
        ],
    )?;
    // A commit can hold a change that the disk copy and the index put back.
    let committed = run_git(
        &root,
        &[
            "diff",
            "--name-only",
            "-z",
            "--no-renames",
            base.as_str(),
            "HEAD",
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
    for output in [tracked, staged, committed, untracked] {
        for name in output
            .split(|byte| *byte == 0)
            .filter(|name| !name.is_empty())
        {
            files.insert(PathBuf::from(OsString::from_vec(name.to_vec())));
        }
    }
    Ok(files.into_iter().collect())
}

/// Marks additions and replacements in a root-relative current file.
/// Deletions have no current lines and every new file line counts.
pub fn list_changed_lines(
    dir: &Path,
    base: &CommitId,
    file: &Path,
) -> Result<BTreeSet<usize>, Error> {
    let root = find_root(dir)?;
    validate_file(file)?;
    let current = match fs::read_to_string(root.join(file)) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(BTreeSet::new());
        }
        Err(error) => return Err(file_error(error)),
    };
    if read_file_at_commit(&root, base, file)?.is_none() {
        return Ok((1..=current.lines().count()).collect());
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
    Ok(lines)
}

/// Reads historical UTF-8 text at a root-relative path, or None when absent.
pub fn read_file_at_commit(
    dir: &Path,
    commit: &CommitId,
    file: &Path,
) -> Result<Option<String>, Error> {
    read_file_at_commit_from_root(&find_root(dir)?, commit, file)
}

/// Reads like [`read_file_at_commit`] from `root`, the worktree's top folder.
pub(crate) fn read_file_at_commit_from_root(
    root: &Path,
    commit: &CommitId,
    file: &Path,
) -> Result<Option<String>, Error> {
    validate_file(file)?;
    let entry = run_git(
        root,
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
    let output = run_git(root, &[OsStr::new("show"), &object])?;
    read_text(output).map(Some)
}

/// Holds one version of a root-relative file.
pub(crate) struct FileVersion {
    /// Says where the version is, such as `in HEAD`, for messages.
    pub(crate) place: &'static str,
    /// Holds the text, or `None` where the file is absent.
    pub(crate) text: Option<String>,
}

/// Returns each distinct version of a root-relative file on disk, in the index
/// and in HEAD that Git would store differently from its version at `base`.
/// Reads only, so the index is never refreshed.
pub(crate) fn read_changed_versions(
    root: &Path,
    base: &CommitId,
    file: &Path,
) -> Result<Vec<FileVersion>, Error> {
    validate_file(file)?;
    let disk = read_disk_copy(root, file)?;
    // hash-object applies the file's line-end rules, such as core.autocrlf, and saves nothing without -w.
    let disk_id = match disk {
        Some(_) => {
            let output = run_git(
                root,
                &[
                    OsStr::new("hash-object"),
                    OsStr::new("--"),
                    file.as_os_str(),
                ],
            )?;
            Some(read_text(output)?.trim().to_owned())
        }
        None => None,
    };
    let mut seen = vec![find_tree_blob(root, base.as_str(), file)?];
    let mut versions = Vec::new();
    if !seen.contains(&disk_id) {
        versions.push(to_version(file, "on disk", disk)?);
        seen.push(disk_id);
    }
    let staged_id = find_staged_blob(root, file)?;
    let head_id = find_tree_blob(root, "HEAD", file)?;
    for (place, id) in [("in the index", staged_id), ("in HEAD", head_id)] {
        if seen.contains(&id) {
            continue;
        }
        let bytes = match &id {
            Some(id) => Some(run_git(root, &["cat-file", "blob", id])?),
            None => None,
        };
        versions.push(to_version(file, place, bytes)?);
        seen.push(id);
    }
    Ok(versions)
}

/// Reads the copy of a root-relative file on disk, where `root` is the
/// worktree's top folder.
pub(crate) fn read_file_on_disk(root: &Path, file: &Path) -> Result<FileVersion, Error> {
    validate_file(file)?;
    let bytes = read_disk_copy(root, file)?;
    to_version(file, "on disk", bytes)
}

/// Returns true when `commit` is HEAD or one of its ancestors.
pub(crate) fn is_ancestor_of_head(dir: &Path, commit: &CommitId) -> Result<bool, Error> {
    let output = git_command(dir)
        .args(["merge-base", "--is-ancestor", commit.as_str(), "HEAD"])
        .output()
        .map_err(command_error)?;
    match output.status.code() {
        Some(0) => Ok(true),
        Some(1) => Ok(false),
        _ => Err(git_failure(dir, output)),
    }
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

/// Returns the blob ID of `file` in a commit's tree, or `None` when no file is there.
fn find_tree_blob(root: &Path, revision: &str, file: &Path) -> Result<Option<String>, Error> {
    let output = run_git(
        root,
        &[
            OsStr::new("ls-tree"),
            OsStr::new("-z"),
            OsStr::new(revision),
            OsStr::new("--"),
            file.as_os_str(),
        ],
    )?;
    // Each entry reads `<mode> <type> <id>` and a tab before its path.
    Ok(read_entries(&output, file)
        .into_iter()
        .find(|fields| fields.get(1).map(String::as_str) == Some("blob"))
        .and_then(|fields| fields.get(2).cloned()))
}

/// Returns the blob ID of `file` in the index, or `None` when it is absent or
/// in a merge conflict.
fn find_staged_blob(root: &Path, file: &Path) -> Result<Option<String>, Error> {
    let output = run_git(
        root,
        &[
            OsStr::new("ls-files"),
            OsStr::new("--stage"),
            OsStr::new("-z"),
            OsStr::new("--"),
            file.as_os_str(),
        ],
    )?;
    // Each entry reads `<mode> <id> <stage>`, and only stage 0 is free of conflicts.
    Ok(read_entries(&output, file)
        .into_iter()
        .find(|fields| fields.get(2).map(String::as_str) == Some("0"))
        .and_then(|fields| fields.get(1).cloned()))
}

/// Returns the space-separated fields of each `-z` entry whose path is `file`.
fn read_entries(output: &[u8], file: &Path) -> Vec<Vec<String>> {
    output
        .split(|byte| *byte == 0)
        .filter_map(|entry| {
            let tab = entry.iter().position(|byte| *byte == b'\t')?;
            (&entry[tab + 1..] == file.as_os_str().as_bytes()).then(|| {
                String::from_utf8_lossy(&entry[..tab])
                    .split(' ')
                    .map(str::to_owned)
                    .collect()
            })
        })
        .collect()
}

fn read_disk_copy(root: &Path, file: &Path) -> Result<Option<Vec<u8>>, Error> {
    match fs::read(root.join(file)) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(unreadable_version(file, "on disk", error)),
    }
}

fn to_version(
    file: &Path,
    place: &'static str,
    bytes: Option<Vec<u8>>,
) -> Result<FileVersion, Error> {
    let text = bytes
        .map(String::from_utf8)
        .transpose()
        .map_err(|error| unreadable_version(file, place, error))?;
    Ok(FileVersion { place, text })
}

fn unreadable_version(file: &Path, place: &str, error: impl std::fmt::Display) -> Error {
    Error {
        code: ErrorCode::FileUnreadable,
        message: format!("Cannot read {} {place}. {error}", file.display()),
    }
}

/// Returns the worktree's top folder, which the readers that take `root` need.
pub(crate) fn find_root(dir: &Path) -> Result<PathBuf, Error> {
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
        return Err(Error {
            code: ErrorCode::BadInput,
            message: "Use a path relative to the repository root.".to_owned(),
        });
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
    Error {
        code,
        message: format!("Git failed with {}. {}", output.status, detail.trim()),
    }
}

fn read_text(output: Vec<u8>) -> Result<String, Error> {
    String::from_utf8(output).map_err(|error| Error {
        code: ErrorCode::FileUnreadable,
        message: format!("Git returned text that is not UTF-8. {error}"),
    })
}

fn command_error(error: io::Error) -> Error {
    Error {
        code: ErrorCode::FileUnreadable,
        message: format!("Cannot run Git. {error}"),
    }
}

fn file_error(error: io::Error) -> Error {
    Error {
        code: ErrorCode::FileUnreadable,
        message: format!("Cannot read the current file. {error}"),
    }
}

fn invalid_diff() -> Error {
    Error {
        code: ErrorCode::BadInput,
        message: "Cannot read Git's line diff.".to_owned(),
    }
}
