use crate::{
    ledger::{self, RunSummary},
    result::{RunResult, Status},
    worktree,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

#[cfg(test)]
use std::sync::{Mutex, OnceLock};

#[cfg(test)]
pub(crate) fn home_env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

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
    #[serde(default)]
    pub run_path: Option<String>,
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
pub fn latest_summary_for_key(repo_root: &Path, key: &str) -> Result<RunSummary, String> {
    let slug = worktree::slugify(key);
    let path = latest_run_json_for_key(repo_root, &slug)?;
    ledger::status_summary_from_path(&path)
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn latest_record_json_for_key(
    repo_root: &Path,
    key: &str,
) -> Result<serde_json::Value, String> {
    let slug = worktree::slugify(key);
    let path = latest_run_json_for_key(repo_root, &slug)?;
    ledger::record_json_from_path(&path)
}

fn latest_run_json_for_key(repo_root: &Path, slug: &str) -> Result<PathBuf, String> {
    let home = std::env::var("HOME").map_err(|_| "HOME not set".to_string())?;
    let home = Path::new(&home);
    let index = ledger::runs_index_path(home, repo_root, slug);
    let entries =
        ledger::read_runs_index(&index).map_err(|err| format!("read runs index: {err}"))?;
    let runs_dir = ledger::runs_root(home, repo_root, slug).join("runs");
    let canonical_runs_dir = runs_dir.canonicalize().ok();

    let mut candidates = Vec::new();
    let mut seen = HashSet::new();

    for entry in entries {
        if entry.record_path.is_empty() {
            continue;
        }
        let Some(path) = trusted_run_json_candidate(
            Path::new(&entry.record_path),
            canonical_runs_dir.as_deref(),
        ) else {
            continue;
        };
        if seen.insert(path.clone()) {
            candidates.push(path);
        }
    }

    if runs_dir.exists() {
        for entry in fs::read_dir(&runs_dir).map_err(|err| format!("read runs dir: {err}"))? {
            let entry = entry.map_err(|err| format!("read runs entry: {err}"))?;
            let run_dir = entry.path();
            if !run_dir.is_dir() {
                continue;
            }
            let Some(path) = trusted_run_json_candidate(
                &run_dir.join("run.json"),
                canonical_runs_dir.as_deref(),
            ) else {
                continue;
            };
            if seen.insert(path.clone()) {
                candidates.push(path);
            }
        }
    }

    candidates
        .into_iter()
        .filter_map(|path| {
            let record = ledger::record_json_from_path(&path).ok()?;
            let created_at = record.get("created_at")?.as_u64()?;
            Some((created_at, path))
        })
        .max_by(
            |(left_created_at, left_path), (right_created_at, right_path)| {
                left_created_at
                    .cmp(right_created_at)
                    .then_with(|| left_path.cmp(right_path))
            },
        )
        .map(|(_, path)| path)
        .ok_or_else(|| format!("no run.json artifacts found for key {slug}"))
}

fn trusted_run_json_candidate(path: &Path, canonical_runs_dir: Option<&Path>) -> Option<PathBuf> {
    if path.file_name()? != "run.json" || !path.exists() {
        return None;
    }

    let canonical_path = path.canonicalize().ok()?;
    let canonical_runs_dir = canonical_runs_dir?;
    canonical_path
        .starts_with(canonical_runs_dir)
        .then_some(canonical_path)
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn write_terminal_from_result(_result: &RunResult) -> Result<(), String> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        latest_record_json_for_key, latest_summary_for_key, read, write, PersistedRunState,
        RunPhase,
    };
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
            run_path: Some("/tmp/run/run.json".to_string()),
            result_path: Some("/tmp/run/result.json".to_string()),
            wrapper_log_path: Some("/tmp/run/vibe.log".to_string()),
            summary_path: Some("/tmp/run/summary.json".to_string()),
            changed_files: Vec::new(),
            error_message: None,
            persistence_error: None,
        }
    }

    fn write_run_json(path: &std::path::Path, run_id: &str, created_at: u64) {
        let parent = path.parent().expect("run dir");
        std::fs::create_dir_all(parent).expect("run dir");
        std::fs::write(
            path,
            serde_json::json!({
                "run_id": run_id,
                "key": "PDEV-055 demo/key",
                "slug": "pdev-055-demo-key",
                "created_at": created_at,
                "phase": "finished",
                "terminal_status": "completed",
                "branch": "vibe/pdev-055-demo-key",
                "worktree": "/tmp/worktree",
                "model": "openai-codex/gpt-5.4",
                "pre_run_commit": null,
                "commit": null,
                "snapshot_commits": [],
                "changed_files": [],
                "artifacts_dir": "/tmp/run",
                "run_path": "/tmp/run/run.json",
                "summary_path": "/tmp/run/summary.json",
                "result_path": "/tmp/run/result.json",
                "events_log_path": "/tmp/run/events.jsonl",
                "stderr_path": "/tmp/run/agent.stderr.log",
                "error_message": null,
                "persistence_error": null
            })
            .to_string(),
        )
        .expect("write run json");
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
    fn latest_summary_for_key_reads_run_json_from_index() {
        let _guard = super::home_env_lock().lock().expect("lock HOME env");
        let temp = tempdir().expect("tempdir");
        let saved_home = std::env::var_os("HOME");
        std::env::set_var("HOME", temp.path());
        let repo_root = temp.path().join("personal-dev-env");
        std::fs::create_dir_all(&repo_root).expect("repo dir");

        let runs = temp
            .path()
            .join(".local/state/vibe/personal-dev-env/pdev-055-demo-key/runs");
        let run_path = runs.join("1778000000-42/run.json");
        write_run_json(&run_path, "run-id", 1778000000);

        let summary =
            latest_summary_for_key(&repo_root, "PDEV-055 demo/key").expect("latest summary");
        assert_eq!(summary.run_id, "run-id");
        assert_eq!(summary.phase, RunPhase::Finished);
        assert_eq!(summary.status, Some(Status::Completed));

        if let Some(saved_home) = saved_home {
            std::env::set_var("HOME", saved_home);
        } else {
            std::env::remove_var("HOME");
        }
    }

    #[test]
    fn latest_summary_errors_when_run_json_missing() {
        let _guard = super::home_env_lock().lock().expect("lock HOME env");
        let temp = tempdir().expect("tempdir");
        let saved_home = std::env::var_os("HOME");
        std::env::set_var("HOME", temp.path());
        let repo_root = temp.path().join("personal-dev-env");
        std::fs::create_dir_all(&repo_root).expect("repo dir");

        let runs = temp
            .path()
            .join(".local/state/vibe/personal-dev-env/pdev-055-demo-key/runs");
        std::fs::create_dir_all(runs.join("1778000004-46")).expect("run dir");

        let err = latest_summary_for_key(&repo_root, "PDEV-055 demo/key")
            .expect_err("missing run json should fail");

        assert!(err.contains("no run.json artifacts found"));

        if let Some(saved_home) = saved_home {
            std::env::set_var("HOME", saved_home);
        } else {
            std::env::remove_var("HOME");
        }
    }

    #[test]
    fn latest_record_json_reads_authoritative_run_file() {
        let _guard = super::home_env_lock().lock().expect("lock HOME env");
        let temp = tempdir().expect("tempdir");
        let saved_home = std::env::var_os("HOME");
        std::env::set_var("HOME", temp.path());
        let repo_root = temp.path().join("personal-dev-env");
        std::fs::create_dir_all(&repo_root).expect("repo dir");

        let runs = temp
            .path()
            .join(".local/state/vibe/personal-dev-env/pdev-055-demo-key/runs");
        write_run_json(&runs.join("a-uuid/run.json"), "newer-run", 20);

        let latest = latest_record_json_for_key(&repo_root, "PDEV-055 demo/key")
            .expect("latest record json");
        assert_eq!(latest["run_id"], "newer-run");

        if let Some(saved_home) = saved_home {
            std::env::set_var("HOME", saved_home);
        } else {
            std::env::remove_var("HOME");
        }
    }

    #[test]
    fn latest_summary_reports_in_progress_run_without_panicking() {
        let _guard = super::home_env_lock().lock().expect("lock HOME env");
        let temp = tempdir().expect("tempdir");
        let saved_home = std::env::var_os("HOME");
        std::env::set_var("HOME", temp.path());
        let repo_root = temp.path().join("personal-dev-env");
        std::fs::create_dir_all(&repo_root).expect("repo dir");

        let runs = temp
            .path()
            .join(".local/state/vibe/personal-dev-env/pdev-055-demo-key/runs");
        let parent = runs.join("a-uuid");
        std::fs::create_dir_all(&parent).expect("run dir");
        std::fs::write(
            parent.join("run.json"),
            serde_json::json!({
                "run_id": "in-progress-run",
                "key": "PDEV-055 demo/key",
                "slug": "pdev-055-demo-key",
                "created_at": 1778000002_u64,
                "phase": "running_agent",
                "terminal_status": null,
                "branch": "vibe/pdev-055-demo-key",
                "worktree": "/tmp/worktree",
                "model": "openai-codex/gpt-5.4",
                "pre_run_commit": null,
                "commit": null,
                "snapshot_commits": [],
                "changed_files": [],
                "artifacts_dir": "/tmp/run",
                "run_path": "/tmp/run/run.json",
                "summary_path": "/tmp/run/summary.json",
                "result_path": "/tmp/run/result.json",
                "events_log_path": "/tmp/run/events.jsonl",
                "stderr_path": "/tmp/run/agent.stderr.log",
                "error_message": null,
                "persistence_error": null
            })
            .to_string(),
        )
        .expect("write run json");

        let summary =
            latest_summary_for_key(&repo_root, "PDEV-055 demo/key").expect("latest summary");
        assert_eq!(summary.run_id, "in-progress-run");
        assert_eq!(summary.phase, RunPhase::RunningAgent);
        assert_eq!(summary.status, None);

        if let Some(saved_home) = saved_home {
            std::env::set_var("HOME", saved_home);
        } else {
            std::env::remove_var("HOME");
        }
    }

    #[test]
    fn latest_summary_skips_broken_newest_index_entry() {
        let _guard = super::home_env_lock().lock().expect("lock HOME env");
        let temp = tempdir().expect("tempdir");
        let saved_home = std::env::var_os("HOME");
        std::env::set_var("HOME", temp.path());
        let repo_root = temp.path().join("personal-dev-env");
        std::fs::create_dir_all(&repo_root).expect("repo dir");

        let slug_root = temp
            .path()
            .join(".local/state/vibe/personal-dev-env/pdev-055-demo-key");
        let runs = slug_root.join("runs");
        let older_run = runs.join("1778000000-42/run.json");
        write_run_json(&older_run, "older-run", 1778000000);
        std::fs::write(
            slug_root.join("runs_index.jsonl"),
            format!(
                "{{\"run_id\":\"older-run\",\"created_at\":1778000000,\"state_path\":\"\",\"record_path\":\"{}\",\"summary_path\":\"{}\"}}\n{{\"run_id\":\"broken-run\",\"created_at\":1778000001,\"state_path\":\"\",\"record_path\":\"{}\",\"summary_path\":\"{}\"}}\n",
                older_run.display(),
                runs.join("1778000000-42/summary.json").display(),
                runs.join("1778000001-43/run.json").display(),
                runs.join("1778000001-43/summary.json").display(),
            ),
        )
        .expect("write runs index");

        let summary =
            latest_summary_for_key(&repo_root, "PDEV-055 demo/key").expect("latest summary");
        assert_eq!(summary.run_id, "older-run");

        if let Some(saved_home) = saved_home {
            std::env::set_var("HOME", saved_home);
        } else {
            std::env::remove_var("HOME");
        }
    }

    #[test]
    fn latest_record_json_uses_created_at_over_uuid_path_order() {
        let _guard = super::home_env_lock().lock().expect("lock HOME env");
        let temp = tempdir().expect("tempdir");
        let saved_home = std::env::var_os("HOME");
        std::env::set_var("HOME", temp.path());
        let repo_root = temp.path().join("personal-dev-env");
        std::fs::create_dir_all(&repo_root).expect("repo dir");

        let runs = temp
            .path()
            .join(".local/state/vibe/personal-dev-env/pdev-055-demo-key/runs");
        write_run_json(&runs.join("z-uuid/run.json"), "lexically-later", 10);
        write_run_json(&runs.join("a-uuid/run.json"), "created-later", 20);

        let latest = latest_record_json_for_key(&repo_root, "PDEV-055 demo/key")
            .expect("latest record json");
        assert_eq!(latest["run_id"], "created-later");

        if let Some(saved_home) = saved_home {
            std::env::set_var("HOME", saved_home);
        } else {
            std::env::remove_var("HOME");
        }
    }

    #[test]
    fn latest_record_json_ignores_index_path_outside_runs_root() {
        let _guard = super::home_env_lock().lock().expect("lock HOME env");
        let temp = tempdir().expect("tempdir");
        let saved_home = std::env::var_os("HOME");
        std::env::set_var("HOME", temp.path());
        let repo_root = temp.path().join("personal-dev-env");
        std::fs::create_dir_all(&repo_root).expect("repo dir");

        let slug_root = temp
            .path()
            .join(".local/state/vibe/personal-dev-env/pdev-055-demo-key");
        let runs = slug_root.join("runs");
        let trusted_run = runs.join("1778000000-42/run.json");
        let untrusted_run = temp.path().join("escaped/run.json");
        write_run_json(&trusted_run, "trusted-run", 1778000000);
        write_run_json(&untrusted_run, "untrusted-run", 1778000001);
        std::fs::write(
            slug_root.join("runs_index.jsonl"),
            format!(
                "{{\"run_id\":\"trusted-run\",\"created_at\":1778000000,\"state_path\":\"\",\"record_path\":\"{}\",\"summary_path\":\"{}\"}}\n{{\"run_id\":\"untrusted-run\",\"created_at\":1778000001,\"state_path\":\"\",\"record_path\":\"{}\",\"summary_path\":\"{}\"}}\n",
                trusted_run.display(),
                runs.join("1778000000-42/summary.json").display(),
                untrusted_run.display(),
                temp.path().join("escaped/summary.json").display(),
            ),
        )
        .expect("write runs index");

        let latest = latest_record_json_for_key(&repo_root, "PDEV-055 demo/key")
            .expect("latest record json");
        assert_eq!(latest["run_id"], "trusted-run");

        if let Some(saved_home) = saved_home {
            std::env::set_var("HOME", saved_home);
        } else {
            std::env::remove_var("HOME");
        }
    }
}
