use std::fs::File;
use std::path::Path;
use std::process::{Command, Stdio};

use crate::paths::RunPaths;

const IMAGE: &str = "vibe-pi:0.1.0";

pub fn ensure_image(repo_root: &Path) -> Result<(), String> {
    let status = Command::new("docker")
        .args([
            "build",
            "-t",
            IMAGE,
            "-f",
            repo_root
                .join("vibe/docker/Dockerfile")
                .to_str()
                .unwrap_or(""),
            repo_root.to_str().unwrap_or(""),
        ])
        .status()
        .map_err(|e| format!("docker build: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err("docker build failed".to_string())
    }
}

/// Fail early so setup errors are distinguishable from agent failures.
pub fn require_docker() -> Result<(), String> {
    let status = Command::new("docker")
        .arg("version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|e| format!("docker not available: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err("docker not available".to_string())
    }
}

pub fn run_step(
    repo_root: &Path,
    worktree: &Path,
    run_paths: &RunPaths,
    model: &str,
) -> Result<i32, String> {
    let stdout =
        File::create(&run_paths.events_jsonl).map_err(|e| format!("create events log: {e}"))?;
    let stderr =
        File::create(&run_paths.stderr_log).map_err(|e| format!("create stderr log: {e}"))?;
    let snapshot_ref = format!(
        "refs/vibe/snapshots/{}",
        worktree
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("run")
    );
    let status = Command::new("docker")
        .args([
            "run",
            "--rm",
            "-v",
            &format!("{}:{}", repo_root.display(), repo_root.display()),
            "-v",
            &format!("{}:/artifacts", run_paths.dir.display()),
            "-w",
            worktree.to_str().unwrap_or(""),
            "-e",
            &format!("VIBE_MODEL={model}"),
            "-e",
            "VIBE_PROMPT_FILE=/artifacts/prompt.txt",
            "-e",
            "VIBE_EXTENSION_LOG=/artifacts/extension-events.jsonl",
            "-e",
            "VIBE_SNAPSHOT_LOG=/artifacts/snapshots.jsonl",
            "-e",
            &format!("VIBE_SNAPSHOT_REF={snapshot_ref}"),
            "-e",
            "VIBE_GIT_HOOKS_DIR=/opt/vibe/hooks",
            IMAGE,
        ])
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr))
        .status()
        .map_err(|e| format!("docker run: {e}"))?;
    Ok(status.code().unwrap_or(-1))
}
