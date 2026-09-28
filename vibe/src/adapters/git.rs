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

fn remote_exists(repo_root: &Path, name: &str) -> Result<bool, String> {
    Ok(git(repo_root, &["remote"])?
        .lines()
        .any(|remote| remote == name))
}

fn resolve_new_branch_base(repo_root: &Path, base: Option<&str>) -> Result<String, String> {
    let base_ref = match base {
        Some(base) => {
            if let Some((remote, branch)) = base.split_once('/') {
                if remote_exists(repo_root, remote)? {
                    git(repo_root, &["fetch", remote, branch])?;
                }
            }
            base.to_string()
        }
        None => {
            let (remote, branch) = resolve_base(repo_root)?;
            git(repo_root, &["fetch", &remote, &branch])?;
            format!("{remote}/{branch}")
        }
    };
    git(
        repo_root,
        &["rev-parse", "--verify", &format!("{base_ref}^{{commit}}")],
    )?;
    Ok(base_ref)
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

pub fn validate_base_target(
    repo_root: &Path,
    worktree: &Path,
    branch: &str,
    base: Option<&str>,
) -> Result<(), String> {
    if base.is_none() {
        return Ok(());
    }
    if worktree.exists() {
        return Err(format!(
            "--base requires a new managed worktree; already exists: {}",
            worktree.display()
        ));
    }
    if branch_exists(repo_root, branch)? {
        return Err(format!(
            "--base requires a new managed branch; already exists: {branch}"
        ));
    }
    Ok(())
}

/// Worktrees are the durable branch state; Docker is only the execution boundary.
pub fn ensure_worktree(
    repo_root: &Path,
    worktree: &Path,
    branch: &str,
    git_common_dir: &Path,
    base: Option<&str>,
) -> Result<(), String> {
    validate_base_target(repo_root, worktree, branch, base)?;
    if base.is_some() {
        let base_ref = resolve_new_branch_base(repo_root, base)?;
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
        return validate_worktree(worktree, branch, git_common_dir);
    }

    let existing_branch = branch_exists(repo_root, branch)?;
    if worktree.exists() {
        return validate_worktree(worktree, branch, git_common_dir);
    }
    if existing_branch {
        git(
            repo_root,
            &["worktree", "add", worktree.to_str().unwrap_or(""), branch],
        )?;
    } else {
        let base_ref = resolve_new_branch_base(repo_root, None)?;
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

pub fn validate_repository_skills(repo: &Path) -> Result<(), String> {
    let tracked = git_paths_z(
        repo,
        &["ls-files", "--stage", "-v", "-z", "--", ".agents/skills"],
    )?;
    if tracked.is_empty() {
        return Err("repository skills must contain files tracked by HEAD".to_string());
    }
    for entry in &tracked {
        let (index, _path) = entry
            .split_once('\t')
            .ok_or_else(|| "read repository skills index metadata".to_string())?;
        let mut fields = index.split_whitespace();
        let tag = fields.next();
        let mode = fields.next();
        let _object = fields.next();
        let stage = fields.next();
        if tag != Some("H") {
            return Err(
                "repository skills cannot use assume-unchanged or skip-worktree index flags"
                    .to_string(),
            );
        }
        if !matches!(mode, Some("100644" | "100755")) || stage != Some("0") {
            return Err("repository skills must contain only regular tracked files".to_string());
        }
    }

    let changes = git_paths_z(
        repo,
        &[
            "status",
            "--porcelain=v1",
            "-z",
            "--ignored=matching",
            "--untracked-files=all",
            "--",
            ".agents/skills",
        ],
    )?;
    if changes.is_empty() {
        Ok(())
    } else {
        Err("repository skills must match HEAD without ignored or untracked files".to_string())
    }
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
    use super::{changed_files_in_worktree, ensure_worktree, head_sha, validate_repository_skills};
    use std::path::Path;
    use std::process::Command;
    use tempfile::{tempdir, TempDir};

    fn run(repo: &Path, args: &[&str]) {
        let output = Command::new("git")
            .args(["-C", repo.to_str().unwrap_or(".")])
            .args(args)
            .output()
            .expect("git command");
        assert!(
            output.status.success(),
            "{} {}: {}",
            repo.display(),
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn output(repo: &Path, args: &[&str]) -> String {
        let result = Command::new("git")
            .args(["-C", repo.to_str().unwrap_or(".")])
            .args(args)
            .output()
            .expect("git command");
        assert!(result.status.success());
        String::from_utf8_lossy(&result.stdout).trim().to_string()
    }

    fn setup_repo() -> (TempDir, std::path::PathBuf, std::path::PathBuf) {
        let temp = tempdir().expect("tempdir");
        let remote = temp.path().join("remote.git");
        let repo = temp.path().join("repo");
        run(temp.path(), &["init", "--bare", remote.to_str().unwrap()]);
        let clone = Command::new("git")
            .args(["clone", remote.to_str().unwrap(), repo.to_str().unwrap()])
            .output()
            .expect("git clone");
        assert!(clone.status.success());
        run(&repo, &["config", "user.name", "Test User"]);
        run(&repo, &["config", "user.email", "test@example.com"]);
        std::fs::write(repo.join("seed.txt"), "seed\n").expect("write seed");
        run(&repo, &["add", "seed.txt"]);
        run(&repo, &["commit", "-m", "seed"]);
        run(&repo, &["branch", "-M", "main"]);
        run(&repo, &["push", "-u", "origin", "main"]);
        run(&repo, &["checkout", "-b", "feature"]);
        std::fs::write(repo.join("feature.txt"), "feature\n").expect("write feature");
        run(&repo, &["add", "feature.txt"]);
        run(&repo, &["commit", "-m", "feature"]);
        run(&repo, &["push", "origin", "feature"]);
        run(&repo, &["checkout", "main"]);
        run(&repo, &["update-ref", "-d", "refs/remotes/origin/feature"]);
        (temp, repo, remote)
    }

    fn common_dir(repo: &Path) -> std::path::PathBuf {
        repo.join(".git")
    }

    fn setup_repository_skills() -> (TempDir, std::path::PathBuf) {
        let temp = tempdir().expect("tempdir");
        let repo = temp.path().join("repo");
        run(temp.path(), &["init", repo.to_str().unwrap()]);
        run(&repo, &["config", "user.name", "Test User"]);
        run(&repo, &["config", "user.email", "test@example.com"]);
        let skill = repo.join(".agents/skills/reviewed/SKILL.md");
        std::fs::create_dir_all(skill.parent().expect("skill parent")).expect("create skills");
        std::fs::write(&skill, "reviewed\n").expect("write skill");
        run(&repo, &["add", ".agents/skills"]);
        run(
            &repo,
            &[
                "-c",
                "core.hooksPath=/dev/null",
                "commit",
                "-m",
                "add skill",
            ],
        );
        (temp, repo)
    }

    fn break_remote(repo: &Path) {
        run(repo, &["remote", "set-url", "origin", "/missing/remote"]);
    }

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

        let hooks_dir = repo.join("hooks");
        std::fs::create_dir(&hooks_dir).expect("create hooks directory");
        run(
            repo,
            &["config", "core.hooksPath", hooks_dir.to_str().unwrap()],
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

    #[test]
    fn repository_skills_accept_clean_head() {
        let (_temp, repo) = setup_repository_skills();

        validate_repository_skills(&repo).expect("clean tracked skills");
    }

    #[test]
    fn repository_skills_require_tracked_files() {
        let temp = tempdir().expect("tempdir");
        let repo = temp.path();
        run(repo, &["init"]);
        std::fs::create_dir_all(repo.join(".agents/skills/local")).expect("create skills");
        std::fs::write(repo.join(".agents/skills/local/SKILL.md"), "local\n").expect("write skill");

        let error = validate_repository_skills(repo).expect_err("untracked skills must fail");

        assert!(error.contains("tracked by HEAD"), "{error}");
    }

    #[test]
    fn repository_skills_reject_untracked_extras() {
        let (_temp, repo) = setup_repository_skills();
        std::fs::write(repo.join(".agents/skills/local.txt"), "local\n")
            .expect("write untracked file");

        let error = validate_repository_skills(&repo).expect_err("untracked files must fail");

        assert!(error.contains("must match HEAD"), "{error}");
    }

    #[test]
    fn repository_skills_reject_assume_unchanged() {
        let (_temp, repo) = setup_repository_skills();
        run(
            &repo,
            &[
                "update-index",
                "--assume-unchanged",
                ".agents/skills/reviewed/SKILL.md",
            ],
        );
        std::fs::write(repo.join(".agents/skills/reviewed/SKILL.md"), "changed\n")
            .expect("change skill");

        let error = validate_repository_skills(&repo).expect_err("index flag must fail");

        assert!(error.contains("assume-unchanged"), "{error}");
    }

    #[test]
    fn repository_skills_reject_skip_worktree() {
        let (_temp, repo) = setup_repository_skills();
        run(
            &repo,
            &[
                "update-index",
                "--skip-worktree",
                ".agents/skills/reviewed/SKILL.md",
            ],
        );
        std::fs::write(repo.join(".agents/skills/reviewed/SKILL.md"), "changed\n")
            .expect("change skill");

        let error = validate_repository_skills(&repo).expect_err("index flag must fail");

        assert!(error.contains("skip-worktree"), "{error}");
    }

    #[test]
    fn repository_skills_reject_ignored_files() {
        let (_temp, repo) = setup_repository_skills();
        std::fs::write(repo.join(".gitignore"), ".agents/skills/local.txt\n")
            .expect("write ignore rule");
        run(&repo, &["add", ".gitignore"]);
        run(
            &repo,
            &[
                "-c",
                "core.hooksPath=/dev/null",
                "commit",
                "-m",
                "ignore local skill file",
            ],
        );
        std::fs::write(repo.join(".agents/skills/local.txt"), "local\n")
            .expect("write ignored file");

        let error = validate_repository_skills(&repo).expect_err("ignored files must fail");

        assert!(error.contains("must match HEAD"), "{error}");
    }

    #[test]
    fn repository_skills_reject_tracked_changes() {
        let (_temp, repo) = setup_repository_skills();
        std::fs::write(repo.join(".agents/skills/reviewed/SKILL.md"), "changed\n")
            .expect("change skill");

        let error = validate_repository_skills(&repo).expect_err("changed skills must fail");

        assert!(error.contains("must match HEAD"), "{error}");
    }

    // A requested remote branch may not be present in the clone yet.
    #[test]
    fn vibe_uses_requested_remote_branch() {
        let (_temp, repo, _remote) = setup_repo();
        let worktree = repo.join("worktrees/remote-feature");
        std::fs::create_dir_all(worktree.parent().expect("worktree parent")).expect("mkdir");

        ensure_worktree(
            &repo,
            &worktree,
            "vibe/remote-feature",
            &common_dir(&repo),
            Some("origin/feature"),
        )
        .expect("create worktree");

        assert_eq!(
            head_sha(&worktree).expect("worktree head"),
            output(&repo, &["rev-parse", "origin/feature"])
        );
    }

    // The default remains remote main rather than the caller's HEAD.
    #[test]
    fn vibe_uses_main_without_base() {
        let (_temp, repo, _remote) = setup_repo();
        let remote_main = output(&repo, &["rev-parse", "origin/main"]);
        std::fs::write(repo.join("local.txt"), "local\n").expect("write local");
        run(&repo, &["add", "local.txt"]);
        run(&repo, &["commit", "-m", "local"]);
        let worktree = repo.join("worktrees/default-main");
        std::fs::create_dir_all(worktree.parent().expect("worktree parent")).expect("mkdir");

        ensure_worktree(
            &repo,
            &worktree,
            "vibe/default-main",
            &common_dir(&repo),
            None,
        )
        .expect("create worktree");

        assert_eq!(head_sha(&worktree).expect("worktree head"), remote_main);
    }

    // Reusing a key works offline because its worktree already exists.
    #[test]
    fn vibe_reuses_existing_worktree_without_fetch() {
        let (_temp, repo, _remote) = setup_repo();
        let worktree = repo.join("worktrees/reuse");
        std::fs::create_dir_all(worktree.parent().expect("worktree parent")).expect("mkdir");
        ensure_worktree(&repo, &worktree, "vibe/reuse", &common_dir(&repo), None)
            .expect("create worktree");
        break_remote(&repo);

        ensure_worktree(&repo, &worktree, "vibe/reuse", &common_dir(&repo), None)
            .expect("reuse worktree");
    }

    // A detached managed worktree can reattach without remote access.
    #[test]
    fn vibe_reuses_existing_branch_without_fetch() {
        let (_temp, repo, _remote) = setup_repo();
        let worktree = repo.join("worktrees/reuse-branch");
        std::fs::create_dir_all(worktree.parent().expect("worktree parent")).expect("mkdir");
        ensure_worktree(
            &repo,
            &worktree,
            "vibe/reuse-branch",
            &common_dir(&repo),
            None,
        )
        .expect("create worktree");
        run(&repo, &["worktree", "remove", worktree.to_str().unwrap()]);
        break_remote(&repo);

        ensure_worktree(
            &repo,
            &worktree,
            "vibe/reuse-branch",
            &common_dir(&repo),
            None,
        )
        .expect("reuse branch");
    }

    #[test]
    fn base_rejects_existing_worktree() {
        let (_temp, repo, _remote) = setup_repo();
        let worktree = repo.join("worktrees/existing-worktree");
        std::fs::create_dir_all(worktree.parent().expect("worktree parent")).expect("mkdir");
        ensure_worktree(
            &repo,
            &worktree,
            "vibe/existing-worktree",
            &common_dir(&repo),
            None,
        )
        .expect("create worktree");

        let error = ensure_worktree(
            &repo,
            &worktree,
            "vibe/existing-worktree",
            &common_dir(&repo),
            Some("HEAD"),
        )
        .expect_err("reject base");

        assert!(error.contains("new managed worktree"), "{error}");
    }

    #[test]
    fn base_rejects_existing_branch() {
        let (_temp, repo, _remote) = setup_repo();
        let worktree = repo.join("worktrees/existing-branch");
        std::fs::create_dir_all(worktree.parent().expect("worktree parent")).expect("mkdir");
        run(&repo, &["branch", "vibe/existing-branch", "HEAD"]);

        let error = ensure_worktree(
            &repo,
            &worktree,
            "vibe/existing-branch",
            &common_dir(&repo),
            Some("HEAD"),
        )
        .expect_err("reject base");

        assert!(error.contains("new managed branch"), "{error}");
    }
}
