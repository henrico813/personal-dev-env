use crate::prompts::RenderedPrompt;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Artifact paths for one Vibe run.
pub struct ArtifactPaths {
    pub dir: PathBuf,
    pub prompt_txt: PathBuf,
    pub system_prompt_txt: PathBuf,
    pub combined_prompt_txt: PathBuf,
    pub system_prompt_versions_txt: PathBuf,
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
        system_prompt_txt: dir.join("system-prompt.txt"),
        combined_prompt_txt: dir.join("combined-prompt.txt"),
        system_prompt_versions_txt: dir.join("system-prompt-versions.txt"),
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
        assert_eq!(paths.events_jsonl, dir.join("events.jsonl"));
        assert_eq!(paths.stderr_log, dir.join("agent.stderr.log"));
        assert_eq!(paths.extension_jsonl, dir.join("extension-events.jsonl"));
        assert_eq!(paths.snapshots_jsonl, dir.join("snapshots.jsonl"));
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
}
