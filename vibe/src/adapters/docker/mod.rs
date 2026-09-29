use std::{
    fs::File,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
};

mod auth;
mod skills;

use crate::{observe::ArtifactPaths, worktree::SandboxMounts};

pub(crate) use auth::prepare_provider_auth;
use auth::{auth_env_args, HOST_GIT_CONFIG_KEYS};
pub(crate) use skills::{prepare_repository_skills, prepare_user_skills, PreparedSkills};
use skills::{
    reject_user_skills_writable_overlap, revalidate_skill_root, validate_repository_skills_mount,
};

const IMAGE: &str = "vibe-pi:0.8.3";

struct HostUser {
    uid: String,
    gid: String,
}

pub fn ensure_image(runtime_root: &Path) -> Result<(), String> {
    let dockerfile = runtime_root.join("docker/Dockerfile");
    if !dockerfile.exists() {
        return Err(format!(
            "vibe runtime assets unavailable: {}",
            dockerfile.display()
        ));
    }
    let out = Command::new("docker")
        .args([
            "build",
            "-t",
            IMAGE,
            "-f",
            dockerfile.to_str().unwrap_or(""),
            runtime_root.to_str().unwrap_or(""),
        ])
        .output()
        .map_err(|e| format!("docker build: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        Err("docker build failed from extracted runtime assets".to_string())
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

struct DockerRunArgs<'a> {
    repo_root: &'a Path,
    git_common_dir: &'a Path,
    worktree: &'a Path,
    inputs: &'a [PathBuf],
    artifacts: &'a ArtifactPaths,
    model: &'a str,
    stderr_level: &'a str,
    insecure_tls: bool,
    snapshot_ref: &'a str,
    user: &'a HostUser,
    pi_agent_dir: Option<&'a Path>,
    user_skills_dir: Option<&'a Path>,
    repository_skills_dir: Option<&'a Path>,
}

/// Keep prompt/env wiring pure so tests can lock the Docker seam.
fn docker_run_args(args: &DockerRunArgs<'_>) -> Vec<String> {
    let DockerRunArgs {
        repo_root,
        git_common_dir,
        worktree,
        inputs,
        artifacts,
        model,
        stderr_level,
        insecure_tls,
        snapshot_ref,
        user,
        pi_agent_dir,
        user_skills_dir,
        repository_skills_dir,
    } = args;

    let mut run_args = vec![
        "run".to_string(),
        "--rm".to_string(),
        "--user".to_string(),
        format!("{}:{}", user.uid, user.gid),
        "--tmpfs".to_string(),
        format!("/vibe-home:uid={},gid={},mode=700", user.uid, user.gid),
        "-v".to_string(),
        format!("{}:{}", worktree.display(), worktree.display()),
        "-v".to_string(),
        format!("{}:{}", git_common_dir.display(), git_common_dir.display()),
        "-v".to_string(),
        format!("{}:/artifacts", artifacts.dir.display()),
        "-v".to_string(),
        format!("{}:/artifacts/run.json:ro", artifacts.run_json.display()),
        "-w".to_string(),
        worktree.to_str().unwrap_or("").to_string(),
        "-e".to_string(),
        "HOME=/vibe-home".to_string(),
        "-e".to_string(),
        format!("VIBE_MODEL={model}"),
        "-e".to_string(),
        format!("VIBE_STDERR_LEVEL={stderr_level}"),
        "-e".to_string(),
        "VIBE_COMBINED_PROMPT_FILE=/artifacts/combined-prompt.txt".to_string(),
        "-e".to_string(),
        "VIBE_COMMIT_MESSAGE_FILE=/artifacts/commit-message.txt".to_string(),
        "-e".to_string(),
        format!(
            "VIBE_EXTENSION_LOG=/artifacts/{}",
            artifacts
                .extension_jsonl
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("extension-events.jsonl")
        ),
        "-e".to_string(),
        "VIBE_SNAPSHOT_LOG=/artifacts/snapshots.jsonl".to_string(),
        "-e".to_string(),
        format!("VIBE_SNAPSHOT_REF={snapshot_ref}"),
        "-e".to_string(),
        "VIBE_GIT_HOOKS_DIR=/opt/vibe/hooks".to_string(),
        "-e".to_string(),
        format!("VIBE_REPO_ROOT={}", repo_root.display()),
    ];
    for input in *inputs {
        run_args.extend([
            "--mount".to_string(),
            format!(
                "type=bind,src={},dst={},readonly",
                input.display(),
                input.display()
            ),
        ]);
    }
    if let Some(user_skills_dir) = user_skills_dir {
        run_args.extend([
            "--mount".to_string(),
            format!(
                "type=bind,src={},dst=/vibe-home/.agents/skills,readonly",
                user_skills_dir.display()
            ),
        ]);
    }
    if let Some(repository_skills_dir) = repository_skills_dir {
        run_args.extend([
            "--mount".to_string(),
            format!(
                "type=bind,src={},dst={},readonly",
                repository_skills_dir.display(),
                repository_skills_dir.display()
            ),
            "-e".to_string(),
            format!("VIBE_REPO_SKILLS_DIR={}", repository_skills_dir.display()),
        ]);
    }
    if let Some(pi_agent_dir) = pi_agent_dir {
        // Pi rotates OAuth tokens and locks beside auth.json, so the
        // directory must stay writable and shared across runs.
        run_args.extend([
            "-v".to_string(),
            format!("{}:/vibe-home/.pi/agent:rw", pi_agent_dir.display()),
        ]);
    }
    if *insecure_tls {
        run_args.push("-e".to_string());
        run_args.push("NODE_TLS_REJECT_UNAUTHORIZED=0".to_string());
    }
    run_args
}

pub fn run_task(
    mounts: &SandboxMounts,
    artifacts: &ArtifactPaths,
    model: &str,
    stderr_level: &str,
    insecure_tls: bool,
    pi_agent_dir: Option<&Path>,
    prepared_skills: &PreparedSkills,
) -> Result<i32, String> {
    let stderr_log =
        File::create(&artifacts.stderr_log).map_err(|e| format!("create stderr log: {e}"))?;
    let snapshot_ref = format!(
        "refs/vibe/snapshots/{}",
        mounts
            .worktree
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("run")
    );
    let user = host_user()?;
    // Auth depends on host state, so the deterministic Docker prompt wiring stays separate.
    if insecure_tls {
        eprintln!("warning: --insecure-tls disables TLS certificate verification inside Docker");
    }
    let mut git_env_args = Vec::new();
    for (git_key, env_key) in HOST_GIT_CONFIG_KEYS {
        let out = Command::new("git")
            .args(["config", "--global", git_key])
            .output()
            .map_err(|e| format!("read git {git_key}: {e}"))?;
        if out.status.success() {
            let value = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !value.is_empty() {
                git_env_args.extend(["-e".to_string(), format!("{env_key}={value}")]);
            }
        }
    }
    let user_skills_dir = match prepared_skills.user.as_ref() {
        Some(prepared) => revalidate_skill_root(prepared)?,
        None => None,
    };
    if let Some(user_skills_dir) = &user_skills_dir {
        reject_user_skills_writable_overlap(
            user_skills_dir,
            &mounts.worktree,
            &mounts.git_common_dir,
            &artifacts.dir,
            pi_agent_dir,
        )?;
    }
    let repository_skills_dir = match prepared_skills.repository.as_ref() {
        Some(prepared) => revalidate_skill_root(prepared)?,
        None => None,
    };
    if let Some(repository_skills_dir) = &repository_skills_dir {
        validate_repository_skills_mount(
            repository_skills_dir,
            &mounts.worktree,
            &mounts.git_common_dir,
            &artifacts.dir,
            pi_agent_dir,
        )?;
    }
    let mut cmd = Command::new("docker");
    cmd.args(docker_run_args(&DockerRunArgs {
        repo_root: &mounts.repo_root,
        git_common_dir: &mounts.git_common_dir,
        worktree: &mounts.worktree,
        inputs: &mounts.inputs,
        artifacts,
        model,
        stderr_level,
        insecure_tls,
        snapshot_ref: &snapshot_ref,
        user: &user,
        pi_agent_dir,
        user_skills_dir: user_skills_dir.as_deref(),
        repository_skills_dir: repository_skills_dir.as_deref(),
    }));
    cmd.args(auth_env_args(model));
    cmd.args(git_env_args);
    let mut child = cmd
        .arg(IMAGE)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("docker run: {e}"))?;

    let mut child_stderr = child
        .stderr
        .take()
        .ok_or_else(|| "docker run: missing stderr pipe".to_string())?;
    let stderr_thread = thread::spawn(move || -> Result<(), String> {
        let mut host_stderr = std::io::stderr();
        let mut stderr_log = stderr_log;
        let mut buf = [0_u8; 8192];
        loop {
            let n = child_stderr
                .read(&mut buf)
                .map_err(|e| format!("read docker stderr: {e}"))?;
            if n == 0 {
                break;
            }
            host_stderr
                .write_all(&buf[..n])
                .map_err(|e| format!("write host stderr: {e}"))?;
            stderr_log
                .write_all(&buf[..n])
                .map_err(|e| format!("write stderr log: {e}"))?;
        }
        host_stderr
            .flush()
            .map_err(|e| format!("flush host stderr: {e}"))?;
        stderr_log
            .flush()
            .map_err(|e| format!("flush stderr log: {e}"))?;
        Ok(())
    });

    let status = child
        .wait()
        .map_err(|e| format!("wait for docker run: {e}"))?;
    if let Err(err) = stderr_thread
        .join()
        .map_err(|_| "join stderr copier thread failed".to_string())
        .and_then(|result| result)
    {
        eprintln!("warning: stderr copier failed: {err}");
    }
    Ok(status.code().unwrap_or(-1))
}

#[cfg(test)]
mod tests;
