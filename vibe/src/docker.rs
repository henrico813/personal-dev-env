use std::fs::File;
use std::path::Path;
use std::process::{Command, Stdio};

use crate::paths::RunPaths;

const IMAGE: &str = "vibe-pi:0.1.0";
const AUTH_VARS: &[&str] = &[
    "ANTHROPIC_API_KEY",
    "OPENAI_API_KEY",
    "GEMINI_API_KEY",
    "DEEPSEEK_API_KEY",
    "AZURE_OPENAI_API_KEY",
    "AZURE_OPENAI_BASE_URL",
];
const HOST_GIT_CONFIG_KEYS: &[(&str, &str)] = &[
    ("user.name", "VIBE_GIT_USER_NAME"),
    ("user.email", "VIBE_GIT_USER_EMAIL"),
];

struct HostUser {
    uid: String,
    gid: String,
}

pub fn ensure_image(checkout_root: &Path) -> Result<(), String> {
    let out = Command::new("docker")
        .args([
            "build",
            "-t",
            IMAGE,
            "-f",
            checkout_root
                .join("vibe/docker/Dockerfile")
                .to_str()
                .unwrap_or(""),
            checkout_root.to_str().unwrap_or(""),
        ])
        .output()
        .map_err(|e| format!("docker build: {e}"))?;
    if !out.stdout.is_empty() {
        eprint!("{}", String::from_utf8_lossy(&out.stdout));
    }
    if !out.stderr.is_empty() {
        eprint!("{}", String::from_utf8_lossy(&out.stderr));
    }
    if out.status.success() {
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

fn host_user() -> Result<HostUser, String> {
    let uid = Command::new("id")
        .args(["-u"])
        .output()
        .map_err(|e| format!("read uid: {e}"))?;
    if !uid.status.success() {
        return Err("read uid failed".to_string());
    }
    let gid = Command::new("id")
        .args(["-g"])
        .output()
        .map_err(|e| format!("read gid: {e}"))?;
    if !gid.status.success() {
        return Err("read gid failed".to_string());
    }
    Ok(HostUser {
        uid: String::from_utf8_lossy(&uid.stdout).trim().to_string(),
        gid: String::from_utf8_lossy(&gid.stdout).trim().to_string(),
    })
}

pub fn run_step(
    canonical_repo_root: &Path,
    git_common_dir: &Path,
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
    let user = host_user()?;
    let auth_file = std::env::var("HOME")
        .ok()
        .map(|home| format!("{home}/.pi/agent/auth.json"))
        .filter(|path| Path::new(path).exists());

    let mut cmd = Command::new("docker");
    cmd.args([
        "run",
        "--rm",
        "--user",
        &format!("{}:{}", user.uid, user.gid),
        "--tmpfs",
        &format!("/vibe-home:uid={},gid={},mode=700", user.uid, user.gid),
        "-v",
        &format!("{}:{}", worktree.display(), worktree.display()),
        "-v",
        &format!("{}:{}", git_common_dir.display(), git_common_dir.display()),
        "-v",
        &format!("{}:/artifacts", run_paths.dir.display()),
        "-w",
        worktree.to_str().unwrap_or(""),
        "-e",
        "HOME=/vibe-home",
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
        "-e",
        &format!("VIBE_CANONICAL_REPO_ROOT={}", canonical_repo_root.display()),
    ]);
    for key in AUTH_VARS {
        if let Ok(value) = std::env::var(key) {
            cmd.args(["-e", &format!("{key}={value}")]);
        }
    }
    for (git_key, env_key) in HOST_GIT_CONFIG_KEYS {
        let out = Command::new("git")
            .args(["config", "--global", git_key])
            .output()
            .map_err(|e| format!("read git {git_key}: {e}"))?;
        if out.status.success() {
            let value = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !value.is_empty() {
                cmd.args(["-e", &format!("{env_key}={value}")]);
            }
        }
    }
    if let Some(path) = auth_file.as_deref() {
        cmd.args(["-v", &format!("{path}:/vibe-auth/auth.json:ro")]);
        cmd.args(["-e", "VIBE_AUTH_FILE=/vibe-auth/auth.json"]);
    }
    let status = cmd
        .arg(IMAGE)
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr))
        .status()
        .map_err(|e| format!("docker run: {e}"))?;
    Ok(status.code().unwrap_or(-1))
}
