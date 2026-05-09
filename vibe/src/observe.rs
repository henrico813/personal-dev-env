use crate::prompts::RenderedPrompt;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Artifact paths for one Vibe run.
pub struct ArtifactPaths {
    pub dir: PathBuf,
    pub prompt_txt: PathBuf,
    pub system_prompt_txt: PathBuf,
    pub combined_prompt_txt: PathBuf,
    pub system_prompt_versions_txt: PathBuf,
    pub state_json: PathBuf,
    pub result_json: PathBuf,
    pub log_txt: PathBuf,
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
    let snapshots_jsonl = dir.join("snapshots.jsonl");
    fs::write(&snapshots_jsonl, "").map_err(|e| format!("seed snapshots artifact: {e}"))?;
    Ok(ArtifactPaths {
        dir: dir.clone(),
        prompt_txt: dir.join("prompt.txt"),
        system_prompt_txt: dir.join("system-prompt.txt"),
        combined_prompt_txt: dir.join("combined-prompt.txt"),
        system_prompt_versions_txt: dir.join("system-prompt-versions.txt"),
        state_json: dir.join("state.json"),
        result_json: dir.join("result.json"),
        log_txt: dir.join("log.txt"),
        events_jsonl: dir.join("events.jsonl"),
        stderr_log: dir.join("agent.stderr.log"),
        extension_jsonl: dir.join("extension-events.jsonl"),
        snapshots_jsonl,
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

fn parse_run_dir_name(name: &str) -> Option<(u64, u32)> {
    let (ts, pid) = name.rsplit_once('-')?;
    Some((ts.parse().ok()?, pid.parse().ok()?))
}

fn latest_run_dir_in(home: &Path, repo_root: &Path, key: &str) -> Result<Option<PathBuf>, String> {
    let repo_id = repo_root
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("repo");
    let runs_dir = home
        .join(".local/state/vibe")
        .join(repo_id)
        .join(key)
        .join("runs");
    let entries = match fs::read_dir(&runs_dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(format!("read runs dir: {err}")),
    };

    let mut latest: Option<(u64, u32, PathBuf)> = None;
    for entry in entries {
        let entry = entry.map_err(|e| format!("read runs entry: {e}"))?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        let Some((ts, pid)) = parse_run_dir_name(name) else {
            continue;
        };
        let replace = latest
            .as_ref()
            .map(|(latest_ts, latest_pid, _)| ts > *latest_ts || (ts == *latest_ts && pid > *latest_pid))
            .unwrap_or(true);
        if replace {
            latest = Some((ts, pid, path));
        }
    }

    Ok(latest.map(|(_, _, path)| path))
}

pub fn latest_run_dir(repo_root: &Path, key: &str) -> Result<Option<PathBuf>, String> {
    let home = std::env::var("HOME").map_err(|_| "HOME not set".to_string())?;
    latest_run_dir_in(Path::new(&home), repo_root, key)
}

fn write_text(dst: &Path, text: &str, label: &str) -> Result<(), String> {
    fs::write(dst, text).map_err(|e| format!("write {label}: {e}"))
}

pub(crate) fn write_prompt_artifact(dst: &Path, prompt: &str) -> Result<(), String> {
    write_text(dst, prompt, "prompt artifact")
}

pub(crate) fn write_rendered_prompt(
    paths: &ArtifactPaths,
    rendered: &RenderedPrompt,
) -> Result<(), String> {
    write_text(
        &paths.system_prompt_txt,
        &rendered.system_prompt,
        "system prompt artifact",
    )?;
    write_text(
        &paths.combined_prompt_txt,
        &rendered.combined_prompt,
        "combined prompt artifact",
    )?;
    write_text(
        &paths.system_prompt_versions_txt,
        &rendered.version_manifest,
        "system prompt versions artifact",
    )
}

#[cfg(test)]
mod tests {
    use super::{create_artifacts_in, write_prompt_artifact, write_rendered_prompt};
    use crate::prompts::RenderedPrompt;
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
        assert_eq!(paths.system_prompt_txt, dir.join("system-prompt.txt"));
        assert_eq!(paths.combined_prompt_txt, dir.join("combined-prompt.txt"));
        assert_eq!(
            paths.system_prompt_versions_txt,
            dir.join("system-prompt-versions.txt")
        );
        assert_eq!(paths.state_json, dir.join("state.json"));
        assert_eq!(paths.result_json, dir.join("result.json"));
        assert_eq!(paths.log_txt, dir.join("log.txt"));
        assert_eq!(paths.events_jsonl, dir.join("events.jsonl"));
        assert_eq!(paths.stderr_log, dir.join("agent.stderr.log"));
        assert_eq!(paths.extension_jsonl, dir.join("extension-events.jsonl"));
        assert_eq!(paths.snapshots_jsonl, dir.join("snapshots.jsonl"));
        assert_eq!(std::fs::read_to_string(&paths.snapshots_jsonl).expect("snapshots"), "");
    }

    #[test]
    fn writes_prompt_artifacts_exactly() {
        let temp = tempdir().expect("tempdir");
        let repo_root = temp.path().join("repo");
        std::fs::create_dir_all(&repo_root).expect("repo dir");
        let paths =
            create_artifacts_in(temp.path(), &repo_root, "demo", "run").expect("artifact paths");
        let rendered = RenderedPrompt {
            system_prompt: "system text".to_string(),
            combined_prompt: "combined text".to_string(),
            version_manifest: "v1\nv2".to_string(),
        };

        write_prompt_artifact(&paths.prompt_txt, "raw text").expect("raw prompt");
        write_rendered_prompt(&paths, &rendered).expect("rendered prompt");

        assert_eq!(
            std::fs::read_to_string(&paths.prompt_txt).expect("raw"),
            "raw text"
        );
        assert_eq!(
            std::fs::read_to_string(&paths.system_prompt_txt).expect("system"),
            "system text"
        );
        assert_eq!(
            std::fs::read_to_string(&paths.combined_prompt_txt).expect("combined"),
            "combined text"
        );
        assert_eq!(
            std::fs::read_to_string(&paths.system_prompt_versions_txt).expect("versions"),
            "v1\nv2"
        );
        assert!(paths.dir.exists());
    }

    #[test]
    fn latest_run_dir_prefers_newest_timestamp_and_pid() {
        let temp = tempdir().expect("tempdir");
        let repo_root = temp.path().join("repo");
        std::fs::create_dir_all(&repo_root).expect("repo dir");

        let older = temp
            .path()
            .join(".local/state/vibe/repo/demo/runs/1700000000-1");
        let newer_pid = temp
            .path()
            .join(".local/state/vibe/repo/demo/runs/1700000000-2");
        let newest = temp
            .path()
            .join(".local/state/vibe/repo/demo/runs/1700000001-1");
        std::fs::create_dir_all(&older).expect("older dir");
        std::fs::create_dir_all(&newer_pid).expect("newer pid dir");
        std::fs::create_dir_all(&newest).expect("newest dir");

        let latest = latest_run_dir_in(temp.path(), &repo_root, "demo")
            .expect("latest run dir")
            .expect("run dir");

        assert_eq!(latest, newest);
    }
}
