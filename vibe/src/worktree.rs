use std::fs::{File, OpenOptions, TryLockError};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::adapters::git;

pub struct WorktreeSession {
    pub key: String,
    pub slug: String,
    pub branch: String,
    pub worktree: PathBuf,
    repo_root: PathBuf,
    git_common_dir: PathBuf,
}

pub struct SandboxMounts {
    pub repo_root: PathBuf,
    pub git_common_dir: PathBuf,
    pub worktree: PathBuf,
    pub inputs: Vec<PathBuf>,
}

#[derive(Debug)]
pub struct RunLock {
    _file: File,
}

impl Drop for RunLock {
    fn drop(&mut self) {
        let _ = self._file.unlock();
    }
}

impl WorktreeSession {
    pub fn repo_root(&self) -> &Path {
        &self.repo_root
    }

    pub fn sandbox_mounts(&self, inputs: &[PathBuf]) -> SandboxMounts {
        SandboxMounts {
            repo_root: self.repo_root.clone(),
            git_common_dir: self.git_common_dir.clone(),
            worktree: self.worktree.clone(),
            inputs: inputs.to_vec(),
        }
    }
}

pub fn slugify(key: &str) -> String {
    let mut out = String::new();
    let mut dash = false;
    for ch in key.to_lowercase().chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
            dash = false;
        } else if !dash {
            out.push('-');
            dash = true;
        }
    }
    let trimmed: String = out.trim_matches('-').chars().take(48).collect();
    if trimmed.is_empty() {
        "vibe".to_string()
    } else {
        trimmed
    }
}

pub fn acquire_run_lock(key: &str) -> Result<RunLock, String> {
    let repo = git::repo_layout()?;
    let home = std::env::var("HOME").map_err(|_| "HOME not set".to_string())?;
    let repo_id = repo
        .repo_root
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("repo");
    let lock_dir = Path::new(&home)
        .join(".local/state/vibe")
        .join(repo_id)
        .join("locks");
    acquire_run_lock_in(&lock_dir, key)
}

fn acquire_run_lock_in(lock_dir: &Path, key: &str) -> Result<RunLock, String> {
    let slug = slugify(key);
    std::fs::create_dir_all(lock_dir)
        .map_err(|error| format!("create Vibe lock directory: {error}"))?;
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(lock_dir.join(format!("{slug}.lock")))
        .map_err(|error| format!("open Vibe run lock for {slug}: {error}"))?;
    match file.try_lock() {
        Ok(()) => Ok(RunLock { _file: file }),
        Err(TryLockError::WouldBlock) => Err(format!("vibe run already active for slug {slug}")),
        Err(TryLockError::Error(error)) => Err(format!("lock Vibe run for {slug}: {error}")),
    }
}

pub fn validate_base_target(key: &str, base: Option<&str>) -> Result<(), String> {
    let repo = git::repo_layout()?;
    let slug = slugify(key);
    let branch = format!("vibe/{slug}");
    let worktree = repo.repo_root.join("worktrees").join(&slug);
    git::validate_base_target(&repo.repo_root, &worktree, &branch, base)
}

pub fn prepare(key: &str, base: Option<&str>) -> Result<WorktreeSession, String> {
    let repo = git::repo_layout()?;
    let slug = slugify(key);
    let branch = format!("vibe/{slug}");
    let worktree = repo.repo_root.join("worktrees").join(&slug);
    git::ensure_worktree(
        &repo.repo_root,
        &worktree,
        &branch,
        &repo.git_common_dir,
        base,
    )?;
    Ok(WorktreeSession {
        key: key.to_string(),
        slug,
        branch,
        worktree,
        repo_root: repo.repo_root,
        git_common_dir: repo.git_common_dir,
    })
}

pub fn refuse_if_dirty(worktree: &Path) -> Result<(), String> {
    if git::is_dirty(worktree)? {
        Err("worktree has uncommitted changes".to_string())
    } else {
        Ok(())
    }
}

pub fn pre_run_commit(worktree: &Path) -> Result<String, String> {
    git::head_sha(worktree)
}

pub fn is_dirty(worktree: &Path) -> Result<bool, String> {
    git::is_dirty(worktree)
}

pub fn changed_files(worktree: &Path) -> Result<Vec<String>, String> {
    git::changed_files_in_worktree(worktree)
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn changed_files_since(
    worktree: &Path,
    from: &str,
    to: Option<&str>,
) -> Result<Vec<String>, String> {
    let mut command = Command::new("git");
    command.arg("-C").arg(worktree.to_str().unwrap_or("."));
    command.args(["diff", "--name-only"]);
    match to {
        Some(to) => {
            command.args([from, to]);
        }
        None => {
            command.arg(from);
        }
    }
    let out = command
        .output()
        .map_err(|e| format!("git diff --name-only: {e}"))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    let text = String::from_utf8_lossy(&out.stdout);
    Ok(text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| line.to_string())
        .collect())
}

pub fn commit_result(worktree: &Path, message: &str, hooks_dir: &Path) -> Result<String, String> {
    git::commit_all(worktree, message, hooks_dir, "result")
}

#[cfg(test)]
mod tests {
    use super::{acquire_run_lock_in, slugify};
    use tempfile::tempdir;

    #[test]
    fn slugify_normalizes_keys() {
        assert_eq!(slugify("PDEV-049 demo/key"), "pdev-049-demo-key");
    }

    #[test]
    fn empty_slug_falls_back() {
        assert_eq!(slugify("---"), "vibe");
    }

    #[test]
    fn same_slug_blocks_until_release() {
        let temp = tempdir().expect("tempdir");
        let first = acquire_run_lock_in(temp.path(), "Demo/key").expect("first lock");

        let error = acquire_run_lock_in(temp.path(), "demo-key").expect_err("second lock");
        assert!(error.contains("already active"), "{error}");

        drop(first);
        acquire_run_lock_in(temp.path(), "demo-key").expect("released lock");
    }
}
