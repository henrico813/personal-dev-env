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

pub fn create_run_paths(repo_root: &Path, branch: &str, step: u32) -> Result<RunPaths, String> {
    let home = std::env::var("HOME").map_err(|_| "HOME not set".to_string())?;
    let repo_id = repo_root
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("repo");
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_secs();
    let run_id = format!("{}-{}-{}", step, ts, std::process::id());
    let dir = PathBuf::from(home)
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
