use std::{
    ffi::OsStr,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{SystemTime, UNIX_EPOCH},
};

use crate::{observe::ArtifactPaths, worktree::SandboxMounts};

const IMAGE: &str = "vibe-pi:0.6.0";
const AUTH_VARS: &[&str] = &[
    "ANTHROPIC_API_KEY",
    "OPENAI_API_KEY",
    "GEMINI_API_KEY",
    "DEEPSEEK_API_KEY",
    "AZURE_OPENAI_API_KEY",
    "AZURE_OPENAI_BASE_URL",
    "OPENCODE_API_KEY",
];
// Forward provider config into the container, but only allow complete
// credential groups to satisfy the host-side auth preflight.
const REQUIRED_AUTH_GROUPS: &[&[&str]] = &[
    &["ANTHROPIC_API_KEY"],
    &["OPENAI_API_KEY"],
    &["GEMINI_API_KEY"],
    &["DEEPSEEK_API_KEY"],
    &["AZURE_OPENAI_API_KEY", "AZURE_OPENAI_BASE_URL"],
    &["OPENCODE_API_KEY"],
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

fn auth_env_args() -> Vec<String> {
    AUTH_VARS
        .iter()
        .filter(|key| env_var_is_set(key))
        .flat_map(|key| ["-e".to_string(), (*key).to_string()])
        .collect()
}

pub(crate) fn prepare_provider_auth(home: Option<&str>) -> Result<Option<PathBuf>, String> {
    let pi_agent_dir = home.and_then(|home| {
        let pi_agent_dir = PathBuf::from(home).join(".pi/agent");
        let auth_file = pi_agent_dir.join("auth.json");
        let metadata = std::fs::metadata(&auth_file).ok()?;
        if !metadata.is_file() {
            return None;
        }
        File::open(auth_file).ok()?;

        // Pi rotates OAuth tokens and writes refresh locks beside auth.json.
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .ok()?
            .as_nanos();
        let probe = pi_agent_dir.join(format!(".vibe-write-check-{}-{nonce}", std::process::id()));
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&probe)
            .ok()?;
        std::fs::remove_file(probe).ok()?;
        Some(pi_agent_dir)
    });

    if has_provider_env() || pi_agent_dir.is_some() {
        Ok(pi_agent_dir)
    } else {
        Err("vibe requires provider auth via env vars or ~/.pi/agent/auth.json".to_string())
    }
}

#[derive(Debug)]
pub(crate) struct SharedSkillsDir {
    path: PathBuf,
    device: u64,
    inode: u64,
}

fn inspect_shared_skills_with<F>(
    skills_dir: &Path,
    read_dir: F,
) -> Result<Option<SharedSkillsDir>, String>
where
    F: FnOnce(&Path) -> std::io::Result<()>,
{
    match fs::symlink_metadata(skills_dir) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(format!(
            "shared skills path cannot be a symlink: {}",
            skills_dir.display()
        )),
        Ok(metadata) if metadata.is_dir() => {
            let resolved = match fs::canonicalize(skills_dir) {
                Ok(resolved) => resolved,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
                Err(error) => {
                    return Err(format!(
                        "resolve shared skills {}: {error}",
                        skills_dir.display()
                    ))
                }
            };
            let resolved_metadata = match fs::metadata(&resolved) {
                Ok(metadata) => metadata,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
                Err(error) => {
                    return Err(format!(
                        "read shared skills metadata {}: {error}",
                        resolved.display()
                    ))
                }
            };
            let resolved_text = resolved.to_str().ok_or_else(|| {
                format!(
                    "resolved shared skills path must be valid UTF-8: {}",
                    resolved.display()
                )
            })?;
            if resolved_text.contains(',') {
                return Err(format!(
                    "resolved shared skills path cannot contain commas: {}",
                    resolved.display()
                ));
            }
            if metadata.dev() != resolved_metadata.dev()
                || metadata.ino() != resolved_metadata.ino()
            {
                return Err(format!(
                    "shared skills path changed during validation: {}",
                    skills_dir.display()
                ));
            }
            match read_dir(&resolved) {
                Ok(()) => Ok(Some(SharedSkillsDir {
                    path: resolved,
                    device: metadata.dev(),
                    inode: metadata.ino(),
                })),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
                Err(error) => Err(format!(
                    "read shared skills {}: {error}",
                    resolved.display()
                )),
            }
        }
        Ok(_) => Err(format!(
            "shared skills path is not a directory: {}",
            skills_dir.display()
        )),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!(
            "read shared skills metadata {}: {error}",
            skills_dir.display()
        )),
    }
}

