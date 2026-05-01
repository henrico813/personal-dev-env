use std::path::{Path, PathBuf};
use std::process::Command;

fn git(cwd: &Path, args: &[&str]) -> Result<String, String> {
    let out = Command::new("git")
        .args(["-C", cwd.to_str().unwrap_or(".")])
        .args(args)
        .output()
        .map_err(|e| format!("spawn git: {e}"))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

pub fn repo_root() -> Result<PathBuf, String> {
    Ok(PathBuf::from(git(
        Path::new("."),
        &["rev-parse", "--show-toplevel"],
    )?))
}

pub fn resolve_base(repo_root: &Path) -> Result<(String, String), String> {
    let remotes = git(repo_root, &["remote"])?;
    for name in ["origin", "goog", "github"] {
        if remotes.lines().any(|line| line == name) {
            return Ok((name.to_string(), "main".to_string()));
        }
    }
    let first = remotes
        .lines()
        .find(|line| !line.is_empty())
        .ok_or_else(|| "no git remote configured".to_string())?;
    Ok((first.to_string(), "main".to_string()))
}

fn branch_exists(repo_root: &Path, branch: &str) -> Result<bool, String> {
    let status = Command::new("git")
        .args([
            "-C",
            repo_root.to_str().unwrap_or("."),
            "show-ref",
            "--verify",
            "--quiet",
            &format!("refs/heads/{branch}"),
        ])
        .status()
        .map_err(|e| format!("check branch: {e}"))?;
    Ok(status.success())
}

/// Worktrees are the durable branch state; Docker is only the execution boundary.
pub fn ensure_worktree(
    repo_root: &Path,
    worktree: &Path,
    branch: &str,
    remote: &str,
    base_branch: &str,
) -> Result<(), String> {
    if worktree.exists() {
        return Ok(());
    }
    let base_ref = format!("{remote}/{base_branch}");
    git(repo_root, &["fetch", remote, base_branch])?;
    if branch_exists(repo_root, branch)? {
        git(
            repo_root,
            &["worktree", "add", worktree.to_str().unwrap_or(""), branch],
        )?;
    } else {
        git(
            repo_root,
            &[
                "worktree",
                "add",
                "-b",
                branch,
                worktree.to_str().unwrap_or(""),
                &base_ref,
            ],
        )?;
    }
    Ok(())
}

pub fn is_dirty(repo: &Path) -> Result<bool, String> {
    Ok(!git(repo, &["status", "--porcelain"])?.is_empty())
}

pub fn head_sha(repo: &Path) -> Result<String, String> {
    git(repo, &["rev-parse", "HEAD"])
}

/// Final commits are the canonical run result; snapshot hooks ignore this kind.
pub fn commit_all(
    repo: &Path,
    message: &str,
    hooks_dir: &Path,
    kind: &str,
) -> Result<String, String> {
    let add = Command::new("git")
        .args(["-C", repo.to_str().unwrap_or("."), "add", "-A"])
        .status()
        .map_err(|e| format!("git add: {e}"))?;
    if !add.success() {
        return Err("git add -A failed".to_string());
    }
    let commit = Command::new("git")
        .env("VIBE_COMMIT_KIND", kind)
        .args([
            "-C",
            repo.to_str().unwrap_or("."),
            "-c",
            &format!("core.hooksPath={}", hooks_dir.display()),
            "commit",
            "-m",
            message,
        ])
        .status()
        .map_err(|e| format!("git commit: {e}"))?;
    if !commit.success() {
        return Err("git commit failed".to_string());
    }
    head_sha(repo)
}
