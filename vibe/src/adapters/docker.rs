use std::{
    fs::File,
    io::{Read, Write},
    path::Path,
    process::{Command, Stdio},
    thread,
};

use crate::observe::ArtifactPaths;

const IMAGE: &str = "vibe-pi:0.5.0";
const AUTH_VARS: &[&str] = &[
    "ANTHROPIC_API_KEY",
    "OPENAI_API_KEY",
    "GEMINI_API_KEY",
    "DEEPSEEK_API_KEY",
    "AZURE_OPENAI_API_KEY",
    "AZURE_OPENAI_BASE_URL",
];
// Forward provider config into the container, but only allow complete
// credential groups to satisfy the host-side auth preflight.
const REQUIRED_AUTH_GROUPS: &[&[&str]] = &[
    &["ANTHROPIC_API_KEY"],
    &["OPENAI_API_KEY"],
    &["GEMINI_API_KEY"],
    &["DEEPSEEK_API_KEY"],
    &["AZURE_OPENAI_API_KEY", "AZURE_OPENAI_BASE_URL"],
];
const HOST_GIT_CONFIG_KEYS: &[(&str, &str)] = &[
    ("user.name", "VIBE_GIT_USER_NAME"),
    ("user.email", "VIBE_GIT_USER_EMAIL"),
];

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

fn auth_file_from_home(home: Option<&str>) -> Option<String> {
    let path = home.map(|home| format!("{home}/.pi/agent/auth.json"))?;
    let metadata = std::fs::metadata(&path).ok()?;
    if !metadata.is_file() {
        return None;
    }
    File::open(&path).ok()?;
    Some(path)
}

fn env_var_is_set(key: &str) -> bool {
    std::env::var(key)
        .ok()
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
}

fn has_provider_env() -> bool {
    REQUIRED_AUTH_GROUPS
        .iter()
        .any(|keys| keys.iter().all(|key| env_var_is_set(key)))
}

fn auth_is_configured(home: Option<&str>) -> bool {
    has_provider_env() || auth_file_from_home(home).is_some()
}

pub fn require_auth() -> Result<(), String> {
    let home = std::env::var("HOME").ok();

    if auth_is_configured(home.as_deref()) {
        Ok(())
    } else {
        Err("vibe requires provider auth via env vars or ~/.pi/agent/auth.json".to_string())
    }
}

struct DockerRunArgs<'a> {
    repo_root: &'a Path,
    git_common_dir: &'a Path,
    worktree: &'a Path,
    artifacts: &'a ArtifactPaths,
    model: &'a str,
    stderr_level: &'a str,
    insecure_tls: bool,
    snapshot_ref: &'a str,
    user: &'a HostUser,
}

