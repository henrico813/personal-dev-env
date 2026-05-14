use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

use crate::{
    observe::ArtifactPaths,
    result::{RunResult, Status},
    state::{self, PersistedRunState, RunPhase},
};

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
    #[serde(default)]
    pub created_at: u64,
    pub status: Status,
    pub branch: Option<String>,
    pub worktree: Option<String>,
    pub model: Option<String>,
    pub pre_run_commit: Option<String>,
    pub commit: Option<String>,
    pub snapshot_commits: Vec<String>,
    pub changed_files: Vec<String>,
    pub artifacts_dir: String,
    #[serde(default)]
    pub summary_path: String,
    #[serde(default)]
    pub result_path: String,
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
            created_at: 0,
            status: result.status.clone(),
            branch: result.branch.clone(),
            worktree: result.worktree.clone(),
            model: result.model.clone(),
            pre_run_commit: result.pre_run_commit.clone(),
            commit: result.commit.clone(),
            snapshot_commits: result.snapshot_commits.clone(),
            changed_files,
            artifacts_dir: result.artifacts_dir.clone().unwrap_or_default(),
            summary_path: result.summary_path.clone().unwrap_or_default(),
            result_path: String::new(),
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

fn write_json_atomic<T: Serialize>(path: &Path, value: &T, label: &str) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("{label} path has no parent: {}", path.display()))?;
    fs::create_dir_all(parent).map_err(|e| format!("create {label} parent: {e}"))?;
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let tmp = parent.join(format!(".{label}.{ts}.tmp"));
    let json =
        serde_json::to_string_pretty(value).map_err(|e| format!("serialize {label}: {e}"))?;
    fs::write(&tmp, json).map_err(|e| format!("write {label} temp: {e}"))?;
    fs::rename(&tmp, path).map_err(|e| format!("rename {label} temp: {e}"))?;
    Ok(())
}

