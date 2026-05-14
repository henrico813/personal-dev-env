use crate::{
    ledger, observe,
    result::{RunResult, Status},
    worktree,
};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

#[cfg(test)]
use std::sync::{Mutex, OnceLock};

#[cfg(test)]
pub(crate) fn home_env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

// Phase 1 defines the persisted-state contract ahead of the runtime and
// recovery wiring that will consume it in later phases.
#[cfg_attr(not(test), allow(dead_code))]
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

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PersistedRunState {
    #[serde(default)]
    pub run_id: String,
    pub key: String,
    pub slug: String,
    #[serde(default)]
    pub created_at: u64,
    pub branch: Option<String>,
    pub worktree: Option<String>,
    pub model: Option<String>,
    pub phase: RunPhase,
    pub terminal_status: Option<Status>,
    pub pre_run_commit: Option<String>,
    pub commit: Option<String>,
    pub snapshot_commits: Vec<String>,
    pub artifacts_dir: Option<String>,
    pub events_log_path: Option<String>,
    pub stderr_path: Option<String>,
    pub result_path: Option<String>,
    pub wrapper_log_path: Option<String>,
    #[serde(default)]
    pub summary_path: Option<String>,
    #[serde(default)]
    pub changed_files: Vec<String>,
    pub error_message: Option<String>,
    #[serde(default)]
    pub persistence_error: Option<String>,
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn write(path: &Path, state: &PersistedRunState) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("state path has no parent: {}", path.display()))?;
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let tmp = parent.join(format!(".run-state.{ts}.tmp"));
    let json = serde_json::to_string_pretty(state).map_err(|e| format!("serialize state: {e}"))?;
    fs::write(&tmp, json).map_err(|e| format!("write state temp: {e}"))?;
    fs::rename(&tmp, path).map_err(|e| format!("rename state temp: {e}"))?;
    Ok(())
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn read(path: &Path) -> Result<PersistedRunState, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("read state: {e}"))?;
    serde_json::from_str(&text).map_err(|e| format!("parse state: {e}"))
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn latest_for_key(repo_root: &Path, key: &str) -> Result<PersistedRunState, String> {
    let slug = worktree::slugify(key);
    let home = std::env::var("HOME").map_err(|_| "HOME not set".to_string())?;
    let mut last_error = None;

    if let Ok(Some(state)) = latest_for_key_from_index(repo_root, &home, &slug) {
        return Ok(state);
    }

    for run_dir in observe::run_dirs_newest_to_oldest_in(Path::new(&home), repo_root, &slug)? {
        match read(&run_dir.join("run-state.json")) {
            Ok(state) => return Ok(state),
            Err(err) => last_error = Some(err),
        }
    }

    Err(last_error.unwrap_or_else(|| format!("no persisted runs found for key {key}")))
}

fn latest_for_key_from_index(
    repo_root: &Path,
    home: &str,
    slug: &str,
) -> Result<Option<PersistedRunState>, String> {
    let home = Path::new(home);
    let index = ledger::runs_index_path(home, repo_root, slug);
    let entries = match ledger::read_runs_index(&index) {
        Ok(entries) => entries,
        Err(_) => return read_fallback(home, repo_root, slug),
    };

    if entries.is_empty() {
        return Ok(None);
    }

    for entry in entries.into_iter().rev() {
        match read(Path::new(&entry.state_path)) {
            Ok(state) => return Ok(Some(state)),
            Err(_) => continue,
        }
    }
    Ok(None)
}

