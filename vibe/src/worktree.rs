use std::path::{Path, PathBuf};

use crate::adapters::git;

pub struct WorktreeSession {
    pub key: String,
    pub slug: String,
    pub branch: String,
    pub worktree: PathBuf,
    pub canonical_repo_root: PathBuf,
    pub git_common_dir: PathBuf,
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

pub fn prepare(key: &str) -> Result<WorktreeSession, String> {
    let repo = git::repo_layout()?;
    let slug = slugify(key);
    let branch = format!("vibe/{slug}");
    let worktree = repo.canonical_repo_root.join("worktrees").join(&slug);
    let (remote, base_branch) = git::resolve_base(&repo.canonical_repo_root)?;
    git::ensure_worktree(
        &repo.canonical_repo_root,
        &worktree,
        &branch,
        &remote,
        &base_branch,
    )?;
    Ok(WorktreeSession {
        key: key.to_string(),
        slug,
        branch,
        worktree,
        canonical_repo_root: repo.canonical_repo_root,
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

pub fn is_dirty(worktree: &Path) -> bool {
    git::is_dirty(worktree).unwrap_or(false)
}

pub fn commit_result(worktree: &Path, message: &str, hooks_dir: &Path) -> Result<String, String> {
    git::commit_all(worktree, message, hooks_dir, "result")
}

#[cfg(test)]
mod tests {
    use super::slugify;

    #[test]
    fn slugify_normalizes_keys() {
        assert_eq!(slugify("PDEV-049 demo/key"), "pdev-049-demo-key");
    }

    #[test]
    fn empty_slug_falls_back() {
        assert_eq!(slugify("---"), "vibe");
    }
}
