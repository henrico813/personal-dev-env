use std::fs::{File, OpenOptions, TryLockError};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::{adapters::git, target::RunTarget};

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
    pub fn sandbox_mounts(&self, inputs: &[PathBuf]) -> SandboxMounts {
        SandboxMounts {
            repo_root: self.repo_root.clone(),
            git_common_dir: self.git_common_dir.clone(),
            worktree: self.worktree.clone(),
            inputs: inputs.to_vec(),
        }
    }
}

pub fn acquire_run_lock(target: &RunTarget) -> Result<RunLock, String> {
    let home = std::env::var("HOME").map_err(|_| "HOME not set".to_string())?;
    acquire_run_lock_in(target, Path::new(&home))
}

fn acquire_run_lock_in(target: &RunTarget, home: &Path) -> Result<RunLock, String> {
    let state_dir = target.state_dir(home);
    std::fs::create_dir_all(&state_dir)
        .map_err(|error| format!("create Vibe state directory: {error}"))?;
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(state_dir.join("run.lock"))
        .map_err(|error| format!("open Vibe run lock for {}: {error}", target.slug()))?;
    match file.try_lock() {
        Ok(()) => {
            target.check_stored_key(home)?;
            write_key_atomic(&target.key_path(home), target.key())?;
            Ok(RunLock { _file: file })
        }
        Err(TryLockError::WouldBlock) => Err(format!(
            "vibe run already active for slug {}",
            target.slug()
        )),
        Err(TryLockError::Error(error)) => {
            Err(format!("lock Vibe run for {}: {error}", target.slug()))
        }
    }
}

fn write_key_atomic(path: &Path, key: &str) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("key path has no parent: {}", path.display()))?;
    let tmp = parent.join(".key.tmp");
    std::fs::write(&tmp, key).map_err(|error| format!("write Vibe key: {error}"))?;
    std::fs::rename(&tmp, path).map_err(|error| format!("rename Vibe key: {error}"))
}

pub fn validate_base_target(target: &RunTarget, base: Option<&str>) -> Result<(), String> {
    git::validate_base_target(
        target.repo_root(),
        &target.worktree_path(),
        &target.branch(),
        base,
    )
}

pub fn prepare(target: &RunTarget, base: Option<&str>) -> Result<WorktreeSession, String> {
    let worktree = target.worktree_path();
    let branch = target.branch();
    git::ensure_worktree(
        target.repo_root(),
        &worktree,
        &branch,
        target.git_common_dir(),
        base,
    )?;
    Ok(WorktreeSession {
        key: target.key().to_string(),
        slug: target.slug().to_string(),
        branch,
        worktree,
        repo_root: target.repo_root().to_path_buf(),
        git_common_dir: target.git_common_dir().to_path_buf(),
    })
}

pub fn refuse_if_dirty(worktree: &Path) -> Result<(), String> {
    if git::is_dirty(worktree)? {
        Err("worktree has uncommitted changes".to_string())
    } else {
        Ok(())
    }
}

pub fn validate_repository_skills(worktree: &Path) -> Result<(), String> {
    git::validate_repository_skills(worktree)
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
    use super::acquire_run_lock_in;
    use crate::target::RunTarget;
    use std::path::{Path, PathBuf};
    use tempfile::tempdir;

    #[test]
    fn same_slug_rejects_different_keys() {
        let temp = tempdir().expect("tempdir");
        let first_target = RunTarget::from_parts(
            "Demo/key",
            PathBuf::from("/repo"),
            PathBuf::from("/git/one"),
        );
        let cases = ["demo-key", "DEMO key"];
        let first = acquire_run_lock_in(&first_target, temp.path()).expect("first claim");
        drop(first);

        for candidate in cases {
            let target =
                RunTarget::from_parts(candidate, PathBuf::from("/repo"), PathBuf::from("/git/one"));
            let error =
                acquire_run_lock_in(&target, temp.path()).expect_err("collision should fail");
            assert_eq!(
                error, "slug demo-key already belongs to key Demo/key",
                "case {candidate}"
            );
        }
    }

    #[test]
    fn same_slug_blocks_until_release() {
        let temp = tempdir().expect("tempdir");
        let target = RunTarget::from_parts(
            "Demo/key",
            PathBuf::from("/repo"),
            PathBuf::from("/git/one"),
        );
        let first = acquire_run_lock_in(&target, temp.path()).expect("first lock");
        let error = acquire_run_lock_in(&target, temp.path()).expect_err("second lock");
        assert!(error.contains("already active"), "{error}");
        drop(first);
        acquire_run_lock_in(&target, temp.path()).expect("released lock");
    }

    #[test]
    fn same_key_reuses_claimed_slug() {
        let temp = tempdir().expect("tempdir");
        let target = RunTarget::from_parts(
            "Demo/key",
            PathBuf::from("/repo"),
            PathBuf::from("/git/one"),
        );
        let first = acquire_run_lock_in(&target, temp.path()).expect("first lock");
        drop(first);
        acquire_run_lock_in(&target, temp.path()).expect("same key should reuse");
        assert_eq!(
            std::fs::read_to_string(target.state_dir(Path::new(temp.path())).join("key")).unwrap(),
            "Demo/key"
        );
    }
}