fn read_fallback(
    home: &Path,
    repo_root: &Path,
    slug: &str,
) -> Result<Option<PersistedRunState>, String> {
    for run_dir in observe::run_dirs_newest_to_oldest_in(home, repo_root, slug)? {
        match read(&run_dir.join("run-state.json")) {
            Ok(state) => return Ok(Some(state)),
            Err(_) => continue,
        }
    }
    Ok(None)
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn write_terminal_from_result(result: &RunResult) -> Result<(), String> {
    let Some(dir) = result.artifacts_dir.as_deref() else {
        return Ok(());
    };
    let path = std::path::Path::new(dir).join("run-state.json");
    if !path.exists() {
        return Ok(());
    }

    let mut persisted = read(&path)?;
    persisted.phase = RunPhase::Finished;
    persisted.terminal_status = Some(result.status.clone());
    persisted.pre_run_commit = result.pre_run_commit.clone();
    persisted.commit = result.commit.clone();
    persisted.snapshot_commits = result.snapshot_commits.clone();
    persisted.changed_files = result.changed_files.clone();
    persisted.error_message = result.error_message.clone();
    persisted.persistence_error = result.persistence_error.clone();
    write(&path, &persisted)
}

#[cfg(test)]
mod tests {
    use super::{latest_for_key, read, write, PersistedRunState, RunPhase};
    use crate::result::Status;
    use tempfile::tempdir;

    fn sample_state() -> PersistedRunState {
        PersistedRunState {
            run_id: "run-id".to_string(),
            key: "PDEV-055 demo/key".to_string(),
            slug: "pdev-055-demo-key".to_string(),
            created_at: 1778000000,
            branch: Some("vibe/pdev-055-demo-key".to_string()),
            worktree: Some("/tmp/worktree".to_string()),
            model: Some("openai-codex/gpt-5.4".to_string()),
            phase: RunPhase::RunningAgent,
            terminal_status: None,
            pre_run_commit: Some("abc".to_string()),
            commit: None,
            snapshot_commits: vec!["snap".to_string()],
            artifacts_dir: Some("/tmp/run".to_string()),
            events_log_path: Some("/tmp/run/events.jsonl".to_string()),
            stderr_path: Some("/tmp/run/agent.stderr.log".to_string()),
            result_path: Some("/tmp/run/result.json".to_string()),
            wrapper_log_path: Some("/tmp/run/vibe.log".to_string()),
            summary_path: Some("/tmp/run/summary.json".to_string()),
            changed_files: Vec::new(),
            error_message: None,
            persistence_error: None,
        }
    }

    #[test]
    fn state_round_trips_through_atomic_write() {
        let temp = tempdir().expect("tempdir");
        let path = temp.path().join("run-state.json");
        let state = sample_state();

        write(&path, &state).expect("write state");
        let read_back = read(&path).expect("read state");

        assert_eq!(read_back.slug, state.slug);
        assert_eq!(read_back.phase, state.phase);
        assert_eq!(read_back.snapshot_commits, state.snapshot_commits);
    }

    #[test]
    fn latest_for_key_normalizes_raw_key() {
        let _guard = super::home_env_lock().lock().expect("lock HOME env");
        let temp = tempdir().expect("tempdir");
        let saved_home = std::env::var_os("HOME");
        std::env::set_var("HOME", temp.path());
        let repo_root = temp.path().join("personal-dev-env");
        std::fs::create_dir_all(&repo_root).expect("repo dir");

        let run_dir = temp
            .path()
            .join(".local/state/vibe/personal-dev-env/pdev-055-demo-key/runs/1778000000-42");
        std::fs::create_dir_all(&run_dir).expect("run dir");

        let state = sample_state();
        write(&run_dir.join("run-state.json"), &state).expect("write state");

        let latest = latest_for_key(&repo_root, "PDEV-055 demo/key").expect("latest state");

        assert_eq!(latest.slug, "pdev-055-demo-key");

        if let Some(saved_home) = saved_home {
            std::env::set_var("HOME", saved_home);
        } else {
            std::env::remove_var("HOME");
        }
    }

    #[test]
    fn latest_for_key_skips_broken_newest_run() {
        let _guard = super::home_env_lock().lock().expect("lock HOME env");
        let temp = tempdir().expect("tempdir");
        let saved_home = std::env::var_os("HOME");
        std::env::set_var("HOME", temp.path());
        let repo_root = temp.path().join("personal-dev-env");
        std::fs::create_dir_all(&repo_root).expect("repo dir");

        let runs = temp
            .path()
            .join(".local/state/vibe/personal-dev-env/pdev-055-demo-key/runs");
        std::fs::create_dir_all(runs.join("1778000001-43")).expect("older run");
        std::fs::create_dir_all(runs.join("1778000003-45")).expect("incomplete run");
        std::fs::create_dir_all(runs.join("1778000004-46")).expect("broken run");

        let mut state = sample_state();
        state.terminal_status = Some(Status::Completed);
        write(&runs.join("1778000001-43/run-state.json"), &state).expect("write older state");
        std::fs::write(runs.join("1778000004-46/run-state.json"), "{").expect("write broken state");

        let latest = latest_for_key(&repo_root, "PDEV-055 demo/key").expect("latest state");

        assert_eq!(latest.terminal_status, Some(Status::Completed));

        if let Some(saved_home) = saved_home {
            std::env::set_var("HOME", saved_home);
        } else {
            std::env::remove_var("HOME");
        }
    }
}
