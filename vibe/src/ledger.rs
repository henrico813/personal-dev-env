use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

use crate::{
    observe::ArtifactPaths,
    result::{RunResult, Status},
};

const SUMMARY_FILE: &str = "summary.json";
const RUNS_INDEX_FILE: &str = "runs_index.jsonl";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RunPhase {
    PreparingArtifacts,
    CopyingPrompt,
    CheckingDirty,
    ReadingPreRunCommit,
    PreparingSandbox,
    RunningAgent,
    ReadingSnapshots,
    CommittingResult,
    Finished,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct RunSummary {
    pub run_id: String,
    pub key: String,
    pub slug: String,
    #[serde(default)]
    pub created_at: u64,
    pub phase: RunPhase,
    pub status: Option<Status>,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
struct RunRecord {
    pub run_id: String,
    pub key: String,
    pub slug: String,
    pub created_at: u64,
    pub phase: RunPhase,
    pub terminal_status: Option<Status>,
    pub branch: Option<String>,
    pub worktree: Option<String>,
    pub model: Option<String>,
    pub pre_run_commit: Option<String>,
    pub commit: Option<String>,
    pub snapshot_commits: Vec<String>,
    pub changed_files: Vec<String>,
    pub artifacts_dir: String,
    pub run_path: String,
    pub summary_path: String,
    pub result_path: String,
    pub events_log_path: String,
    pub stderr_path: String,
    pub error_message: Option<String>,
    pub persistence_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct RunIndexEntry {
    pub run_id: String,
    pub created_at: u64,
    #[serde(default)]
    pub record_path: String,
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

fn read_run_record(path: &Path) -> Result<RunRecord, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("read run record: {e}"))?;
    serde_json::from_str(&text).map_err(|e| format!("parse run record: {e}"))
}

fn write_run_record(path: &Path, record: &RunRecord) -> Result<(), String> {
    write_json_atomic(path, record, "run")
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

fn append_log(path: &Path, message: &str) -> Result<(), String> {
    let mut log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| format!("open log for index append: {e}"))?;
    writeln!(log, "{message}").map_err(|e| format!("write log for index append: {e}"))
}

fn run_summary(record: &RunRecord) -> RunSummary {
    RunSummary {
        run_id: record.run_id.clone(),
        key: record.key.clone(),
        slug: record.slug.clone(),
        created_at: record.created_at,
        phase: record.phase.clone(),
        status: record.terminal_status.clone(),
        branch: record.branch.clone(),
        worktree: record.worktree.clone(),
        model: record.model.clone(),
        pre_run_commit: record.pre_run_commit.clone(),
        commit: record.commit.clone(),
        snapshot_commits: record.snapshot_commits.clone(),
        changed_files: record.changed_files.clone(),
        artifacts_dir: record.artifacts_dir.clone(),
        summary_path: record.summary_path.clone(),
        result_path: record.result_path.clone(),
        events_log_path: record.events_log_path.clone(),
        stderr_path: record.stderr_path.clone(),
        error_message: record.error_message.clone(),
        persistence_error: record.persistence_error.clone(),
    }
}

fn record_run_persistence_error(
    result: &mut RunResult,
    run_path: &Path,
    message: String,
) -> Result<(), String> {
    let merged = merge_persistence_error(result.persistence_error.as_deref(), &message);
    result.persistence_error = Some(merged.clone());

    let mut record = read_run_record(run_path)?;
    record.persistence_error = Some(merged);
    write_run_record(run_path, &record)
}

// The wrapper already has these values separately at run start, so keep the
// ledger entrypoint flat instead of creating a one-off transport struct.
#[allow(clippy::too_many_arguments)]
pub fn start_run(
    artifacts: &ArtifactPaths,
    key: &str,
    slug: &str,
    branch: &str,
    worktree: &Path,
    model: &str,
    created_at: u64,
    run_id: String,
) -> Result<(), String> {
    let record = RunRecord {
        run_id,
        key: key.to_string(),
        slug: slug.to_string(),
        created_at,
        phase: RunPhase::PreparingArtifacts,
        terminal_status: None,
        branch: Some(branch.to_string()),
        worktree: Some(worktree.display().to_string()),
        model: Some(model.to_string()),
        pre_run_commit: None,
        commit: None,
        snapshot_commits: Vec::new(),
        changed_files: Vec::new(),
        artifacts_dir: artifacts.dir.display().to_string(),
        run_path: artifacts.run_json.display().to_string(),
        summary_path: artifacts.summary_json.display().to_string(),
        result_path: artifacts.result_json.display().to_string(),
        events_log_path: artifacts.events_jsonl.display().to_string(),
        stderr_path: artifacts.stderr_log.display().to_string(),
        error_message: None,
        persistence_error: None,
    };
    write_run_record(&artifacts.run_json, &record)
}

pub fn persist_phase(path: &Path, phase: RunPhase) -> Result<(), String> {
    let mut record = read_run_record(path)?;
    record.phase = phase;
    write_run_record(path, &record)
}

pub fn persist_pre_run_commit(path: &Path, pre_run_commit: &str) -> Result<(), String> {
    let mut record = read_run_record(path)?;
    record.pre_run_commit = Some(pre_run_commit.to_string());
    write_run_record(path, &record)
}

pub fn status_summary_from_path(path: &Path) -> Result<RunSummary, String> {
    read_run_record(path).map(|record| run_summary(&record))
}

pub fn record_json_from_path(path: &Path) -> Result<serde_json::Value, String> {
    let record = read_run_record(path)?;
    serde_json::to_value(record).map_err(|e| format!("serialize run record: {e}"))
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

fn summary_path_from_run_path(run_path: &Path) -> Result<PathBuf, String> {
    let parent = run_path
        .parent()
        .ok_or_else(|| format!("run record path has no parent: {}", run_path.display()))?;
    Ok(parent.join(SUMMARY_FILE))
}

fn rewrite_summary_from_run_path(run_path: &Path, record: &RunRecord) -> Result<(), String> {
    let summary_path = summary_path_from_run_path(run_path)?;
    write_summary(&summary_path, &run_summary(record))
}

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

fn run_path_from_result(result: &RunResult) -> Option<PathBuf> {
    result.run_path.as_deref().map(PathBuf::from).or_else(|| {
        result
            .artifacts_dir
            .as_deref()
            .map(|dir| Path::new(dir).join("run.json"))
    })
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

    if let Some(run_path) = run_path_from_result(result) {
        match read_run_record(&run_path) {
            Ok(mut record) => {
                if let Ok(summary_path) = summary_path_from_run_path(&run_path) {
                    record.summary_path = summary_path.display().to_string();
                }
                record.persistence_error = Some(merged.clone());
                if let Err(err) = write_run_record(&run_path, &record) {
                    repair_errors.push(format!("write run record: {err}"));
                } else if let Err(err) = rewrite_summary_from_run_path(&run_path, &record) {
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

pub fn persist_terminal_run(
    artifacts: &ArtifactPaths,
    result: &mut RunResult,
) -> Result<(), String> {
    let mut record = read_run_record(&artifacts.run_json)?;
    record.phase = RunPhase::Finished;
    record.terminal_status = Some(result.status.clone());
    record.pre_run_commit = result.pre_run_commit.clone();
    record.commit = result.commit.clone();
    record.snapshot_commits = result.snapshot_commits.clone();
    record.changed_files = result.changed_files.clone();
    record.error_message = result.error_message.clone();
    record.persistence_error = result.persistence_error.clone();
    write_run_record(&artifacts.run_json, &record)?;

    let summary = run_summary(&record);
    if let Err(err) = write_summary(&artifacts.summary_json, &summary) {
        record_run_persistence_error(result, &artifacts.run_json, format!("write summary: {err}"))?;
        return Ok(());
    }

    if let Err(err) = append_run_index(
        &artifacts.runs_index_jsonl,
        &RunIndexEntry {
            run_id: record.run_id.clone(),
            created_at: record.created_at,
            record_path: artifacts.run_json.display().to_string(),
            summary_path: artifacts.summary_json.display().to_string(),
        },
        &artifacts.vibe_log,
    ) {
        record_late_persistence_error(result, format!("append runs_index.jsonl: {err}"))?;
    }

    Ok(())
}

pub fn persist_result_from_run(path: &Path, result_path: &Path) -> Result<(), String> {
    let record = read_run_record(path)?;
    let result = RunResult {
        run_id: Some(record.run_id),
        status: record.terminal_status.expect("terminal record status"),
        branch: record.branch,
        worktree: record.worktree,
        model: record.model,
        pre_run_commit: record.pre_run_commit,
        commit: record.commit,
        snapshot_commits: record.snapshot_commits,
        artifacts_dir: Some(record.artifacts_dir),
        events_log_path: Some(record.events_log_path),
        stderr_path: Some(record.stderr_path),
        run_path: Some(record.run_path),
        summary_path: Some(record.summary_path),
        changed_files: record.changed_files,
        persistence_error: record.persistence_error,
        error_message: record.error_message,
    };
    write_json_atomic(result_path, &result, "result")
}

pub fn latest_summary_for_key(repo_root: &Path, key: &str) -> Result<RunSummary, String> {
    let home = std::env::var("HOME").map_err(|_| "HOME not set".to_string())?;
    latest_summary_for_key_in(Path::new(&home), repo_root, key)
}

pub fn latest_summary_for_key_in(
    home: &Path,
    repo_root: &Path,
    key: &str,
) -> Result<RunSummary, String> {
    let slug = crate::worktree::slugify(key);
    let path = latest_run_json_for_key(home, repo_root, &slug)?;
    status_summary_from_path(&path)
}

pub fn latest_record_json_for_key(
    repo_root: &Path,
    key: &str,
) -> Result<serde_json::Value, String> {
    let home = std::env::var("HOME").map_err(|_| "HOME not set".to_string())?;
    latest_record_json_for_key_in(Path::new(&home), repo_root, key)
}

pub fn latest_record_json_for_key_in(
    home: &Path,
    repo_root: &Path,
    key: &str,
) -> Result<serde_json::Value, String> {
    let slug = crate::worktree::slugify(key);
    let path = latest_run_json_for_key(home, repo_root, &slug)?;
    record_json_from_path(&path)
}

fn latest_run_json_for_key(home: &Path, repo_root: &Path, slug: &str) -> Result<PathBuf, String> {
    let index = runs_index_path(home, repo_root, slug);
    let entries = read_runs_index(&index).map_err(|err| format!("read runs index: {err}"))?;
    let runs_dir = runs_root(home, repo_root, slug).join("runs");
    let canonical_runs_dir = runs_dir.canonicalize().ok();
    let mut candidates = Vec::new();
    let mut seen = HashSet::new();

    for entry in entries {
        if entry.record_path.is_empty() { continue; }
        let Some(path) = trusted_run_json_candidate(Path::new(&entry.record_path), canonical_runs_dir.as_deref()) else { continue; };
        if seen.insert(path.clone()) { candidates.push(path); }
    }
    if runs_dir.exists() {
        for entry in fs::read_dir(&runs_dir).map_err(|err| format!("read runs dir: {err}"))? {
            let entry = entry.map_err(|err| format!("read runs entry: {err}"))?;
            let run_dir = entry.path();
            if !run_dir.is_dir() { continue; }
            let Some(path) = trusted_run_json_candidate(&run_dir.join("run.json"), canonical_runs_dir.as_deref()) else { continue; };
            if seen.insert(path.clone()) { candidates.push(path); }
        }
    }
    candidates.into_iter()
        .filter_map(|path| {
            let record = record_json_from_path(&path).ok()?;
            let created_at = record.get("created_at")?.as_u64()?;
            Some((created_at, path))
        })
        .max_by(|(left_created_at, left_path), (right_created_at, right_path)| {
            left_created_at.cmp(right_created_at).then_with(|| left_path.cmp(right_path))
        })
        .map(|(_, path)| path)
        .ok_or_else(|| format!("no run.json artifacts found for key {slug}"))
}

fn trusted_run_json_candidate(path: &Path, canonical_runs_dir: Option<&Path>) -> Option<PathBuf> {
    if path.file_name()? != "run.json" || !path.exists() { return None; }
    let canonical_path = path.canonicalize().ok()?;
    let canonical_runs_dir = canonical_runs_dir?;
    canonical_path.starts_with(canonical_runs_dir).then_some(canonical_path)
}

#[cfg(test)]
mod tests {
    use super::{
        latest_record_json_for_key_in, latest_summary_for_key_in, persist_terminal_run,
        read_run_record, read_runs_index, record_late_persistence_error, ArtifactPaths, RunRecord,
    };
    use super::RunPhase;
    use crate::result::{RunResult, Status};
    use tempfile::tempdir;

    fn sample_result(artifacts_dir: &std::path::Path, summary_path: &std::path::Path) -> RunResult {
        RunResult {
            run_id: Some("run-id".to_string()),
            status: Status::Completed,
            branch: Some("vibe/pdev-099b".to_string()),
            worktree: Some("/tmp/worktree".to_string()),
            model: Some("gpt-5.4-mini".to_string()),
            pre_run_commit: Some("abc".to_string()),
            commit: Some("def".to_string()),
            snapshot_commits: vec!["snap".to_string()],
            artifacts_dir: Some(artifacts_dir.display().to_string()),
            events_log_path: Some(artifacts_dir.join("events.jsonl").display().to_string()),
            stderr_path: Some(artifacts_dir.join("agent.stderr.log").display().to_string()),
            run_path: Some(artifacts_dir.join("run.json").display().to_string()),
            summary_path: Some(summary_path.display().to_string()),
            changed_files: vec!["vibe/src/ledger.rs".to_string()],
            persistence_error: None,
            error_message: None,
        }
    }

    fn sample_record(summary_path: &std::path::Path, artifacts_dir: &std::path::Path) -> RunRecord {
        RunRecord {
            run_id: "run-id".to_string(),
            key: "pdev-099b".to_string(),
            slug: "pdev-099b".to_string(),
            created_at: 1778781727,
            branch: Some("vibe/pdev-099b".to_string()),
            worktree: Some("/tmp/worktree".to_string()),
            model: Some("gpt-5.4-mini".to_string()),
            phase: RunPhase::Finished,
            terminal_status: Some(Status::Completed),
            pre_run_commit: Some("abc".to_string()),
            commit: Some("def".to_string()),
            snapshot_commits: vec!["snap".to_string()],
            artifacts_dir: artifacts_dir.display().to_string(),
            run_path: artifacts_dir.join("run.json").display().to_string(),
            summary_path: summary_path.display().to_string(),
            result_path: artifacts_dir.join("result.json").display().to_string(),
            events_log_path: artifacts_dir.join("events.jsonl").display().to_string(),
            stderr_path: artifacts_dir.join("agent.stderr.log").display().to_string(),
            changed_files: vec!["vibe/src/ledger.rs".to_string()],
            error_message: None,
            persistence_error: None,
        }
    }

    fn sample_artifacts(dir: &std::path::Path) -> ArtifactPaths {
        ArtifactPaths {
            dir: dir.to_path_buf(),
            run_id: "run-id".to_string(),
            prompt_txt: dir.join("prompt.txt"),
            system_prompt_txt: dir.join("system-prompt.txt"),
            combined_prompt_txt: dir.join("combined-prompt.txt"),
            system_prompt_versions_txt: dir.join("system-prompt-versions.txt"),
            result_json: dir.join("result.json"),
            run_json: dir.join("run.json"),
            vibe_log: dir.join("vibe.log"),
            events_jsonl: dir.join("events.jsonl"),
            stderr_log: dir.join("agent.stderr.log"),
            extension_jsonl: dir.join("extension-events.jsonl"),
            snapshots_jsonl: dir.join("snapshots.jsonl"),
            summary_json: dir.join("summary.json"),
            runs_index_jsonl: dir.join("runs_index.jsonl"),
        }
    }

    #[test]
    fn late_persistence_error_updates_run_record() {
        let temp = tempdir().expect("tempdir");
        let artifacts_dir = temp.path().join("run");
        std::fs::create_dir_all(&artifacts_dir).expect("artifacts dir");
        let summary_path = artifacts_dir.join("summary.json");
        let record_path = artifacts_dir.join("run.json");
        let state = sample_record(&summary_path, &artifacts_dir);
        super::write_run_record(&record_path, &state).expect("write record");
        super::write_summary(&summary_path, &super::run_summary(&state)).expect("write summary");
        let mut result = sample_result(&artifacts_dir, &summary_path);

        record_late_persistence_error(&mut result, "append runs_index.jsonl: boom".to_string())
            .expect("repair run record");

        assert_eq!(
            result.persistence_error.as_deref(),
            Some("append runs_index.jsonl: boom")
        );

        let repaired = read_run_record(&record_path).expect("read repaired record");
        assert_eq!(
            repaired.persistence_error.as_deref(),
            Some("append runs_index.jsonl: boom")
        );

        let repaired_summary: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&summary_path).expect("read summary"))
                .expect("parse summary");
        assert_eq!(
            repaired_summary["persistence_error"],
            "append runs_index.jsonl: boom"
        );
        assert_eq!(
            repaired_summary["summary_path"],
            summary_path.display().to_string()
        );
    }

    #[test]
    fn late_persistence_error_rewrites_only_trusted_summary_path() {
        let temp = tempdir().expect("tempdir");
        let artifacts_dir = temp.path().join("run");
        std::fs::create_dir_all(&artifacts_dir).expect("artifacts dir");
        let summary_path = artifacts_dir.join("summary.json");
        let outside_summary_path = temp.path().join("outside-summary.json");
        let record_path = artifacts_dir.join("run.json");
        let mut state = sample_record(&summary_path, &artifacts_dir);
        state.summary_path = outside_summary_path.display().to_string();
        super::write_run_record(&record_path, &state).expect("write record");
        super::write_summary(&summary_path, &super::run_summary(&state)).expect("write summary");
        let mut result = sample_result(&artifacts_dir, &summary_path);

        record_late_persistence_error(&mut result, "append runs_index.jsonl: boom".to_string())
            .expect("repair run record");

        assert!(!outside_summary_path.exists());

        let repaired = read_run_record(&record_path).expect("read repaired record");
        assert_eq!(repaired.summary_path, summary_path.display().to_string());

        let repaired_summary: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&summary_path).expect("read summary"))
                .expect("parse summary");
        assert_eq!(
            repaired_summary["summary_path"],
            summary_path.display().to_string()
        );
    }

    #[test]
    fn summary_write_failure_keeps_terminal_state_durable() {
        let temp = tempdir().expect("tempdir");
        let artifacts_dir = temp.path().join("run");
        std::fs::create_dir_all(&artifacts_dir).expect("artifacts dir");
        std::fs::create_dir_all(artifacts_dir.join("summary.json")).expect("block summary file");
        let artifacts = sample_artifacts(&artifacts_dir);
        let summary_path = artifacts.summary_json.clone();
        let persisted = sample_record(&summary_path, &artifacts_dir);
        super::write_run_record(&artifacts.run_json, &persisted).expect("write record");
        let mut result = sample_result(&artifacts_dir, &summary_path);

        persist_terminal_run(&artifacts, &mut result).expect("persist terminal run");

        let repaired = read_run_record(&artifacts.run_json).expect("read repaired record");
        assert_eq!(repaired.phase, RunPhase::Finished);
        assert_eq!(repaired.terminal_status, Some(Status::Completed));
        assert_eq!(repaired.commit.as_deref(), Some("def"));
        assert_eq!(
            repaired.persistence_error.as_deref(),
            Some("write summary: rename summary temp: Is a directory (os error 21)")
        );
    }

    #[test]
    fn runs_index_reads_legacy_state_path() {
        let temp = tempdir().expect("tempdir");
        let path = temp.path().join("runs_index.jsonl");
        std::fs::write(&path, "{\"run_id\":\"legacy-run\",\"created_at\":1,\"state_path\":\"\",\"record_path\":\"/tmp/run.json\",\"summary_path\":\"/tmp/summary.json\"}\n").expect("write runs index");
        let entries = read_runs_index(&path).expect("read runs index");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].record_path, "/tmp/run.json");
    }

    fn write_run_json(path: &std::path::Path, run_id: &str, created_at: u64, phase: &str) {
        std::fs::create_dir_all(path.parent().expect("run dir")).expect("run dir");
        std::fs::write(path, serde_json::json!({
            "run_id": run_id, "key": "PDEV-055 demo/key", "slug": "pdev-055-demo-key",
            "created_at": created_at, "phase": phase, "terminal_status": if phase == "finished" { Some("completed") } else { None::<&str> },
            "branch": null, "worktree": null, "model": null, "pre_run_commit": null, "commit": null,
            "snapshot_commits": [], "changed_files": [], "artifacts_dir": "/tmp/run",
            "run_path": "/tmp/run/run.json", "summary_path": "/tmp/run/summary.json",
            "result_path": "/tmp/run/result.json", "events_log_path": "/tmp/run/events.jsonl",
            "stderr_path": "/tmp/run/agent.stderr.log", "error_message": null, "persistence_error": null
        }).to_string()).expect("write run json");
    }

    #[test]
    fn latest_summary_reads_run_json() {
        let temp = tempdir().expect("tempdir");
        let repo_root = temp.path().join("personal-dev-env");
        std::fs::create_dir_all(&repo_root).expect("repo dir");
        let run = temp.path().join(".local/state/vibe/personal-dev-env/pdev-055-demo-key/runs/a/run.json");
        write_run_json(&run, "run-id", 20, "finished");
        let summary = latest_summary_for_key_in(temp.path(), &repo_root, "PDEV-055 demo/key").expect("latest summary");
        assert_eq!(summary.run_id, "run-id");
        assert_eq!(summary.phase, RunPhase::Finished);
        assert_eq!(summary.status, Some(Status::Completed));
    }

    #[test]
    fn latest_record_orders_by_created_at() {
        let temp = tempdir().expect("tempdir");
        let repo_root = temp.path().join("personal-dev-env");
        std::fs::create_dir_all(&repo_root).expect("repo dir");
        let runs = temp.path().join(".local/state/vibe/personal-dev-env/pdev-055-demo-key/runs");
        write_run_json(&runs.join("z/run.json"), "old", 10, "finished");
        write_run_json(&runs.join("a/run.json"), "new", 20, "running_agent");
        let latest = latest_record_json_for_key_in(temp.path(), &repo_root, "PDEV-055 demo/key").expect("latest record");
        assert_eq!(latest["run_id"], "new");
    }

    #[test]
    fn latest_summary_errors_without_run() {
        let temp = tempdir().expect("tempdir");
        let repo_root = temp.path().join("personal-dev-env");
        std::fs::create_dir_all(&repo_root).expect("repo dir");
        let err = latest_summary_for_key_in(temp.path(), &repo_root, "PDEV-055 demo/key").expect_err("missing run");
        assert!(err.contains("no run.json artifacts found"));
    }

}
