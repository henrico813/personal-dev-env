use std::path::{Path, PathBuf};
use std::process::Command;

pub struct RepoLayout {
    pub repo_root: PathBuf,
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

fn git_paths_z(cwd: &Path, args: &[&str]) -> Result<Vec<String>, String> {
    let out = Command::new("git")
        .args(["-C", cwd.to_str().unwrap_or(".")])
        .args(args)
        .output()
        .map_err(|e| format!("spawn git: {e}"))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    Ok(out
        .stdout
        .split(|byte| *byte == 0)
        .filter(|entry| !entry.is_empty())
        .map(|entry| String::from_utf8_lossy(entry).to_string())
        .collect())
}

fn command_output(out: &std::process::Output) -> String {
    let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();

    match (stdout.is_empty(), stderr.is_empty()) {
        (true, true) => String::new(),
        (false, true) => stdout,
        (true, false) => stderr,
        (false, false) => format!("stdout:\n{stdout}\n\nstderr:\n{stderr}"),
    }
}

pub fn validate_worktree(
    worktree: &Path,
    expected_branch: &str,
    expected_git_common_dir: &Path,
) -> Result<(), String> {
    let inside = git(worktree, &["rev-parse", "--is-inside-work-tree"])?;
    if inside != "true" {
        return Err(format!("not a git worktree: {}", worktree.display()));
    }

    let branch = git(worktree, &["rev-parse", "--abbrev-ref", "HEAD"])?;
    if branch != expected_branch {
        return Err(format!(
            "worktree {} is on branch {branch}, expected {expected_branch}",
            worktree.display()
        ));
    }

    let actual_common_dir = PathBuf::from(git(
        worktree,
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
    )?);
    if actual_common_dir != expected_git_common_dir {
        return Err(format!(
            "worktree {} belongs to {}, expected {}",
            worktree.display(),
            actual_common_dir.display(),
            expected_git_common_dir.display()
        ));
    }
    Ok(())
}

pub fn repo_layout() -> Result<RepoLayout, String> {
    let checkout_root = PathBuf::from(git(Path::new("."), &["rev-parse", "--show-toplevel"])?);
    let git_common_dir = PathBuf::from(git(
        &checkout_root,
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
    )?);
    let repo_root = git_common_dir
        .parent()
        .filter(|_| git_common_dir.file_name().and_then(|name| name.to_str()) == Some(".git"))
        .ok_or_else(|| format!("unexpected git common dir: {}", git_common_dir.display()))?
        .to_path_buf();
    Ok(RepoLayout {
        repo_root,
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
    git_common_dir: &Path,
    remote: &str,
    base_branch: &str,
) -> Result<(), String> {
    if worktree.exists() {
        return validate_worktree(worktree, branch, git_common_dir);
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
    validate_worktree(worktree, branch, git_common_dir)
}

pub fn is_dirty(repo: &Path) -> Result<bool, String> {
    Ok(!git(repo, &["status", "--porcelain"])?.is_empty())
}

pub fn head_sha(repo: &Path) -> Result<String, String> {
    git(repo, &["rev-parse", "HEAD"])
}

/// Final dirty worktree reporting must include both tracked diffs and
/// untracked files so callers can describe terminal state truthfully.
pub fn changed_files_in_worktree(repo: &Path) -> Result<Vec<String>, String> {
    let mut files = git_paths_z(repo, &["diff", "--name-only", "-z", "HEAD"])?;
    files.extend(git_paths_z(
        repo,
        &["ls-files", "--others", "--exclude-standard", "-z"],
    )?);
    files.sort();
    files.dedup();
    Ok(files)
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
    if !add.status.success() {
        let output = command_output(&add);
        return Err(if output.is_empty() {
            "git add -A failed".to_string()
        } else {
            format!("git add -A failed:\n{output}")
        });
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
    if !commit.status.success() {
        let output = command_output(&commit);
        return Err(if output.is_empty() {
            "git commit failed".to_string()
        } else {
            format!("git commit failed:\n{output}")
        });
    }
    head_sha(repo)
}

#[cfg(test)]
mod tests {
    use super::changed_files_in_worktree;
    use std::process::Command;
    use tempfile::tempdir;

    #[test]
    fn changed_files_in_worktree_includes_tracked_and_untracked_files() {
        let temp = tempdir().expect("tempdir");
        let repo = temp.path();

        let init = Command::new("git")
            .args(["init"])
            .current_dir(repo)
            .output()
            .expect("git init");
        assert!(
            init.status.success(),
            "{}",
            String::from_utf8_lossy(&init.stderr)
        );

        let config_name = Command::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(repo)
            .output()
            .expect("git config name");
        assert!(config_name.status.success());

        let config_email = Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(repo)
            .output()
            .expect("git config email");
        assert!(config_email.status.success());

        std::fs::write(repo.join("tracked.txt"), "one\n").expect("write tracked file");

        let add = Command::new("git")
            .args(["add", "tracked.txt"])
            .current_dir(repo)
            .output()
            .expect("git add");
        assert!(add.status.success());

        let commit = Command::new("git")
            .args(["commit", "-m", "seed"])
            .current_dir(repo)
            .output()
            .expect("git commit");
        assert!(
            commit.status.success(),
            "{}",
            String::from_utf8_lossy(&commit.stderr)
        );

        std::fs::write(repo.join("tracked.txt"), "one\ntwo\n").expect("modify tracked");
        std::fs::write(repo.join("untracked.txt"), "hello\n").expect("write untracked");

        let changed = changed_files_in_worktree(repo).expect("changed files");

        assert_eq!(
            changed,
            vec!["tracked.txt".to_string(), "untracked.txt".to_string()]
        );
    }
}