fn prepare_shared_skills_with<F>(
    home: Option<&OsStr>,
    read_dir: F,
) -> Result<Option<SharedSkillsDir>, String>
where
    F: FnOnce(&Path) -> std::io::Result<()>,
{
    let Some(home) = home else {
        return Ok(None);
    };
    let home_path = PathBuf::from(home);
    let skills_dir = home_path.join(".agents/skills");
    let prepared = inspect_shared_skills_with(&skills_dir, read_dir)?;
    if prepared.is_none() {
        return Ok(None);
    }

    let home = home
        .to_str()
        .ok_or_else(|| "HOME must be valid UTF-8 to mount shared skills".to_string())?;
    if home.is_empty() || !home_path.is_absolute() {
        return Err("HOME must be an absolute path to mount shared skills".to_string());
    }
    if home.contains(',') {
        return Err("HOME cannot contain commas when mounting shared skills".to_string());
    }
    Ok(prepared)
}

pub(crate) fn prepare_shared_skills(
    home: Option<&OsStr>,
) -> Result<Option<SharedSkillsDir>, String> {
    prepare_shared_skills_with(home, |path| fs::read_dir(path).map(|_| ()))
}

fn revalidate_shared_skills(prepared: &SharedSkillsDir) -> Result<Option<PathBuf>, String> {
    let Some(current) =
        inspect_shared_skills_with(&prepared.path, |path| fs::read_dir(path).map(|_| ()))?
    else {
        return Ok(None);
    };
    if current.device != prepared.device || current.inode != prepared.inode {
        return Err(format!(
            "shared skills path changed after validation: {}",
            prepared.path.display()
        ));
    }
    Ok(Some(current.path))
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
    shared_skills_dir: Option<&'a Path>,
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
        shared_skills_dir,
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
    if let Some(shared_skills_dir) = shared_skills_dir {
        run_args.extend([
            "--mount".to_string(),
            format!(
                "type=bind,src={},dst=/vibe-home/.agents/skills,readonly",
                shared_skills_dir.display()
            ),
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
    shared_skills: Option<&SharedSkillsDir>,
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
    let shared_skills_dir = match shared_skills {
        Some(prepared) => revalidate_shared_skills(prepared)?,
        None => None,
    };
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
        shared_skills_dir: shared_skills_dir.as_deref(),
    }));
    cmd.args(auth_env_args());
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
mod tests {
    use super::{
        auth_env_args, docker_run_args, prepare_provider_auth, prepare_shared_skills,
        prepare_shared_skills_with, revalidate_shared_skills, ArtifactPaths, DockerRunArgs,
        HostUser, AUTH_VARS,
    };
    use crate::state::home_env_lock;
    use std::{
        ffi::OsString,
        fs,
        path::Path,
    };

    const ERROR_MESSAGE: &str = "vibe requires provider auth via env vars or ~/.pi/agent/auth.json";

    fn auth_env_lock() -> &'static std::sync::Mutex<()> {
        home_env_lock()
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

    fn test_artifacts(dir: &Path) -> ArtifactPaths {
        let artifacts = dir.join("artifacts");
        ArtifactPaths {
            dir: artifacts.clone(),
            run_id: "run-id".to_string(),
            prompt_txt: artifacts.join("prompt.txt"),
            system_prompt_txt: artifacts.join("system-prompt.txt"),
            combined_prompt_txt: artifacts.join("combined-prompt.txt"),
            system_prompt_versions_txt: artifacts.join("system-prompt-versions.txt"),
            state_json: artifacts.join("run.json"),
            result_json: artifacts.join("result.json"),
            run_json: artifacts.join("run.json"),
            vibe_log: artifacts.join("vibe.log"),
            events_jsonl: artifacts.join("events.jsonl"),
            stderr_log: artifacts.join("agent.stderr.log"),
            extension_jsonl: artifacts.join("extension-events.jsonl"),
            snapshots_jsonl: artifacts.join("snapshots.jsonl"),
            summary_json: artifacts.join("summary.json"),
            runs_index_jsonl: dir.join("runs_index.jsonl"),
        }
    }

    #[test]
    fn provider_auth_accepts_env_credentials() {
        let _guard = auth_env_lock().lock().expect("lock auth env");
        let home = tempfile::tempdir().expect("tempdir");
        let saved = save_auth_env();

        std::env::set_var("HOME", home.path());
        clear_auth_env();
        std::env::set_var("OPENAI_API_KEY", "sk-test");

        assert!(prepare_provider_auth(home.path().to_str()).is_ok());

        restore_env(saved);
    }

    #[test]
    fn provider_auth_accepts_opencode_credentials() {
        let _guard = auth_env_lock().lock().expect("lock auth env");
        let home = tempfile::tempdir().expect("tempdir");
        let saved = save_auth_env();

        std::env::set_var("HOME", home.path());
        clear_auth_env();
        std::env::set_var("OPENCODE_API_KEY", "sk-test");

        assert!(prepare_provider_auth(home.path().to_str()).is_ok());
        assert!(auth_env_args().iter().any(|arg| arg == "OPENCODE_API_KEY"));

        restore_env(saved);
    }

    #[test]
    fn shared_skills_accept_missing_directory() {
        let home = tempfile::tempdir().expect("tempdir");

        assert_eq!(
            prepare_shared_skills(Some(home.path().as_os_str()))
                .expect("missing skills are optional"),
            None
        );
    }

    #[test]
    fn shared_skills_find_directory() {
        let home = tempfile::tempdir().expect("tempdir");
        let skills_dir = home.path().join(".agents/skills");
        fs::create_dir_all(&skills_dir).expect("mkdir skills");

        let prepared = prepare_shared_skills(Some(home.path().as_os_str()))
            .expect("skills path");

        assert_eq!(
            prepared.path,
            fs::canonicalize(skills_dir).expect("resolve skills")
        );
    }

    #[test]
    fn shared_skills_reject_file() {
        let home = tempfile::tempdir().expect("tempdir");
        let skills_dir = home.path().join(".agents/skills");
        fs::create_dir_all(skills_dir.parent().expect("skills parent")).expect("mkdir parent");
        fs::write(&skills_dir, b"not a directory").expect("write skills file");

        let error = prepare_shared_skills(Some(home.path().as_os_str()))
            .expect_err("a file cannot be mounted as the skills directory");

        assert!(error.contains("shared skills path is not a directory"));
    }

    #[test]
    fn shared_skills_allow_missing_unsafe_home() {
        let home = tempfile::tempdir().expect("tempdir");
        let missing = home.path().join("missing,home");

        assert!(prepare_shared_skills(Some(missing.as_os_str()))
            .expect("missing skills are optional")
            .is_none());
    }

    #[test]
    fn shared_skills_reject_unsafe_home() {
        let parent = tempfile::tempdir().expect("tempdir");
        let home = parent.path().join("home,with-comma");
        fs::create_dir_all(home.join(".agents/skills")).expect("mkdir skills");

        assert!(prepare_shared_skills(Some(home.as_os_str())).is_err());
    }

    #[test]
    fn shared_skills_reject_symlink() {
        let home = tempfile::tempdir().expect("tempdir");
        let target = home.path().join("target");
        let skills_dir = home.path().join(".agents/skills");
        fs::create_dir_all(&target).expect("mkdir target");
        fs::create_dir_all(skills_dir.parent().expect("skills parent")).expect("mkdir parent");
        std::os::unix::fs::symlink(target, &skills_dir).expect("symlink skills");

        let error = prepare_shared_skills(Some(home.path().as_os_str()))
            .expect_err("symlinked skills must fail setup");

        assert!(error.contains("cannot be a symlink"));
    }

    #[test]
    fn shared_skills_reject_unsafe_resolved_path() {
        let parent = tempfile::tempdir().expect("tempdir");
        let home = parent.path().join("home");
        let target = parent.path().join("target,with-comma");
        fs::create_dir_all(&home).expect("mkdir home");
        fs::create_dir_all(target.join("skills")).expect("mkdir target skills");
        std::os::unix::fs::symlink(&target, home.join(".agents")).expect("symlink agents");

        let error = prepare_shared_skills(Some(home.as_os_str()))
            .expect_err("unsafe resolved path must fail setup");

        assert!(error.contains("resolved shared skills path cannot contain commas"));
    }

    #[test]
    fn shared_skills_omit_disappeared_directory() {
        let home = tempfile::tempdir().expect("tempdir");
        let skills_dir = home.path().join(".agents/skills");
        fs::create_dir_all(&skills_dir).expect("mkdir skills");
        let prepared = prepare_shared_skills(Some(home.path().as_os_str()))
            .expect("read skills path")
            .expect("skills path");
        fs::remove_dir(&skills_dir).expect("remove skills");

        assert!(revalidate_shared_skills(&prepared)
            .expect("disappeared skills are optional")
            .is_none());
    }

    #[test]
    fn shared_skills_reject_replacement() {
        let home = tempfile::tempdir().expect("tempdir");
        let skills_dir = home.path().join(".agents/skills");
        let original = home.path().join("original-skills");
        fs::create_dir_all(&skills_dir).expect("mkdir skills");
        let prepared = prepare_shared_skills(Some(home.path().as_os_str()))
            .expect("read skills path")
            .expect("skills path");
        fs::rename(&skills_dir, &original).expect("move original skills");
        fs::create_dir(&skills_dir).expect("replace skills");

        let error =
            revalidate_shared_skills(&prepared).expect_err("replacement must fail before launch");

        assert!(error.contains("changed after validation"));
    }

    #[test]
    fn shared_skills_reject_unreadable_directory() {
        let home = tempfile::tempdir().expect("tempdir");
        let skills_dir = home.path().join(".agents/skills");
        fs::create_dir_all(&skills_dir).expect("mkdir skills");

        let error = prepare_shared_skills_with(Some(home.path().as_os_str()), |_| {
            Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "permission denied",
            ))
        })
        .expect_err("unreadable skills must fail setup");

        assert!(error.contains("read shared skills"));
    }

    #[test]
    fn provider_auth_accepts_empty_pi_object() {
        let _guard = auth_env_lock().lock().expect("lock auth env");
        let home = tempfile::tempdir().expect("tempdir");
        let auth_dir = home.path().join(".pi/agent");
        fs::create_dir_all(&auth_dir).expect("mkdir auth dir");
        fs::write(auth_dir.join("auth.json"), b"{}").expect("write auth file");

        let saved = save_auth_env();

        std::env::set_var("HOME", home.path());
        clear_auth_env();

        assert!(prepare_provider_auth(home.path().to_str()).is_ok());

        restore_env(saved);
    }

    #[test]
    fn provider_auth_rejects_missing_credentials() {
        let _guard = auth_env_lock().lock().expect("lock auth env");
        let home = tempfile::tempdir().expect("tempdir");
        let saved = save_auth_env();

        std::env::set_var("HOME", home.path());
        clear_auth_env();

        assert!(prepare_provider_auth(home.path().to_str()).is_err());

        restore_env(saved);
    }

    #[test]
    fn provider_auth_rejects_incomplete_config() {
        let _guard = auth_env_lock().lock().expect("lock auth env");
        let home = tempfile::tempdir().expect("tempdir");
        let saved = save_auth_env();

        std::env::set_var("HOME", home.path());
        clear_auth_env();
        std::env::set_var("AZURE_OPENAI_BASE_URL", "https://example.invalid");

        let result = prepare_provider_auth(home.path().to_str());

        restore_env(saved);

        assert_eq!(
            result.expect_err("non-credential config should fail"),
            ERROR_MESSAGE
        );
    }

    #[test]
    fn provider_auth_accepts_complete_azure() {
        let _guard = auth_env_lock().lock().expect("lock auth env");
        let home = tempfile::tempdir().expect("tempdir");
        let saved = save_auth_env();

        std::env::set_var("HOME", home.path());
        clear_auth_env();
        std::env::set_var("AZURE_OPENAI_API_KEY", "azure-key");
        std::env::set_var("AZURE_OPENAI_BASE_URL", "https://example.invalid");

        let result = prepare_provider_auth(home.path().to_str());

        restore_env(saved);

        assert!(result.is_ok());
    }

    #[test]
    fn provider_auth_rejects_incomplete_azure() {
        let _guard = auth_env_lock().lock().expect("lock auth env");
        let home = tempfile::tempdir().expect("tempdir");
        let saved = save_auth_env();

        std::env::set_var("HOME", home.path());
        clear_auth_env();
        std::env::set_var("AZURE_OPENAI_API_KEY", "azure-key");

        let result = prepare_provider_auth(home.path().to_str());

        restore_env(saved);

        assert_eq!(
            result.expect_err("missing Azure base URL should fail"),
            ERROR_MESSAGE
        );
    }

    #[test]
    fn auth_env_args_exclude_secret_values() {
        let _guard = auth_env_lock().lock().expect("lock auth env");
        let saved = save_auth_env();

        clear_auth_env();
        std::env::set_var("OPENAI_API_KEY", "test-secret");
        let args = auth_env_args();

        restore_env(saved);

        assert!(args.iter().any(|arg| arg == "OPENAI_API_KEY"));
        assert!(!args.iter().any(|arg| arg.contains("test-secret")));
    }

    #[test]
    fn docker_uses_combined_prompt_artifact() {
        let temp = tempfile::tempdir().expect("tempdir");
        let repo_root = temp.path().join("repo");
        let git_common_dir = temp.path().join("git");
        let worktree = temp.path().join("worktree");
        let input = temp.path().join("input:notes.md");
        let artifacts = test_artifacts(temp.path());
        let user = HostUser {
            uid: "1000".to_string(),
            gid: "1001".to_string(),
        };

        let args = docker_run_args(&DockerRunArgs {
            repo_root: &repo_root,
            git_common_dir: &git_common_dir,
            worktree: &worktree,
            inputs: &[input.clone()],
            artifacts: &artifacts,
            model: "vibe-fixture/dynamic-model",
            stderr_level: "info",
            insecure_tls: false,
            snapshot_ref: "refs/vibe/snapshots/run",
            user: &user,
            pi_agent_dir: None,
            shared_skills_dir: None,
        });

        assert!(args
            .iter()
            .any(|arg| arg == "VIBE_COMBINED_PROMPT_FILE=/artifacts/combined-prompt.txt"));
        assert!(args
            .iter()
            .any(|arg| arg == "VIBE_MODEL=vibe-fixture/dynamic-model"));
        assert!(!args
            .iter()
            .any(|arg| arg == "VIBE_PROMPT_FILE=/artifacts/prompt.txt"));
        assert!(!args
            .iter()
            .any(|arg| arg == "NODE_TLS_REJECT_UNAUTHORIZED=0"));
        assert!(args
            .iter()
            .any(|arg| arg == &format!("VIBE_REPO_ROOT={}", repo_root.display())));
        assert!(args.iter().any(|arg| arg
            == &format!(
                "type=bind,src={},dst={},readonly",
                input.display(),
                input.display()
            )));
    }

    #[test]
    fn docker_sets_insecure_tls_env() {
        let temp = tempfile::tempdir().expect("tempdir");
        let repo_root = temp.path().join("repo");
        let git_common_dir = temp.path().join("git");
        let worktree = temp.path().join("worktree");
        let artifacts = test_artifacts(temp.path());
        let user = HostUser {
            uid: "1000".to_string(),
            gid: "1001".to_string(),
        };

        let args = docker_run_args(&DockerRunArgs {
            repo_root: &repo_root,
            git_common_dir: &git_common_dir,
            worktree: &worktree,
            inputs: &[],
            artifacts: &artifacts,
            model: "openai-codex/gpt-5.4",
            stderr_level: "info",
            insecure_tls: true,
            snapshot_ref: "refs/vibe/snapshots/run",
            user: &user,
            pi_agent_dir: None,
            shared_skills_dir: None,
        });

        assert!(args
            .iter()
            .any(|arg| arg == "NODE_TLS_REJECT_UNAUTHORIZED=0"));
    }

    #[test]
    fn docker_mounts_pi_state_writable() {
        let temp = tempfile::tempdir().expect("tempdir");
        let repo_root = temp.path().join("repo");
        let git_common_dir = temp.path().join("git");
        let worktree = temp.path().join("worktree");
        let pi_agent_dir = temp.path().join(".pi/agent");
        let artifacts = test_artifacts(temp.path());
        let user = HostUser {
            uid: "1000".to_string(),
            gid: "1001".to_string(),
        };

        let args = docker_run_args(&DockerRunArgs {
            repo_root: &repo_root,
            git_common_dir: &git_common_dir,
            worktree: &worktree,
            inputs: &[],
            artifacts: &artifacts,
            model: "openai-codex/gpt-5.4",
            stderr_level: "info",
            insecure_tls: false,
            snapshot_ref: "refs/vibe/snapshots/run",
            user: &user,
            pi_agent_dir: Some(&pi_agent_dir),
            shared_skills_dir: None,
        });

        // This checks Vibe's mount only; Pi owns refresh behavior.
        let expected_mount = format!("{}:/vibe-home/.pi/agent:rw", pi_agent_dir.display());
        assert!(args.iter().any(|arg| arg == &expected_mount));
    }

    #[test]
    fn docker_mounts_shared_skills_read_only() {
        let temp = tempfile::tempdir().expect("tempdir");
        let repo_root = temp.path().join("repo");
        let git_common_dir = temp.path().join("git");
        let worktree = temp.path().join("worktree");
        let shared_skills_dir = temp.path().join(".agents/skills");
        let artifacts = test_artifacts(temp.path());
        let user = HostUser {
            uid: "1000".to_string(),
            gid: "1001".to_string(),
        };

        let args = docker_run_args(&DockerRunArgs {
            repo_root: &repo_root,
            git_common_dir: &git_common_dir,
            worktree: &worktree,
            inputs: &[],
            artifacts: &artifacts,
            model: "openai-codex/gpt-5.4",
            stderr_level: "info",
            insecure_tls: false,
            snapshot_ref: "refs/vibe/snapshots/run",
            user: &user,
            pi_agent_dir: None,
            shared_skills_dir: Some(&shared_skills_dir),
        });
        let expected_mount = format!(
            "type=bind,src={},dst=/vibe-home/.agents/skills,readonly",
            shared_skills_dir.display()
        );

        assert!(args
            .windows(2)
            .any(|pair| pair[0] == "--mount" && pair[1] == expected_mount));
        assert!(!args
            .iter()
            .any(|arg| arg.contains("/vibe-home/.agents/skills:rw")));
    }

    #[test]
    fn pi_state_rejects_auth_directory() {
        let _guard = auth_env_lock().lock().expect("lock auth env");
        let home = tempfile::tempdir().expect("tempdir");
        let auth_path = home.path().join(".pi/agent/auth.json");
        fs::create_dir_all(&auth_path).expect("mkdir fake auth dir");
        let saved = save_auth_env();
        clear_auth_env();

        let result = prepare_provider_auth(home.path().to_str());

        restore_env(saved);
        assert!(result.is_err());
    }

    #[test]
    fn pi_state_rejects_missing_auth() {
        let _guard = auth_env_lock().lock().expect("lock auth env");
        let home = tempfile::tempdir().expect("tempdir");
        let saved = save_auth_env();
        clear_auth_env();

        let result = prepare_provider_auth(home.path().to_str());

        restore_env(saved);
        assert!(result.is_err());
    }

    #[test]
    fn pi_state_rejects_unwritable_directory() {
        let _guard = auth_env_lock().lock().expect("lock auth env");
        let home = tempfile::tempdir().expect("tempdir");
        let auth_dir = home.path().join(".pi/agent");
        fs::create_dir_all(&auth_dir).expect("mkdir auth dir");
        fs::write(auth_dir.join("auth.json"), b"{}").expect("write auth file");

        let mut permissions = fs::metadata(&auth_dir).expect("metadata").permissions();
        permissions.set_readonly(true);
        fs::set_permissions(&auth_dir, permissions).expect("readonly auth dir");
        let saved = save_auth_env();
        clear_auth_env();

        let result = prepare_provider_auth(home.path().to_str());

        let mut permissions = fs::metadata(&auth_dir).expect("metadata").permissions();
        permissions.set_readonly(false);
        fs::set_permissions(&auth_dir, permissions).expect("restore auth dir");
        restore_env(saved);

        assert!(result.is_err());
    }
}