fn write_summary(path: &Path, summary: &RunSummary) -> Result<(), String> {
    write_json_atomic(path, summary, "summary")
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn append_run_index(path: &Path, entry: &RunIndexEntry, log_path: &Path) -> Result<(), String> {
    let existing = read_runs_index(path)?;
    if existing.iter().any(|it| it.run_id == entry.run_id) {
        append_log(
            log_path,
            &format!("skip duplicate runs index append for {}", entry.run_id),
        )?;
        return Ok(());
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
        serde_json::to_string(entry).map_err(|e| format!("serialize runs index entry: {e}"))?;
    writeln!(file, "{line}").map_err(|e| format!("append runs index: {e}"))?;
    Ok(())
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn state_path_from_summary(summary_path: &Path) -> Option<String> {
    Some(
        summary_path
            .to_str()?
            .replace(SUMMARY_FILE, "run-state.json"),
    )
}

fn state_path_from_result(result: &RunResult) -> Option<PathBuf> {
    result
        .summary_path
        .as_deref()
        .and_then(|path| state_path_from_summary(Path::new(path)).map(PathBuf::from))
        .or_else(|| {
            result
                .artifacts_dir
                .as_deref()
                .map(|dir| Path::new(dir).join("run-state.json"))
        })
}

fn read_summary(path: &Path) -> Result<RunSummary, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("read summary: {e}"))?;
    serde_json::from_str(&text).map_err(|e| format!("parse summary: {e}"))
}

fn merge_persistence_error(existing: Option<&str>, next: &str) -> String {
    match existing {
        Some(current) if current == next => current.to_string(),
        Some(current) => format!("{current}; {next}"),
        None => next.to_string(),
    }
}

pub fn record_late_persistence_error(
    result: &mut RunResult,
    message: String,
) -> Result<(), String> {
    let merged = merge_persistence_error(result.persistence_error.as_deref(), &message);
    result.persistence_error = Some(merged.clone());

    let mut repair_errors = Vec::new();

    if let Some(state_path) = state_path_from_result(result) {
        match state::read(&state_path) {
            Ok(mut persisted) => {
                persisted.persistence_error = Some(merged.clone());
                if let Err(err) = state::write(&state_path, &persisted) {
                    repair_errors.push(format!("write run state: {err}"));
                }
            }
            Err(err) => repair_errors.push(format!("read run state: {err}")),
        }
    }

    if let Some(summary_path) = result.summary_path.as_deref() {
        let summary_path = Path::new(summary_path);
        match read_summary(summary_path) {
            Ok(mut summary) => {
                summary.persistence_error = Some(merged.clone());
                if let Err(err) = write_summary(summary_path, &summary) {
                    repair_errors.push(format!("write summary: {err}"));
                }
            }
            Err(err) => repair_errors.push(err),
        }
    }

    if repair_errors.is_empty() {
        Ok(())
    } else {
        Err(repair_errors.join("; "))
    }
}

fn run_summary(
    artifacts: &ArtifactPaths,
    persisted: &PersistedRunState,
    result: &RunResult,
) -> RunSummary {
    RunSummary {
        run_id: persisted.run_id.clone(),
        key: persisted.key.clone(),
        slug: persisted.slug.clone(),
        created_at: persisted.created_at,
        status: result.status.clone(),
        branch: persisted.branch.clone(),
        worktree: persisted.worktree.clone(),
        model: persisted.model.clone(),
        pre_run_commit: result.pre_run_commit.clone(),
        commit: result.commit.clone(),
        snapshot_commits: result.snapshot_commits.clone(),
        changed_files: result.changed_files.clone(),
        artifacts_dir: artifacts.dir.display().to_string(),
        summary_path: artifacts.summary_json.display().to_string(),
        result_path: artifacts.result_json.display().to_string(),
        events_log_path: artifacts.events_jsonl.display().to_string(),
        stderr_path: artifacts.stderr_log.display().to_string(),
        error_message: result.error_message.clone(),
        persistence_error: result.persistence_error.clone(),
    }
}

pub fn persist_terminal_run(
    artifacts: &ArtifactPaths,
    persisted: &mut PersistedRunState,
    result: &mut RunResult,
) -> Result<(), String> {
    let summary = run_summary(artifacts, persisted, result);
    write_summary(&artifacts.summary_json, &summary)?;

    let mut state_copy = persisted.clone();
    state_copy.phase = RunPhase::Finished;
    state_copy.terminal_status = Some(result.status.clone());
    state_copy.pre_run_commit = result.pre_run_commit.clone();
    state_copy.commit = result.commit.clone();
    state_copy.snapshot_commits = result.snapshot_commits.clone();
    state_copy.changed_files = result.changed_files.clone();
    state_copy.persistence_error = result.persistence_error.clone();
    state_copy.error_message = result.error_message.clone();
    state::write(&artifacts.state_json, &state_copy)?;
    *persisted = state_copy;

    if let Err(err) = append_run_index(
        &artifacts.runs_index_jsonl,
        &RunIndexEntry {
            run_id: persisted.run_id.clone(),
            created_at: persisted.created_at,
            state_path: artifacts.state_json.display().to_string(),
            summary_path: artifacts.summary_json.display().to_string(),
        },
        &artifacts.vibe_log,
    ) {
        record_late_persistence_error(result, format!("append runs_index.jsonl: {err}"))?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{record_late_persistence_error, RunSummary};
    use crate::{
        result::{RunResult, Status},
        state::{self, PersistedRunState, RunPhase},
    };
    use tempfile::tempdir;

    fn sample_result(artifacts_dir: &std::path::Path, summary_path: &std::path::Path) -> RunResult {
        RunResult {
            run_id: Some("run-id".to_string()),
            status: Status::Completed,
            branch: Some("vibe/pdev-099b".to_string()),
            worktree: Some("/tmp/worktree".to_string()),
            model: Some("openai-codex/gpt-5.4-mini".to_string()),
            pre_run_commit: Some("abc".to_string()),
            commit: Some("def".to_string()),
            snapshot_commits: vec!["snap".to_string()],
            artifacts_dir: Some(artifacts_dir.display().to_string()),
            events_log_path: Some(artifacts_dir.join("events.jsonl").display().to_string()),
            stderr_path: Some(artifacts_dir.join("agent.stderr.log").display().to_string()),
            summary_path: Some(summary_path.display().to_string()),
            changed_files: vec!["vibe/src/ledger.rs".to_string()],
            persistence_error: None,
            error_message: None,
        }
    }

    fn sample_state(
        summary_path: &std::path::Path,
        artifacts_dir: &std::path::Path,
    ) -> PersistedRunState {
        PersistedRunState {
            run_id: "run-id".to_string(),
            key: "pdev-099b".to_string(),
            slug: "pdev-099b".to_string(),
            created_at: 1778781727,
            branch: Some("vibe/pdev-099b".to_string()),
            worktree: Some("/tmp/worktree".to_string()),
            model: Some("openai-codex/gpt-5.4-mini".to_string()),
            phase: RunPhase::Finished,
            terminal_status: Some(Status::Completed),
            pre_run_commit: Some("abc".to_string()),
            commit: Some("def".to_string()),
            snapshot_commits: vec!["snap".to_string()],
            artifacts_dir: Some(artifacts_dir.display().to_string()),
            events_log_path: Some(artifacts_dir.join("events.jsonl").display().to_string()),
            stderr_path: Some(artifacts_dir.join("agent.stderr.log").display().to_string()),
            result_path: Some(artifacts_dir.join("result.json").display().to_string()),
            wrapper_log_path: Some(artifacts_dir.join("vibe.log").display().to_string()),
            summary_path: Some(summary_path.display().to_string()),
            changed_files: vec!["vibe/src/ledger.rs".to_string()],
            error_message: None,
            persistence_error: None,
        }
    }

    #[test]
    fn late_persistence_error_updates_run_state_when_summary_is_missing() {
        let temp = tempdir().expect("tempdir");
        let artifacts_dir = temp.path().join("run");
        std::fs::create_dir_all(&artifacts_dir).expect("artifacts dir");
        let summary_path = artifacts_dir.join("summary.json");
        let state_path = artifacts_dir.join("run-state.json");
        let state = sample_state(&summary_path, &artifacts_dir);
        state::write(&state_path, &state).expect("write state");
        let mut result = sample_result(&artifacts_dir, &summary_path);

        let err =
            record_late_persistence_error(&mut result, "append runs_index.jsonl: boom".to_string())
                .expect_err("missing summary should still report repair error");

        assert!(err.starts_with("read summary:"));
        assert_eq!(
            result.persistence_error.as_deref(),
            Some("append runs_index.jsonl: boom")
        );

        let repaired = state::read(&state_path).expect("read repaired state");
        assert_eq!(
            repaired.persistence_error.as_deref(),
            Some("append runs_index.jsonl: boom")
        );
    }

    #[test]
    fn run_summary_round_trip_preserves_new_terminal_fields() {
        let temp = tempdir().expect("tempdir");
        let artifacts_dir = temp.path().join("run");
        std::fs::create_dir_all(&artifacts_dir).expect("artifacts dir");
        let summary_path = artifacts_dir.join("summary.json");
        let result = sample_result(&artifacts_dir, &summary_path);
        let summary = RunSummary::from_run_result(
            "run-id",
            "pdev-099b",
            "pdev-099b",
            &result,
            vec!["vibe/src/ledger.rs".to_string()],
            Some("boom".to_string()),
        );

        let value = serde_json::to_value(summary).expect("serialize summary");

        assert_eq!(value["summary_path"], summary_path.display().to_string());
        assert_eq!(value["result_path"], "");
        assert_eq!(value["created_at"], 0);
        assert_eq!(value["persistence_error"], "boom");
    }
}
