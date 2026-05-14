use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

use crate::result::{RunResult, Status};

#[cfg_attr(not(test), allow(dead_code))]
const SUMMARY_FILE: &str = "summary.json";
const RUNS_INDEX_FILE: &str = "runs_index.jsonl";

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct RunSummary {
    pub run_id: String,
    pub key: String,
    pub slug: String,
    pub status: Status,
    pub branch: Option<String>,
    pub worktree: Option<String>,
    pub model: Option<String>,
    pub pre_run_commit: Option<String>,
    pub commit: Option<String>,
    pub snapshot_commits: Vec<String>,
    pub changed_files: Vec<String>,
    pub artifacts_dir: String,
    pub events_log_path: String,
    pub stderr_path: String,
    pub error_message: Option<String>,
    pub persistence_error: Option<String>,
}

impl RunSummary {
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn from_run_result(
        run_id: &str,
        key: &str,
        slug: &str,
        result: &RunResult,
        changed_files: Vec<String>,
        persistence_error: Option<String>,
    ) -> Self {
        Self {
            run_id: run_id.to_string(),
            key: key.to_string(),
            slug: slug.to_string(),
            status: result.status.clone(),
            branch: result.branch.clone(),
            worktree: result.worktree.clone(),
            model: result.model.clone(),
            pre_run_commit: result.pre_run_commit.clone(),
            commit: result.commit.clone(),
            snapshot_commits: result.snapshot_commits.clone(),
            changed_files,
            artifacts_dir: result.artifacts_dir.clone().unwrap_or_default(),
            events_log_path: result.events_log_path.clone().unwrap_or_default(),
            stderr_path: result.stderr_path.clone().unwrap_or_default(),
            error_message: result.error_message.clone(),
            persistence_error,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct RunIndexEntry {
    pub run_id: String,
    pub created_at: u64,
    pub state_path: String,
    pub summary_path: String,
}

pub fn run_id() -> String {
    Uuid::new_v4().to_string()
}

pub fn created_at() -> Result<u64, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .map_err(|e| e.to_string())
}

fn repo_id(repo_root: &Path) -> String {
    repo_root
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("repo")
        .to_string()
}

pub fn runs_root(home: &Path, repo_root: &Path, slug: &str) -> PathBuf {
    home.join(".local/state/vibe")
        .join(repo_id(repo_root))
        .join(slug)
}

pub fn runs_index_path(home: &Path, repo_root: &Path, slug: &str) -> PathBuf {
    runs_root(home, repo_root, slug).join(RUNS_INDEX_FILE)
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn summary_path(artifacts_dir: &Path) -> PathBuf {
    artifacts_dir.join(SUMMARY_FILE)
}

pub fn read_runs_index(path: &Path) -> Result<Vec<RunIndexEntry>, String> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let text = fs::read_to_string(path).map_err(|e| format!("read runs index: {e}"))?;
    let mut entries = Vec::new();
    for line in text.lines() {
        if let Ok(entry) = serde_json::from_str::<RunIndexEntry>(line) {
            entries.push(entry);
        }
    }
    Ok(entries)
}

#[cfg_attr(not(test), allow(dead_code))]
fn append_log(path: &Path, message: &str) -> Result<(), String> {
    let mut log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| format!("open log for index append: {e}"))?;
    writeln!(log, "{message}").map_err(|e| format!("write log for index append: {e}"))
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn write_summary(path: &Path, summary: &RunSummary) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("summary path has no parent: {}", path.display()))?;
    fs::create_dir_all(parent).map_err(|e| format!("create summary parent: {e}"))?;
    let json =
        serde_json::to_string_pretty(summary).map_err(|e| format!("serialize summary: {e}"))?;
    fs::write(path, json).map_err(|e| format!("write summary: {e}"))
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn append_run_index(
    path: &Path,
    entry: &RunIndexEntry,
    log_path: &Path,
) -> Result<bool, String> {
    if path.exists() {
        for existing in read_runs_index(path)? {
            if existing.run_id == entry.run_id {
                append_log(
                    log_path,
                    &format!(
                        "skip duplicate runs index append for run_id {}",
                        entry.run_id
                    ),
                )?;
                return Ok(false);
            }
        }
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("create runs index parent: {e}"))?;
    }
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| format!("open runs index: {e}"))?;
    let line =
        serde_json::to_string(entry).map_err(|e| format!("serialize run index entry: {e}"))?;
    writeln!(file, "{line}").map_err(|e| format!("append runs index: {e}"))?;
    Ok(true)
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn state_path_from_summary(summary_path: &Path) -> Option<String> {
    Some(
        summary_path
            .to_str()?
            .replace(SUMMARY_FILE, "run-state.json"),
    )
}
