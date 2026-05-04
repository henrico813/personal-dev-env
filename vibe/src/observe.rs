use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Artifact paths for one Vibe run.
pub struct ArtifactPaths {
    pub dir: PathBuf,
    pub prompt_txt: PathBuf,
    pub events_jsonl: PathBuf,
    pub stderr_log: PathBuf,
    pub extension_jsonl: PathBuf,
    pub snapshots_jsonl: PathBuf,
}

fn create_artifacts_in(
    home: &Path,
    repo_root: &Path,
    key: &str,
    run_id: &str,
) -> Result<ArtifactPaths, String> {
    let repo_id = repo_root
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("repo");
    let dir = home
        .join(".local/state/vibe")
        .join(repo_id)
        .join(key)
        .join("runs")
        .join(run_id);
    fs::create_dir_all(&dir).map_err(|e| format!("create run dir: {e}"))?;
    Ok(ArtifactPaths {
        dir: dir.clone(),
        prompt_txt: dir.join("prompt.txt"),
        events_jsonl: dir.join("events.jsonl"),
        stderr_log: dir.join("agent.stderr.log"),
        extension_jsonl: dir.join("extension-events.jsonl"),
        snapshots_jsonl: dir.join("snapshots.jsonl"),
    })
}

pub fn create_artifacts(repo_root: &Path, key: &str) -> Result<ArtifactPaths, String> {
    let home = std::env::var("HOME").map_err(|_| "HOME not set".to_string())?;
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_secs();
    let run_id = format!("{}-{}", ts, std::process::id());
    create_artifacts_in(Path::new(&home), repo_root, key, &run_id)
}

pub fn copy_prompt(src: &Path, dst: &Path) -> Result<(), String> {
    fs::copy(src, dst).map_err(|e| format!("copy prompt file: {e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{copy_prompt, create_artifacts_in};
    use std::path::Path;
    use tempfile::tempdir;

    #[test]
    fn artifacts_use_expected_layout() {
        let temp = tempdir().expect("tempdir");
        let repo_root = temp.path().join("personal-dev-env");
        std::fs::create_dir_all(&repo_root).expect("repo dir");

        let paths =
            create_artifacts_in(temp.path(), &repo_root, "pdev-049-demo", "1700000000-4242")
                .expect("artifact paths");

        let dir = temp
            .path()
            .join(".local/state/vibe/personal-dev-env/pdev-049-demo/runs/1700000000-4242");
        assert_eq!(paths.dir, dir);
        assert_eq!(paths.prompt_txt, dir.join("prompt.txt"));
        assert_eq!(paths.events_jsonl, dir.join("events.jsonl"));
        assert_eq!(paths.stderr_log, dir.join("agent.stderr.log"));
        assert_eq!(paths.extension_jsonl, dir.join("extension-events.jsonl"));
        assert_eq!(paths.snapshots_jsonl, dir.join("snapshots.jsonl"));
    }

    #[test]
    fn prompt_copy_preserves_input() {
        let temp = tempdir().expect("tempdir");
        let src = temp.path().join("prompt.txt");
        let dst = temp.path().join("copied.txt");
        std::fs::write(&src, "hello").expect("write src");

        copy_prompt(&src, &dst).expect("copy prompt");

        assert_eq!(std::fs::read_to_string(dst).expect("read dst"), "hello");
        assert!(Path::new(&src).exists());
    }
}
