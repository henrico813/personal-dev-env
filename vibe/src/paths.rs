use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Artifact paths for one Vibe run.
pub struct RunPaths {
    pub dir: PathBuf,
    pub step_json: PathBuf,
    pub prompt_txt: PathBuf,
    pub events_jsonl: PathBuf,
    pub stderr_log: PathBuf,
    pub snapshots_jsonl: PathBuf,
}

pub fn branch_slug(plan: &Path) -> String {
    let stem = plan.file_stem().and_then(|s| s.to_str()).unwrap_or("vibe");
    let mut out = String::new();
    let mut dash = false;
    for ch in stem.to_lowercase().chars() {
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

pub fn worktree_path(repo_root: &Path, slug: &str) -> PathBuf {
    repo_root.join("worktrees").join(slug)
}

fn create_run_paths_in(
    home: &Path,
    repo_root: &Path,
    branch: &str,
    run_id: &str,
) -> Result<RunPaths, String> {
    let repo_id = repo_root
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("repo");
    let dir = home
        .join(".local/state/vibe")
        .join(repo_id)
        .join(branch)
        .join("runs")
        .join(run_id);
    fs::create_dir_all(&dir).map_err(|e| format!("create run dir: {e}"))?;
    Ok(RunPaths {
        dir: dir.clone(),
        step_json: dir.join("step.json"),
        prompt_txt: dir.join("prompt.txt"),
        events_jsonl: dir.join("events.jsonl"),
        stderr_log: dir.join("agent.stderr.log"),
        snapshots_jsonl: dir.join("snapshots.jsonl"),
    })
}

pub fn create_run_paths(repo_root: &Path, branch: &str, step: u32) -> Result<RunPaths, String> {
    let home = std::env::var("HOME").map_err(|_| "HOME not set".to_string())?;
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_secs();
    let run_id = format!("{}-{}-{}", step, ts, std::process::id());
    create_run_paths_in(Path::new(&home), repo_root, branch, &run_id)
}

#[cfg(test)]
mod tests {
    use super::{branch_slug, create_run_paths_in, worktree_path};
    use std::path::Path;
    use tempfile::tempdir;

    #[test]
    fn branch_slug_normalizes_names() {
        let slug = branch_slug(Path::new(
            "/tmp/PDEV-042 Add Vibe characterization tests!!!.md",
        ));

        assert_eq!(slug, "pdev-042-add-vibe-characterization-tests");
    }

    #[test]
    fn empty_slug_falls_back() {
        assert_eq!(branch_slug(Path::new("/tmp/---.md")), "vibe");
    }

    #[test]
    fn worktree_path_uses_repo_root() {
        let worktree = worktree_path(Path::new("/repo/root"), "slug");

        assert_eq!(worktree, Path::new("/repo/root/worktrees/slug"));
    }

    #[test]
    fn run_paths_use_expected_layout() {
        let temp = tempdir().expect("tempdir");
        let repo_root = temp.path().join("personal-dev-env");
        std::fs::create_dir_all(&repo_root).expect("repo dir");

        let paths = create_run_paths_in(
            temp.path(),
            &repo_root,
            "pdev-042-add-vibe-characterization-tests",
            "1-1700000000-4242",
        )
        .expect("run paths");

        let dir = temp
            .path()
            .join(".local/state/vibe/personal-dev-env")
            .join("pdev-042-add-vibe-characterization-tests/runs/1-1700000000-4242");
        assert_eq!(paths.dir, dir);
        assert_eq!(paths.step_json, dir.join("step.json"));
        assert_eq!(paths.prompt_txt, dir.join("prompt.txt"));
        assert_eq!(paths.events_jsonl, dir.join("events.jsonl"));
        assert_eq!(paths.stderr_log, dir.join("agent.stderr.log"));
        assert_eq!(paths.snapshots_jsonl, dir.join("snapshots.jsonl"));
    }
}
