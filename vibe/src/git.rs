use std::path::{Path, PathBuf};
use std::process::Command;

pub struct RepoLayout {
    pub canonical_repo_root: PathBuf,
    pub git_common_dir: PathBuf,
}

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

pub fn repo_layout() -> Result<RepoLayout, String> {
    let checkout_root = PathBuf::from(git(Path::new("."), &["rev-parse", "--show-toplevel"])?);
    let git_common_dir = PathBuf::from(git(
        &checkout_root,
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
    )?);
    let canonical_repo_root = git_common_dir
        .parent()
        .filter(|_| git_common_dir.file_name().and_then(|name| name.to_str()) == Some(".git"))
        .ok_or_else(|| format!("unexpected git common dir: {}", git_common_dir.display()))?
        .to_path_buf();
    Ok(RepoLayout {
        canonical_repo_root,
        git_common_dir,
    })
}

pub fn resolve_base(repo_root: &Path) -> Result<(String, String), String> {
    let remotes = git(repo_root, &["remote"])?;
    for name in ["origin", "github", "goog"] {
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

pub fn find_vibe_step_commit(repo: &Path, step: u32) -> Result<Option<String>, String> {
    let log = git(repo, &["log", "--format=%H%x00%s"])?;
    Ok(find_vibe_step_commit_in_log(&log, step))
}

fn find_vibe_step_commit_in_log(log: &str, step: u32) -> Option<String> {
    let prefix = format!("vibe: step {step} ");
    log.lines().find_map(|line| {
        let (sha, subject) = line.split_once('\0')?;
        subject.starts_with(&prefix).then(|| sha.to_string())
    })
}

/// Result commits are the canonical run result; snapshot hooks ignore this kind.
pub fn commit_all(
    repo: &Path,
    message: &str,
    hooks_dir: &Path,
    kind: &str,
) -> Result<String, String> {
    let add = Command::new("git")
        .args(["-C", repo.to_str().unwrap_or("."), "add", "-A"])
        .output()
        .map_err(|e| format!("git add: {e}"))?;
    if !add.stdout.is_empty() {
        eprint!("{}", String::from_utf8_lossy(&add.stdout));
    }
    if !add.stderr.is_empty() {
        eprint!("{}", String::from_utf8_lossy(&add.stderr));
    }
    if !add.status.success() {
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
        .output()
        .map_err(|e| format!("git commit: {e}"))?;
    if !commit.stdout.is_empty() {
        eprint!("{}", String::from_utf8_lossy(&commit.stdout));
    }
    if !commit.stderr.is_empty() {
        eprint!("{}", String::from_utf8_lossy(&commit.stderr));
    }
    if !commit.status.success() {
        return Err("git commit failed".to_string());
    }
    head_sha(repo)
}

#[cfg(test)]
mod tests {
    use super::find_vibe_step_commit_in_log;

    #[test]
    fn finds_matching_step_commit() {
        let log = "abc123\0vibe: step 5 Add inline config\n";

        assert_eq!(
            find_vibe_step_commit_in_log(log, 5),
            Some("abc123".to_string())
        );
    }

    #[test]
    fn does_not_match_step_prefixes() {
        let log = "abc123\0vibe: step 50 Add later config\n";

        assert_eq!(find_vibe_step_commit_in_log(log, 5), None);
    }

    #[test]
    fn ignores_non_vibe_commits() {
        let log = "abc123\0fix(nvim): restore inline config\n";

        assert_eq!(find_vibe_step_commit_in_log(log, 5), None);
    }
}
