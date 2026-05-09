use crate::observe;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RunPhase {
    Created,
    Running,
    Finalizing,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PersistedRunState {
    pub key: String,
    pub run_id: String,
    pub phase: RunPhase,
    pub artifacts_dir: String,
}

pub fn atomic_write<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("write {}: missing parent", path.display()))?;
    fs::create_dir_all(parent).map_err(|e| format!("create {}: {e}", parent.display()))?;
    let tmp = parent.join(format!(
        ".{}.{}.tmp",
        path.file_name().and_then(|s| s.to_str()).unwrap_or("state"),
        std::process::id()
    ));
    let bytes = serde_json::to_vec_pretty(value).map_err(|e| format!("serialize state: {e}"))?;
    let mut file = fs::File::create(&tmp).map_err(|e| format!("create {}: {e}", tmp.display()))?;
    file.write_all(&bytes)
        .map_err(|e| format!("write {}: {e}", tmp.display()))?;
    file.sync_all()
        .map_err(|e| format!("sync {}: {e}", tmp.display()))?;
    drop(file);
    fs::rename(&tmp, path).map_err(|e| format!("replace {}: {e}", path.display()))
}

pub fn atomic_read<T: DeserializeOwned>(path: &Path) -> Result<Option<T>, String> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(format!("read {}: {err}", path.display())),
    };
    serde_json::from_str(&text)
        .map(Some)
        .map_err(|e| format!("parse {}: {e}", path.display()))
}

pub fn latest_for_key(repo_root: &Path, key: &str) -> Result<Option<PersistedRunState>, String> {
    let Some(run_dir) = observe::latest_run_dir(repo_root, key)? else {
        return Ok(None);
    };
    let state_path = run_dir.join("state.json");
    atomic_read(&state_path)
}

#[cfg(test)]
mod tests {
    use super::{atomic_read, atomic_write, latest_for_key, PersistedRunState, RunPhase};
    use std::path::{Path, PathBuf};
    use tempfile::tempdir;

    fn run_dir(home: &Path, repo_root: &Path, key: &str, run_id: &str) -> PathBuf {
        home.join(".local/state/vibe")
            .join(repo_root.file_name().and_then(|s| s.to_str()).unwrap_or("repo"))
            .join(key)
            .join("runs")
            .join(run_id)
    }

    #[test]
    fn atomic_helpers_round_trip_state() {
        let temp = tempdir().expect("tempdir");
        let path = temp.path().join("state.json");
        let state = PersistedRunState {
            key: "demo".to_string(),
            run_id: "1700000000-1".to_string(),
            phase: RunPhase::Running,
            artifacts_dir: "/tmp/run".to_string(),
        };

        atomic_write(&path, &state).expect("write state");
        let read_back: PersistedRunState = atomic_read(&path)
            .expect("read state")
            .expect("state present");

        assert_eq!(read_back, state);
    }

    #[test]
    fn latest_for_key_returns_latest_state_file() {
        let temp = tempdir().expect("tempdir");
        let saved_home = std::env::var_os("HOME");
        std::env::set_var("HOME", temp.path());
        let repo_root = temp.path().join("repo");
        std::fs::create_dir_all(&repo_root).expect("repo dir");

        let older_dir = run_dir(temp.path(), &repo_root, "demo", "1700000000-1");
        let newer_dir = run_dir(temp.path(), &repo_root, "demo", "1700000001-1");
        std::fs::create_dir_all(&older_dir).expect("older dir");
        std::fs::create_dir_all(&newer_dir).expect("newer dir");

        let older = PersistedRunState {
            key: "demo".to_string(),
            run_id: "1700000000-1".to_string(),
            phase: RunPhase::Running,
            artifacts_dir: older_dir.display().to_string(),
        };
        let newer = PersistedRunState {
            key: "demo".to_string(),
            run_id: "1700000001-1".to_string(),
            phase: RunPhase::Finalizing,
            artifacts_dir: newer_dir.display().to_string(),
        };
        atomic_write(&older_dir.join("state.json"), &older).expect("write older");
        atomic_write(&newer_dir.join("state.json"), &newer).expect("write newer");

        let latest = latest_for_key(&repo_root, "demo")
            .expect("latest state")
            .expect("state present");

        assert_eq!(latest, newer);

        if let Some(saved_home) = saved_home {
            std::env::set_var("HOME", saved_home);
        } else {
            std::env::remove_var("HOME");
        }
    }
}