/// Keep prompt/env wiring pure so tests can lock the Docker seam.
fn docker_run_args(args: &DockerRunArgs<'_>) -> Vec<String> {
    let DockerRunArgs {
        repo_root,
        git_common_dir,
        worktree,
        artifacts,
        model,
        stderr_level,
        insecure_tls,
        snapshot_ref,
        user,
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
    if *insecure_tls {
        run_args.push("-e".to_string());
        run_args.push("NODE_TLS_REJECT_UNAUTHORIZED=0".to_string());
    }
    run_args
}

pub fn run_task(
    repo_root: &Path,
    git_common_dir: &Path,
    worktree: &Path,
    artifacts: &ArtifactPaths,
    model: &str,
    stderr_level: &str,
    insecure_tls: bool,
) -> Result<i32, String> {
    let stderr_log =
        File::create(&artifacts.stderr_log).map_err(|e| format!("create stderr log: {e}"))?;
    let snapshot_ref = format!(
        "refs/vibe/snapshots/{}",
        worktree
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("run")
    );
    let user = host_user()?;
    let auth_file = auth_file_from_home(std::env::var("HOME").ok().as_deref());

    // Auth depends on host state, so the deterministic Docker prompt wiring stays separate.
    if insecure_tls {
        eprintln!("warning: --insecure-tls disables TLS certificate verification inside Docker");
    }

    let mut cmd = Command::new("docker");
    cmd.args(docker_run_args(&DockerRunArgs {
        repo_root,
        git_common_dir,
        worktree,
        artifacts,
        model,
        stderr_level,
        insecure_tls,
        snapshot_ref: &snapshot_ref,
        user: &user,
    }));
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
mod tests {
    use super::{
        auth_file_from_home, auth_is_configured, docker_run_args, require_auth, ArtifactPaths,
        DockerRunArgs, HostUser, AUTH_VARS,
    };
    use std::{
        ffi::OsString,
        fs,
        sync::{Mutex, OnceLock},
    };

    const ERROR_MESSAGE: &str = "vibe requires provider auth via env vars or ~/.pi/agent/auth.json";

    fn auth_env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn save_env(keys: &[&str]) -> Vec<(String, Option<OsString>)> {
        keys.iter()
            .map(|key| ((*key).to_string(), std::env::var_os(key)))
            .collect()
    }

    fn save_auth_env() -> Vec<(String, Option<OsString>)> {
        let mut keys = AUTH_VARS.to_vec();
        keys.push("HOME");
        save_env(&keys)
    }

    fn restore_env(saved: Vec<(String, Option<OsString>)>) {
        for (key, value) in saved {
            if let Some(value) = value {
                std::env::set_var(key, value);
            } else {
                std::env::remove_var(key);
            }
        }
    }

    fn clear_auth_env() {
        for key in AUTH_VARS {
            std::env::remove_var(key);
        }
    }

    #[test]
    fn auth_is_configured_accepts_real_credentials() {
        let _guard = auth_env_lock().lock().expect("lock auth env");
        let home = tempfile::tempdir().expect("tempdir");
        let saved = save_auth_env();

        std::env::set_var("HOME", home.path());
        clear_auth_env();
        std::env::set_var("OPENAI_API_KEY", "sk-test");

        assert!(auth_is_configured(home.path().to_str()));

        restore_env(saved);
    }

    #[test]
    fn auth_is_configured_accepts_auth_file() {
        let _guard = auth_env_lock().lock().expect("lock auth env");
        let home = tempfile::tempdir().expect("tempdir");
        let auth_dir = home.path().join(".pi/agent");
        fs::create_dir_all(&auth_dir).expect("mkdir auth dir");
        fs::write(auth_dir.join("auth.json"), b"{}").expect("write auth file");

        let saved = save_auth_env();

        std::env::set_var("HOME", home.path());
        clear_auth_env();

        assert!(auth_is_configured(home.path().to_str()));

        restore_env(saved);
    }

    #[test]
    fn auth_is_configured_rejects_missing_credentials_and_auth_file() {
        let _guard = auth_env_lock().lock().expect("lock auth env");
        let home = tempfile::tempdir().expect("tempdir");
        let saved = save_auth_env();

        std::env::set_var("HOME", home.path());
        clear_auth_env();

        assert!(!auth_is_configured(home.path().to_str()));

        restore_env(saved);
    }

    #[test]
    fn require_auth_accepts_auth_file_without_credentials() {
        let _guard = auth_env_lock().lock().expect("lock auth env");
        let home = tempfile::tempdir().expect("tempdir");
        let auth_dir = home.path().join(".pi/agent");
        fs::create_dir_all(&auth_dir).expect("mkdir auth dir");
        fs::write(auth_dir.join("auth.json"), b"{}").expect("write auth file");

        let saved = save_auth_env();

        std::env::set_var("HOME", home.path());
        clear_auth_env();

        let result = require_auth();

        restore_env(saved);

        assert!(result.is_ok());
    }

    #[test]
    fn require_auth_rejects_non_credential_provider_config() {
        let _guard = auth_env_lock().lock().expect("lock auth env");
        let home = tempfile::tempdir().expect("tempdir");
        let saved = save_auth_env();

        std::env::set_var("HOME", home.path());
        clear_auth_env();
        std::env::set_var("AZURE_OPENAI_BASE_URL", "https://example.invalid");

        let result = require_auth();

        restore_env(saved);

        assert_eq!(
            result.expect_err("non-credential config should fail"),
            ERROR_MESSAGE
        );
    }

    #[test]
    fn require_auth_accepts_complete_azure_credentials() {
        let _guard = auth_env_lock().lock().expect("lock auth env");
        let home = tempfile::tempdir().expect("tempdir");
        let saved = save_auth_env();

        std::env::set_var("HOME", home.path());
        clear_auth_env();
        std::env::set_var("AZURE_OPENAI_API_KEY", "azure-key");
        std::env::set_var("AZURE_OPENAI_BASE_URL", "https://example.invalid");

        let result = require_auth();

        restore_env(saved);

        assert!(result.is_ok());
    }

    #[test]
    fn require_auth_rejects_incomplete_azure_credentials() {
        let _guard = auth_env_lock().lock().expect("lock auth env");
        let home = tempfile::tempdir().expect("tempdir");
        let saved = save_auth_env();

        std::env::set_var("HOME", home.path());
        clear_auth_env();
        std::env::set_var("AZURE_OPENAI_API_KEY", "azure-key");

        let result = require_auth();

        restore_env(saved);

        assert_eq!(
            result.expect_err("missing Azure base URL should fail"),
            ERROR_MESSAGE
        );
    }

    #[test]
    fn docker_run_args_uses_combined_prompt_artifact() {
        let temp = tempfile::tempdir().expect("tempdir");
        let repo_root = temp.path().join("repo");
        let git_common_dir = temp.path().join("git");
        let worktree = temp.path().join("worktree");
        let artifacts = ArtifactPaths {
            dir: temp.path().join("artifacts"),
            prompt_txt: temp.path().join("artifacts/prompt.txt"),
            system_prompt_txt: temp.path().join("artifacts/system-prompt.txt"),
            combined_prompt_txt: temp.path().join("artifacts/combined-prompt.txt"),
            system_prompt_versions_txt: temp.path().join("artifacts/system-prompt-versions.txt"),
            state_json: temp.path().join("artifacts/state.json"),
            result_json: temp.path().join("artifacts/result.json"),
            log_txt: temp.path().join("artifacts/log.txt"),
            events_jsonl: temp.path().join("artifacts/events.jsonl"),
            stderr_log: temp.path().join("artifacts/agent.stderr.log"),
            extension_jsonl: temp.path().join("artifacts/extension-events.jsonl"),
            snapshots_jsonl: temp.path().join("artifacts/snapshots.jsonl"),
        };
        let user = HostUser {
            uid: "1000".to_string(),
            gid: "1001".to_string(),
        };

        let args = docker_run_args(&DockerRunArgs {
            repo_root: &repo_root,
            git_common_dir: &git_common_dir,
            worktree: &worktree,
            artifacts: &artifacts,
            model: "openai-codex/gpt-5.4",
            stderr_level: "info",
            insecure_tls: false,
            snapshot_ref: "refs/vibe/snapshots/run",
            user: &user,
        });

        assert!(args
            .iter()
            .any(|arg| arg == "VIBE_COMBINED_PROMPT_FILE=/artifacts/combined-prompt.txt"));
        assert!(!args
            .iter()
            .any(|arg| arg == "VIBE_PROMPT_FILE=/artifacts/prompt.txt"));
        assert!(!args
            .iter()
            .any(|arg| arg == "NODE_TLS_REJECT_UNAUTHORIZED=0"));
        assert!(args
            .iter()
            .any(|arg| arg == &format!("VIBE_REPO_ROOT={}", repo_root.display())));
    }

    #[test]
    fn docker_run_args_sets_insecure_tls_env() {
        let temp = tempfile::tempdir().expect("tempdir");
        let repo_root = temp.path().join("repo");
        let git_common_dir = temp.path().join("git");
        let worktree = temp.path().join("worktree");
        let artifacts = ArtifactPaths {
            dir: temp.path().join("artifacts"),
            prompt_txt: temp.path().join("artifacts/prompt.txt"),
            system_prompt_txt: temp.path().join("artifacts/system-prompt.txt"),
            combined_prompt_txt: temp.path().join("artifacts/combined-prompt.txt"),
            system_prompt_versions_txt: temp.path().join("artifacts/system-prompt-versions.txt"),
            state_json: temp.path().join("artifacts/state.json"),
            result_json: temp.path().join("artifacts/result.json"),
            log_txt: temp.path().join("artifacts/log.txt"),
            events_jsonl: temp.path().join("artifacts/events.jsonl"),
            stderr_log: temp.path().join("artifacts/agent.stderr.log"),
            extension_jsonl: temp.path().join("artifacts/extension-events.jsonl"),
            snapshots_jsonl: temp.path().join("artifacts/snapshots.jsonl"),
        };
        let user = HostUser {
            uid: "1000".to_string(),
            gid: "1001".to_string(),
        };

        let args = docker_run_args(&DockerRunArgs {
            repo_root: &repo_root,
            git_common_dir: &git_common_dir,
            worktree: &worktree,
            artifacts: &artifacts,
            model: "openai-codex/gpt-5.4",
            stderr_level: "info",
            insecure_tls: true,
            snapshot_ref: "refs/vibe/snapshots/run",
            user: &user,
        });

        assert!(args
            .iter()
            .any(|arg| arg == "NODE_TLS_REJECT_UNAUTHORIZED=0"));
    }

    #[test]
    fn auth_file_from_home_rejects_directory_auth_json() {
        let home = tempfile::tempdir().expect("tempdir");
        let auth_path = home.path().join(".pi/agent/auth.json");
        fs::create_dir_all(&auth_path).expect("mkdir fake auth dir");

        assert!(auth_file_from_home(home.path().to_str()).is_none());
    }

    #[test]
    fn auth_file_from_home_rejects_missing_auth_json() {
        let home = tempfile::tempdir().expect("tempdir");

        assert!(auth_file_from_home(home.path().to_str()).is_none());
    }
}
