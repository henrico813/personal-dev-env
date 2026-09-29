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

const IMAGE: &str = "vibe-pi:0.8.0";
#[cfg(test)]
const AUTH_VARS: &[&str] = &[
    "ANTHROPIC_API_KEY",
    "OPENAI_API_KEY",
    "GEMINI_API_KEY",
    "DEEPSEEK_API_KEY",
    "AZURE_OPENAI_API_KEY",
    "AZURE_OPENAI_BASE_URL",
    "OPENCODE_API_KEY",
    "GOOG_BASE_URL",
    "GOOG_API_KEY",
];
// These provider IDs match Pi's model selectors; unknown IDs cannot use env auth.
const AUTH_GROUPS: &[(&[&str], &[&str])] = &[
    (&["anthropic"], &["ANTHROPIC_API_KEY"]),
    (&["openai", "openai-codex"], &["OPENAI_API_KEY"]),
    (&["google", "gemini"], &["GEMINI_API_KEY"]),
    (&["deepseek"], &["DEEPSEEK_API_KEY"]),
    (
        &["azure-openai"],
        &["AZURE_OPENAI_API_KEY", "AZURE_OPENAI_BASE_URL"],
    ),
    (&["opencode", "opencode-go"], &["OPENCODE_API_KEY"]),
    (&["goog"], &["GOOG_BASE_URL", "GOOG_API_KEY"]),
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

fn required_auth_group(model: &str) -> Option<&'static [&'static str]> {
    let provider = model.split_once('/')?.0;
    AUTH_GROUPS
        .iter()
        .find(|(providers, _)| providers.contains(&provider))
        .map(|(_, keys)| *keys)
}

fn has_provider_env(model: &str) -> bool {
    required_auth_group(model).is_some_and(|keys| keys.iter().all(|key| env_var_is_set(key)))
}

fn auth_env_args(model: &str) -> Vec<String> {
    let Some(keys) =
        required_auth_group(model).filter(|keys| keys.iter().all(|key| env_var_is_set(key)))
    else {
        return Vec::new();
    };
    keys.iter()
        .flat_map(|key| ["-e".to_string(), (*key).to_string()])
        .collect()
}

pub(crate) fn prepare_provider_auth(
    home: Option<&str>,
    model: &str,
) -> Result<Option<PathBuf>, String> {
    if model.starts_with("openai-compatible/") {
        return Err("vibe no longer supports openai-compatible; use goog/<model>".to_string());
    }

    if let Some(model_name) = model.strip_prefix("goog/") {
        if model_name.trim().is_empty() {
            return Err("vibe requires a model after goog/".to_string());
        }
        // Goog requires endpoint credentials and never falls back to host Pi state.
        if has_provider_env(model) {
            return Ok(None);
        }
        return Err("vibe requires both Goog endpoint variables".to_string());
    }

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

    if has_provider_env(model) || pi_agent_dir.is_some() {
        Ok(pi_agent_dir)
    } else {
        Err("vibe requires provider auth via env vars or ~/.pi/agent/auth.json".to_string())
    }
}

#[derive(Debug)]
pub(crate) struct PreparedSkillRoot {
    path: PathBuf,
    device: u64,
    inode: u64,
    source: SkillRootSource,
}

pub(crate) struct PreparedSkills {
    pub(crate) user: Option<PreparedSkillRoot>,
    pub(crate) repository: Option<PreparedSkillRoot>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SkillRootSource {
    User,
    Repository,
}

impl SkillRootSource {
    fn label(self) -> &'static str {
        match self {
            Self::User => "user skills",
            Self::Repository => "repository skills",
        }
    }
}

fn reject_symlink_ancestry(path: &Path, label: &str) -> Result<(), String> {
    for ancestor in path.ancestors() {
        match fs::symlink_metadata(ancestor) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(format!(
                    "{label} path ancestry cannot contain a symlink: {}",
                    ancestor.display()
                ));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                return Err(format!(
                    "read {label} path ancestry {}: {error}",
                    ancestor.display()
                ));
            }
        }
    }
    Ok(())
}

fn docker_mount_path(path: &Path, label: &str) -> Result<(), String> {
    let text = path
        .to_str()
        .ok_or_else(|| format!("{label} must be valid UTF-8 to mount skills"))?;
    if text.contains(',') || text.contains('"') || text.contains('\n') || text.contains('\r') {
        return Err(format!(
            "{label} contains syntax unsafe for Docker mounts: {}",
            path.display()
        ));
    }
    Ok(())
}

fn paths_overlap(left: &Path, right: &Path) -> bool {
    left.starts_with(right) || right.starts_with(left)
}

fn reject_writable_mount_overlap(
    skills: &Path,
    skills_label: &str,
    mount_label: &str,
    writable_mount: &Path,
) -> Result<(), String> {
    let writable_mount = fs::canonicalize(writable_mount)
        .map_err(|error| format!("resolve {mount_label} for skills validation: {error}"))?;
    if paths_overlap(skills, &writable_mount) {
        return Err(format!(
            "{skills_label} path overlaps writable Docker mount {mount_label}: {}",
            skills.display()
        ));
    }
    Ok(())
}

fn reject_user_skills_writable_overlap(
    skills: &Path,
    worktree: &Path,
    git_common_dir: &Path,
    artifacts: &Path,
    pi_agent_dir: Option<&Path>,
) -> Result<(), String> {
    for (label, writable_mount) in [
        ("worktree", worktree),
        ("shared Git directory", git_common_dir),
        ("artifacts", artifacts),
    ] {
        reject_writable_mount_overlap(skills, "user skills", label, writable_mount)?;
    }
    if let Some(pi_agent_dir) = pi_agent_dir {
        reject_writable_mount_overlap(skills, "user skills", "Pi state directory", pi_agent_dir)?;
    }
    Ok(())
}

fn validate_repository_skills_mount(
    skills: &Path,
    worktree: &Path,
    git_common_dir: &Path,
    artifacts: &Path,
    pi_agent_dir: Option<&Path>,
) -> Result<(), String> {
    let expected = fs::canonicalize(worktree.join(".agents/skills"))
        .map_err(|error| format!("resolve repository skills in worktree: {error}"))?;
    if skills != expected {
        return Err(format!(
            "repository skills path moved outside the managed worktree: {}",
            skills.display()
        ));
    }
    for (label, writable_mount) in [
        ("shared Git directory", git_common_dir),
        ("artifacts", artifacts),
    ] {
        reject_writable_mount_overlap(skills, "repository skills", label, writable_mount)?;
    }
    if let Some(pi_agent_dir) = pi_agent_dir {
        reject_writable_mount_overlap(
            skills,
            "repository skills",
            "Pi state directory",
            pi_agent_dir,
        )?;
    }
    Ok(())
}

fn reject_descendant_symlinks(root: &Path, label: &str) -> Result<(), String> {
    let mut directories = vec![root.to_path_buf()];
    while let Some(directory) = directories.pop() {
        let entries = fs::read_dir(&directory)
            .map_err(|error| format!("read {label} {}: {error}", directory.display()))?;
        for entry in entries {
            let entry = entry.map_err(|error| {
                format!("read {label} entry in {}: {error}", directory.display())
            })?;
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path)
                .map_err(|error| format!("read {label} metadata {}: {error}", path.display()))?;
            if metadata.file_type().is_symlink() {
                return Err(format!(
                    "{label} cannot contain symlinks: {}",
                    path.display()
                ));
            }
            if metadata.is_file() && metadata.nlink() > 1 {
                return Err(format!(
                    "{label} cannot contain multiply-linked files: {}",
                    path.display()
                ));
            }
            if metadata.is_dir() {
                directories.push(path);
            }
        }
    }
    Ok(())
}

fn inspect_skill_root_with<F>(
    skills_dir: &Path,
    source: SkillRootSource,
    read_dir: F,
) -> Result<Option<PreparedSkillRoot>, String>
where
    F: FnOnce(&Path) -> std::io::Result<()>,
{
    let label = source.label();
    let metadata = match fs::symlink_metadata(skills_dir) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(format!(
                "read {label} metadata {}: {error}",
                skills_dir.display()
            ))
        }
    };
    reject_symlink_ancestry(skills_dir, label)?;
    match metadata {
        metadata if metadata.file_type().is_symlink() => Err(format!(
            "{label} path cannot be a symlink: {}",
            skills_dir.display()
        )),
        metadata if metadata.is_dir() => {
            let resolved = match fs::canonicalize(skills_dir) {
                Ok(resolved) => resolved,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
                Err(error) => {
                    return Err(format!("resolve {label} {}: {error}", skills_dir.display()))
                }
            };
            let resolved_metadata = match fs::metadata(&resolved) {
                Ok(metadata) => metadata,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
                Err(error) => {
                    return Err(format!(
                        "read {label} metadata {}: {error}",
                        resolved.display()
                    ))
                }
            };
            docker_mount_path(&resolved, label)?;
            if metadata.dev() != resolved_metadata.dev()
                || metadata.ino() != resolved_metadata.ino()
            {
                return Err(format!(
                    "{label} path changed during validation: {}",
                    skills_dir.display()
                ));
            }
            match read_dir(&resolved) {
                Ok(()) => {
                    if source == SkillRootSource::Repository {
                        reject_descendant_symlinks(&resolved, label)?;
                    }
                    Ok(Some(PreparedSkillRoot {
                        path: resolved,
                        device: metadata.dev(),
                        inode: metadata.ino(),
                        source,
                    }))
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
                Err(error) => Err(format!("read {label} {}: {error}", resolved.display())),
            }
        }
        _ => Err(format!(
            "{label} path is not a directory: {}",
            skills_dir.display()
        )),
    }
}

fn prepare_user_skills_with<F>(
    home: Option<&OsStr>,
    read_dir: F,
) -> Result<Option<PreparedSkillRoot>, String>
where
    F: FnOnce(&Path) -> std::io::Result<()>,
{
    let Some(home) = home else {
        return Ok(None);
    };
    let home_path = PathBuf::from(home);
    let skills_dir = home_path.join(".agents/skills");
    let prepared = inspect_skill_root_with(&skills_dir, SkillRootSource::User, read_dir)?;
    if prepared.is_none() {
        return Ok(None);
    }

    let home = home
        .to_str()
        .ok_or_else(|| "HOME must be valid UTF-8 to mount user skills".to_string())?;
    if home.is_empty() || !home_path.is_absolute() {
        return Err("HOME must be an absolute path to mount user skills".to_string());
    }
    docker_mount_path(&home_path, "HOME")?;
    Ok(prepared)
}

pub(crate) fn prepare_user_skills(
    home: Option<&OsStr>,
) -> Result<Option<PreparedSkillRoot>, String> {
    prepare_user_skills_with(home, |path| fs::read_dir(path).map(|_| ()))
}

pub(crate) fn prepare_repository_skills(
    worktree: &Path,
) -> Result<Option<PreparedSkillRoot>, String> {
    inspect_skill_root_with(
        &worktree.join(".agents/skills"),
        SkillRootSource::Repository,
        |path| fs::read_dir(path).map(|_| ()),
    )
}

fn revalidate_skill_root(prepared: &PreparedSkillRoot) -> Result<Option<PathBuf>, String> {
    let Some(current) = inspect_skill_root_with(&prepared.path, prepared.source, |path| {
        fs::read_dir(path).map(|_| ())
    })?
    else {
        return Ok(None);
    };
    if current.device != prepared.device || current.inode != prepared.inode {
        return Err(format!(
            "{} path changed after validation: {}",
            prepared.source.label(),
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
mod tests {
    use super::{
        auth_env_args, docker_run_args, prepare_provider_auth, prepare_repository_skills,
        prepare_user_skills, prepare_user_skills_with, reject_user_skills_writable_overlap,
        revalidate_skill_root, validate_repository_skills_mount, ArtifactPaths, DockerRunArgs,
        HostUser, AUTH_VARS,
    };
    use std::{
        ffi::OsString,
        fs,
        path::Path,
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

    fn test_artifacts(dir: &Path) -> ArtifactPaths {
        let artifacts = dir.join("artifacts");
        ArtifactPaths {
            dir: artifacts.clone(),
            run_id: "run-id".to_string(),
            prompt_txt: artifacts.join("prompt.txt"),
            system_prompt_txt: artifacts.join("system-prompt.txt"),
            combined_prompt_txt: artifacts.join("combined-prompt.txt"),
            system_prompt_versions_txt: artifacts.join("system-prompt-versions.txt"),
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

        assert!(prepare_provider_auth(home.path().to_str(), "openai-codex/gpt-5.4").is_ok());

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

        assert!(prepare_provider_auth(home.path().to_str(), "opencode-go/model").is_ok());
        assert!(auth_env_args("opencode-go/model")
            .iter()
            .any(|arg| arg == "OPENCODE_API_KEY"));

        restore_env(saved);
    }

    #[test]
    fn shared_skills_accept_missing_directory() {
        let home = tempfile::tempdir().expect("tempdir");

        assert!(prepare_user_skills(Some(home.path().as_os_str()))
            .expect("missing skills are optional")
            .is_none());
    }

    #[test]
    fn shared_skills_find_directory() {
        let home = tempfile::tempdir().expect("tempdir");
        let skills_dir = home.path().join(".agents/skills");
        fs::create_dir_all(&skills_dir).expect("mkdir skills");

        let prepared = prepare_user_skills(Some(home.path().as_os_str()))
            .expect("read skills path")
            .expect("skills path");

        assert_eq!(
            prepared.path,
            fs::canonicalize(skills_dir).expect("resolve skills")
        );
    }

    #[test]
    fn repository_skills_accept_missing_directory() {
        let worktree = tempfile::tempdir().expect("tempdir");

        assert!(prepare_repository_skills(worktree.path())
            .expect("missing repository skills are optional")
            .is_none());
    }

    #[test]
    fn repository_skills_find_directory() {
        let worktree = tempfile::tempdir().expect("tempdir");
        let skills_dir = worktree.path().join(".agents/skills");
        fs::create_dir_all(&skills_dir).expect("mkdir skills");

        let prepared = prepare_repository_skills(worktree.path())
            .expect("read repository skills")
            .expect("repository skills path");

        assert_eq!(
            prepared.path,
            fs::canonicalize(skills_dir).expect("resolve skills")
        );
    }

    #[test]
    fn repository_skills_reject_file() {
        let worktree = tempfile::tempdir().expect("tempdir");
        let skills_dir = worktree.path().join(".agents/skills");
        fs::create_dir_all(skills_dir.parent().expect("skills parent")).expect("mkdir parent");
        fs::write(&skills_dir, b"not a directory").expect("write skills file");

        let error = prepare_repository_skills(worktree.path())
            .expect_err("a file cannot be mounted as repository skills");

        assert!(error.contains("repository skills path is not a directory"));
    }

    #[test]
    fn repository_skills_reject_descendant_symlinks() {
        let worktree = tempfile::tempdir().expect("tempdir");
        let skills_dir = worktree.path().join(".agents/skills/reviewed");
        let target = worktree.path().join("README.md");
        fs::create_dir_all(&skills_dir).expect("mkdir skills");
        fs::write(&target, "outside skills\n").expect("write target");
        std::os::unix::fs::symlink(&target, skills_dir.join("SKILL.md"))
            .expect("symlink skill file");

        let error = prepare_repository_skills(worktree.path())
            .expect_err("descendant symlinks must fail setup");

        assert!(error.contains("repository skills cannot contain symlinks"));
    }

    #[test]
    fn repository_skills_reject_hard_links() {
        let worktree = tempfile::tempdir().expect("tempdir");
        let skills_dir = worktree.path().join(".agents/skills/reviewed");
        let skill = skills_dir.join("SKILL.md");
        fs::create_dir_all(&skills_dir).expect("mkdir skills");
        fs::write(&skill, "reviewed\n").expect("write skill");
        fs::hard_link(&skill, worktree.path().join("writable-alias.md"))
            .expect("link skill outside root");

        let error =
            prepare_repository_skills(worktree.path()).expect_err("hard links must fail setup");

        assert!(error.contains("repository skills cannot contain multiply-linked files"));
    }

    #[test]
    fn shared_skills_reject_file() {
        let home = tempfile::tempdir().expect("tempdir");
        let skills_dir = home.path().join(".agents/skills");
        fs::create_dir_all(skills_dir.parent().expect("skills parent")).expect("mkdir parent");
        fs::write(&skills_dir, b"not a directory").expect("write skills file");

        let error = prepare_user_skills(Some(home.path().as_os_str()))
            .expect_err("a file cannot be mounted as the skills directory");

        assert!(error.contains("user skills path is not a directory"));
    }

    #[test]
    fn shared_skills_allow_missing_unsafe_home() {
        let home = tempfile::tempdir().expect("tempdir");
        let missing = home.path().join("missing,home");

        assert!(prepare_user_skills(Some(missing.as_os_str()))
            .expect("missing skills are optional")
            .is_none());
    }

    #[test]
    fn shared_skills_reject_unsafe_home() {
        let parent = tempfile::tempdir().expect("tempdir");
        let home = parent.path().join("home,with-comma");
        fs::create_dir_all(home.join(".agents/skills")).expect("mkdir skills");

        assert!(prepare_user_skills(Some(home.as_os_str())).is_err());
    }

    #[test]
    fn shared_skills_reject_symlink() {
        let home = tempfile::tempdir().expect("tempdir");
        let target = home.path().join("target");
        let skills_dir = home.path().join(".agents/skills");
        fs::create_dir_all(&target).expect("mkdir target");
        fs::create_dir_all(skills_dir.parent().expect("skills parent")).expect("mkdir parent");
        std::os::unix::fs::symlink(target, &skills_dir).expect("symlink skills");

        let error = prepare_user_skills(Some(home.path().as_os_str()))
            .expect_err("symlinked skills must fail setup");

        assert!(error.contains("path ancestry cannot contain a symlink"));
    }

    #[test]
    fn shared_skills_allow_missing_symlinked_home() {
        let parent = tempfile::tempdir().expect("tempdir");
        let real_home = parent.path().join("real-home");
        let home = parent.path().join("home");
        fs::create_dir_all(&real_home).expect("mkdir home");
        std::os::unix::fs::symlink(&real_home, &home).expect("symlink home");

        assert!(prepare_user_skills(Some(home.as_os_str()))
            .expect("missing skills remain optional")
            .is_none());
    }

    #[test]
    fn shared_skills_reject_symlinked_parent() {
        let parent = tempfile::tempdir().expect("tempdir");
        let real_home = parent.path().join("real-home");
        let home = parent.path().join("home");
        fs::create_dir_all(real_home.join(".agents/skills")).expect("mkdir skills");
        std::os::unix::fs::symlink(&real_home, &home).expect("symlink home");

        let error = prepare_user_skills(Some(home.as_os_str()))
            .expect_err("existing skills cannot use symlinked home");

        assert!(error.contains("path ancestry cannot contain a symlink"));
    }

    #[test]
    fn shared_skills_reject_newline_path() {
        let parent = tempfile::tempdir().expect("tempdir");
        let home = parent.path().join("home\nnewline");
        fs::create_dir_all(home.join(".agents/skills")).expect("mkdir skills");

        let error = prepare_user_skills(Some(home.as_os_str()))
            .expect_err("newline mount paths must fail setup");

        assert!(error.contains("syntax unsafe for Docker mounts"));
    }

    #[test]
    fn shared_skills_reject_quote_path() {
        let parent = tempfile::tempdir().expect("tempdir");
        let home = parent.path().join("home\"quoted");
        fs::create_dir_all(home.join(".agents/skills")).expect("mkdir skills");

        let error = prepare_user_skills(Some(home.as_os_str()))
            .expect_err("quoted mount paths must fail setup");

        assert!(error.contains("syntax unsafe for Docker mounts"));
    }

    #[test]
    fn shared_skills_reject_writable_overlap() {
        let parent = tempfile::tempdir().expect("tempdir");
        let worktree = parent.path().join("worktree");
        let shared_skills = worktree.join(".agents/skills");
        let git = parent.path().join("git");
        let artifacts = parent.path().join("artifacts");
        fs::create_dir_all(&shared_skills).expect("mkdir skills");
        fs::create_dir_all(&git).expect("mkdir git");
        fs::create_dir_all(&artifacts).expect("mkdir artifacts");

        let error =
            reject_user_skills_writable_overlap(&shared_skills, &worktree, &git, &artifacts, None)
                .expect_err("shared skills cannot overlap writable mounts");

        assert!(error.contains("overlaps writable Docker mount worktree"));
    }

    #[test]
    fn pi_state_rejects_shared_skills_overlap() {
        let parent = tempfile::tempdir().expect("tempdir");
        let shared_skills = parent.path().join("skills");
        let pi_agent_dir = parent.path().join("pi");
        let worktree = parent.path().join("worktree");
        let git = parent.path().join("git");
        let artifacts = parent.path().join("artifacts");
        fs::create_dir_all(&shared_skills).expect("mkdir skills");
        fs::create_dir_all(&worktree).expect("mkdir worktree");
        fs::create_dir_all(&git).expect("mkdir git");
        fs::create_dir_all(&artifacts).expect("mkdir artifacts");
        std::os::unix::fs::symlink(&shared_skills, &pi_agent_dir).expect("symlink pi state");

        let error = reject_user_skills_writable_overlap(
            &shared_skills,
            &worktree,
            &git,
            &artifacts,
            Some(&pi_agent_dir),
        )
        .expect_err("Pi state cannot overlap shared skills");

        assert!(error.contains("Pi state directory"));
    }

    #[test]
    fn repository_skills_allow_exact_subtree() {
        let parent = tempfile::tempdir().expect("tempdir");
        let worktree = parent.path().join("worktree");
        let skills = worktree.join(".agents/skills");
        let git = parent.path().join("git");
        let artifacts = parent.path().join("artifacts");
        fs::create_dir_all(&skills).expect("mkdir skills");
        fs::create_dir_all(&git).expect("mkdir git");
        fs::create_dir_all(&artifacts).expect("mkdir artifacts");
        let skills = fs::canonicalize(skills).expect("resolve skills");

        validate_repository_skills_mount(&skills, &worktree, &git, &artifacts, None)
            .expect("exact repository subtree is protected by nested mount");
    }

    #[test]
    fn repository_skills_reject_wrong_subtree() {
        let parent = tempfile::tempdir().expect("tempdir");
        let worktree = parent.path().join("worktree");
        let expected = worktree.join(".agents/skills");
        let other = worktree.join("other-skills");
        let git = parent.path().join("git");
        let artifacts = parent.path().join("artifacts");
        fs::create_dir_all(&expected).expect("mkdir expected skills");
        fs::create_dir_all(&other).expect("mkdir other skills");
        fs::create_dir_all(&git).expect("mkdir git");
        fs::create_dir_all(&artifacts).expect("mkdir artifacts");
        let other = fs::canonicalize(other).expect("resolve other skills");

        let error = validate_repository_skills_mount(&other, &worktree, &git, &artifacts, None)
            .expect_err("other worktree subtrees stay writable");

        assert!(error.contains("moved outside the managed worktree"));
    }

    #[test]
    fn repository_skills_reject_git_overlap() {
        let parent = tempfile::tempdir().expect("tempdir");
        let worktree = parent.path().join("worktree");
        let skills = worktree.join(".agents/skills");
        let artifacts = parent.path().join("artifacts");
        fs::create_dir_all(&skills).expect("mkdir skills");
        fs::create_dir_all(&artifacts).expect("mkdir artifacts");
        let skills = fs::canonicalize(skills).expect("resolve skills");

        let error = validate_repository_skills_mount(&skills, &worktree, &skills, &artifacts, None)
            .expect_err("Git overlap must stay writable");

        assert!(error.contains("shared Git directory"));
    }

    #[test]
    fn shared_skills_omit_disappeared_directory() {
        let home = tempfile::tempdir().expect("tempdir");
        let skills_dir = home.path().join(".agents/skills");
        fs::create_dir_all(&skills_dir).expect("mkdir skills");
        let prepared = prepare_user_skills(Some(home.path().as_os_str()))
            .expect("read skills path")
            .expect("skills path");
        fs::remove_dir(&skills_dir).expect("remove skills");

        assert!(revalidate_skill_root(&prepared)
            .expect("disappeared skills are optional")
            .is_none());
    }

    #[test]
    fn shared_skills_reject_replacement() {
        let home = tempfile::tempdir().expect("tempdir");
        let skills_dir = home.path().join(".agents/skills");
        let original = home.path().join("original-skills");
        fs::create_dir_all(&skills_dir).expect("mkdir skills");
        let prepared = prepare_user_skills(Some(home.path().as_os_str()))
            .expect("read skills path")
            .expect("skills path");
        fs::rename(&skills_dir, &original).expect("move original skills");
        fs::create_dir(&skills_dir).expect("replace skills");

        let error =
            revalidate_skill_root(&prepared).expect_err("replacement must fail before launch");

        assert!(error.contains("changed after validation"));
    }

    #[test]
    fn repository_skills_reject_replacement() {
        let worktree = tempfile::tempdir().expect("tempdir");
        let skills_dir = worktree.path().join(".agents/skills");
        let original = worktree.path().join("original-skills");
        fs::create_dir_all(&skills_dir).expect("mkdir skills");
        let prepared = prepare_repository_skills(worktree.path())
            .expect("read repository skills")
            .expect("repository skills path");
        fs::rename(&skills_dir, &original).expect("move original skills");
        fs::create_dir(&skills_dir).expect("replace skills");

        let error =
            revalidate_skill_root(&prepared).expect_err("replacement must fail before launch");

        assert!(error.contains("repository skills path changed after validation"));
    }

    #[test]
    fn shared_skills_reject_unreadable_directory() {
        let home = tempfile::tempdir().expect("tempdir");
        let skills_dir = home.path().join(".agents/skills");
        fs::create_dir_all(&skills_dir).expect("mkdir skills");

        let error = prepare_user_skills_with(Some(home.path().as_os_str()), |_| {
            Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "permission denied",
            ))
        })
        .expect_err("unreadable skills must fail setup");

        assert!(error.contains("read user skills"));
    }

    #[test]
    fn goog_auth_isolates_pi_state() {
        let _guard = auth_env_lock().lock().expect("lock auth env");
        let home = tempfile::tempdir().expect("tempdir");
        let auth_dir = home.path().join(".pi/agent");
        fs::create_dir_all(&auth_dir).expect("mkdir auth dir");
        fs::write(auth_dir.join("auth.json"), b"{}").expect("write auth file");
        let saved = save_auth_env();

        std::env::set_var("HOME", home.path());
        clear_auth_env();
        std::env::set_var("GOOG_BASE_URL", "https://example.invalid");
        std::env::set_var("GOOG_API_KEY", "goog");

        let pi_agent_dir =
            prepare_provider_auth(home.path().to_str(), "goog/qwen3.8").expect("Goog auth");
        let auth_args = auth_env_args("goog/qwen3.8");
        let temp = tempfile::tempdir().expect("tempdir");
        let artifacts = test_artifacts(temp.path());
        let user = HostUser {
            uid: "1000".to_string(),
            gid: "1001".to_string(),
        };
        let docker_args = docker_run_args(&DockerRunArgs {
            repo_root: temp.path(),
            git_common_dir: temp.path(),
            worktree: temp.path(),
            inputs: &[],
            artifacts: &artifacts,
            model: "goog/qwen3.8",
            stderr_level: "info",
            insecure_tls: false,
            snapshot_ref: "refs/vibe/snapshots/run",
            user: &user,
            pi_agent_dir: pi_agent_dir.as_deref(),
            user_skills_dir: None,
            repository_skills_dir: None,
        });

        restore_env(saved);
        assert_eq!(pi_agent_dir, None);
        assert_eq!(auth_args, ["-e", "GOOG_BASE_URL", "-e", "GOOG_API_KEY"]);
        assert!(!docker_args
            .iter()
            .any(|arg| arg.contains("/vibe-home/.pi/agent")));
    }

    #[test]
    fn docker_protects_run_record_mount_order() {
        let temp = tempfile::tempdir().expect("tempdir");
        let artifacts = test_artifacts(temp.path());
        let user = HostUser {
            uid: "1000".to_string(),
            gid: "1001".to_string(),
        };

        let args = docker_run_args(&DockerRunArgs {
            repo_root: temp.path(),
            git_common_dir: temp.path(),
            worktree: temp.path(),
            inputs: &[],
            artifacts: &artifacts,
            model: "openai-codex/gpt-5.4",
            stderr_level: "info",
            insecure_tls: false,
            snapshot_ref: "refs/vibe/snapshots/run",
            user: &user,
            pi_agent_dir: None,
            user_skills_dir: None,
            repository_skills_dir: None,
        });

        let writable_mount = format!("{}:/artifacts", artifacts.dir.display());
        let record_mount = format!("{}:/artifacts/run.json:ro", artifacts.run_json.display());
        let writable_index = args
            .windows(2)
            .position(|pair| pair[0] == "-v" && pair[1] == writable_mount)
            .expect("writable artifacts mount");
        let record_index = args
            .windows(2)
            .position(|pair| pair[0] == "-v" && pair[1] == record_mount)
            .expect("read-only run record mount");

        assert!(writable_index < record_index);
        assert!(!args.iter().any(|arg| {
            arg.ends_with(":/artifacts:ro") || arg.ends_with(":/artifacts:readonly")
        }));
    }

    #[test]
    fn goog_auth_rejects_incomplete_environment() {
        let _guard = auth_env_lock().lock().expect("lock auth env");
        let home = tempfile::tempdir().expect("tempdir");
        let auth_dir = home.path().join(".pi/agent");
        fs::create_dir_all(&auth_dir).expect("mkdir auth dir");
        fs::write(auth_dir.join("auth.json"), b"{}").expect("write auth file");
        let saved = save_auth_env();

        let tests = [
            ("missing key", Some("https://example.invalid"), None),
            ("blank key", Some("https://example.invalid"), Some(" ")),
            ("missing base URL", None, Some("goog")),
            ("blank base URL", Some(" "), Some("goog")),
        ];
        for (name, base_url, api_key) in tests {
            std::env::set_var("HOME", home.path());
            clear_auth_env();
            if let Some(base_url) = base_url {
                std::env::set_var("GOOG_BASE_URL", base_url);
            }
            if let Some(api_key) = api_key {
                std::env::set_var("GOOG_API_KEY", api_key);
            }

            let result = prepare_provider_auth(home.path().to_str(), "goog/qwen3.8");

            assert_eq!(
                result.expect_err(name),
                "vibe requires both Goog endpoint variables",
                "{name}"
            );
        }

        restore_env(saved);
    }

    #[test]
    fn goog_auth_rejects_empty_model() {
        let _guard = auth_env_lock().lock().expect("lock auth env");
        let saved = save_auth_env();
        clear_auth_env();
        std::env::set_var("GOOG_BASE_URL", "https://example.invalid");
        std::env::set_var("GOOG_API_KEY", "goog");

        let result = prepare_provider_auth(None, "goog/");

        restore_env(saved);
        assert_eq!(
            result.expect_err("empty model must fail"),
            "vibe requires a model after goog/"
        );
    }

    #[test]
    fn legacy_compatible_selector_is_rejected() {
        let _guard = auth_env_lock().lock().expect("lock auth env");
        let home = tempfile::tempdir().expect("tempdir");
        let saved = save_auth_env();
        clear_auth_env();

        let result = prepare_provider_auth(home.path().to_str(), "openai-compatible/model");

        restore_env(saved);
        assert_eq!(
            result.expect_err("legacy selector must fail"),
            "vibe no longer supports openai-compatible; use goog/<model>"
        );
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

        assert!(prepare_provider_auth(home.path().to_str(), "openai-codex/gpt-5.4").is_ok());

        restore_env(saved);
    }

    #[test]
    fn provider_auth_rejects_missing_credentials() {
        let _guard = auth_env_lock().lock().expect("lock auth env");
        let home = tempfile::tempdir().expect("tempdir");
        let saved = save_auth_env();

        std::env::set_var("HOME", home.path());
        clear_auth_env();

        assert!(prepare_provider_auth(home.path().to_str(), "openai-codex/gpt-5.4").is_err());

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

        let result = prepare_provider_auth(home.path().to_str(), "azure-openai/gpt-5.4");

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

        let result = prepare_provider_auth(home.path().to_str(), "azure-openai/gpt-5.4");

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

        let result = prepare_provider_auth(home.path().to_str(), "azure-openai/gpt-5.4");

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
        let args = auth_env_args("openai-codex/gpt-5.4");

        restore_env(saved);

        assert!(args.iter().any(|arg| arg == "OPENAI_API_KEY"));
        assert!(!args.iter().any(|arg| arg.contains("test-secret")));
    }

    #[test]
    fn forwards_openai_credentials_only() {
        let _guard = auth_env_lock().lock().expect("lock auth env");
        let saved = save_auth_env();
        clear_auth_env();
        for (key, value) in [
            ("ANTHROPIC_API_KEY", "anthropic"),
            ("OPENAI_API_KEY", "openai"),
            ("GEMINI_API_KEY", "gemini"),
            ("DEEPSEEK_API_KEY", "deepseek"),
            ("AZURE_OPENAI_API_KEY", "azure"),
            ("AZURE_OPENAI_BASE_URL", "https://example.invalid"),
            ("OPENCODE_API_KEY", "opencode"),
        ] {
            std::env::set_var(key, value);
        }

        let args = auth_env_args("openai-codex/model");

        restore_env(saved);
        assert_eq!(args, ["-e", "OPENAI_API_KEY"]);
    }

    #[test]
    fn forwards_goog_credentials_only() {
        let _guard = auth_env_lock().lock().expect("lock auth env");
        let saved = save_auth_env();
        clear_auth_env();
        for (key, value) in [
            ("GOOG_BASE_URL", "https://example.invalid"),
            ("GOOG_API_KEY", "goog"),
            ("OPENAI_API_KEY", "openai"),
        ] {
            std::env::set_var(key, value);
        }

        let args = auth_env_args("goog/qwen3.8");

        restore_env(saved);
        assert_eq!(args, ["-e", "GOOG_BASE_URL", "-e", "GOOG_API_KEY"]);
    }

    #[test]
    fn forwards_opencode_credentials_only() {
        let _guard = auth_env_lock().lock().expect("lock auth env");
        let saved = save_auth_env();
        clear_auth_env();
        for (key, value) in [
            ("ANTHROPIC_API_KEY", "anthropic"),
            ("OPENAI_API_KEY", "openai"),
            ("GEMINI_API_KEY", "gemini"),
            ("DEEPSEEK_API_KEY", "deepseek"),
            ("AZURE_OPENAI_API_KEY", "azure"),
            ("AZURE_OPENAI_BASE_URL", "https://example.invalid"),
            ("OPENCODE_API_KEY", "opencode"),
        ] {
            std::env::set_var(key, value);
        }

        let args = auth_env_args("opencode-go/model");

        restore_env(saved);
        assert_eq!(args, ["-e", "OPENCODE_API_KEY"]);
    }

    #[test]
    fn rejects_unknown_provider_environment_auth() {
        let _guard = auth_env_lock().lock().expect("lock auth env");
        let home = tempfile::tempdir().expect("tempdir");
        let saved = save_auth_env();
        clear_auth_env();
        std::env::set_var("OPENAI_API_KEY", "openai");

        let result = prepare_provider_auth(home.path().to_str(), "unknown/model");

        restore_env(saved);
        assert_eq!(
            result.expect_err("unknown providers must fail closed"),
            ERROR_MESSAGE
        );
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
            inputs: std::slice::from_ref(&input),
            artifacts: &artifacts,
            model: "vibe-fixture/dynamic-model",
            stderr_level: "info",
            insecure_tls: false,
            snapshot_ref: "refs/vibe/snapshots/run",
            user: &user,
            pi_agent_dir: None,
            user_skills_dir: None,
            repository_skills_dir: None,
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
            user_skills_dir: None,
            repository_skills_dir: None,
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
            user_skills_dir: None,
            repository_skills_dir: None,
        });

        // This checks Vibe's mount only; Pi owns refresh behavior.
        let expected_mount = format!("{}:/vibe-home/.pi/agent:rw", pi_agent_dir.display());
        assert!(args.iter().any(|arg| arg == &expected_mount));
    }

    #[test]
    fn docker_mounts_both_skill_roots_read_only() {
        let temp = tempfile::tempdir().expect("tempdir");
        let repo_root = temp.path().join("repo");
        let git_common_dir = temp.path().join("git");
        let worktree = temp.path().join("worktree");
        let user_skills_dir = temp.path().join("home/.agents/skills");
        let repository_skills_dir = worktree.join(".agents/skills");
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
            user_skills_dir: Some(&user_skills_dir),
            repository_skills_dir: Some(&repository_skills_dir),
        });
        let expected_user_mount = format!(
            "type=bind,src={},dst=/vibe-home/.agents/skills,readonly",
            user_skills_dir.display()
        );
        let expected_repository_mount = format!(
            "type=bind,src={},dst={},readonly",
            repository_skills_dir.display(),
            repository_skills_dir.display()
        );

        assert!(args
            .windows(2)
            .any(|pair| pair[0] == "--mount" && pair[1] == expected_user_mount));
        assert!(args
            .windows(2)
            .any(|pair| pair[0] == "--mount" && pair[1] == expected_repository_mount));
        let worktree_mount = format!("{}:{}", worktree.display(), worktree.display());
        let worktree_position = args
            .iter()
            .position(|arg| arg == &worktree_mount)
            .expect("worktree mount");
        let repository_position = args
            .iter()
            .position(|arg| arg == &expected_repository_mount)
            .expect("repository skills mount");
        assert!(repository_position > worktree_position);
        assert!(args.iter().any(|arg| {
            arg == &format!("VIBE_REPO_SKILLS_DIR={}", repository_skills_dir.display())
        }));
        assert!(!args
            .iter()
            .any(|arg| arg.contains("/vibe-home/.agents/skills:rw")));
    }

    #[test]
    fn docker_omits_missing_skill_roots() {
        let temp = tempfile::tempdir().expect("tempdir");
        let artifacts = test_artifacts(temp.path());
        let user = HostUser {
            uid: "1000".to_string(),
            gid: "1001".to_string(),
        };

        let args = docker_run_args(&DockerRunArgs {
            repo_root: temp.path(),
            git_common_dir: temp.path(),
            worktree: temp.path(),
            inputs: &[],
            artifacts: &artifacts,
            model: "openai-codex/gpt-5.4",
            stderr_level: "info",
            insecure_tls: false,
            snapshot_ref: "refs/vibe/snapshots/run",
            user: &user,
            pi_agent_dir: None,
            user_skills_dir: None,
            repository_skills_dir: None,
        });

        assert!(!args
            .iter()
            .any(|arg| arg.contains("/vibe-home/.agents/skills")));
        assert!(!args
            .iter()
            .any(|arg| arg.starts_with("VIBE_REPO_SKILLS_DIR=")));
    }

    #[test]
    fn docker_mounts_user_skills_alone() {
        let temp = tempfile::tempdir().expect("tempdir");
        let user_skills_dir = temp.path().join("home/.agents/skills");
        let artifacts = test_artifacts(temp.path());
        let user = HostUser {
            uid: "1000".to_string(),
            gid: "1001".to_string(),
        };

        let args = docker_run_args(&DockerRunArgs {
            repo_root: temp.path(),
            git_common_dir: temp.path(),
            worktree: temp.path(),
            inputs: &[],
            artifacts: &artifacts,
            model: "openai-codex/gpt-5.4",
            stderr_level: "info",
            insecure_tls: false,
            snapshot_ref: "refs/vibe/snapshots/run",
            user: &user,
            pi_agent_dir: None,
            user_skills_dir: Some(&user_skills_dir),
            repository_skills_dir: None,
        });

        assert!(args.iter().any(|arg| {
            arg == &format!(
                "type=bind,src={},dst=/vibe-home/.agents/skills,readonly",
                user_skills_dir.display()
            )
        }));
        assert!(!args
            .iter()
            .any(|arg| arg.starts_with("VIBE_REPO_SKILLS_DIR=")));
    }

    #[test]
    fn docker_mounts_repository_skills_alone() {
        let temp = tempfile::tempdir().expect("tempdir");
        let repository_skills_dir = temp.path().join("worktree/.agents/skills");
        let artifacts = test_artifacts(temp.path());
        let user = HostUser {
            uid: "1000".to_string(),
            gid: "1001".to_string(),
        };

        let args = docker_run_args(&DockerRunArgs {
            repo_root: temp.path(),
            git_common_dir: temp.path(),
            worktree: temp.path(),
            inputs: &[],
            artifacts: &artifacts,
            model: "openai-codex/gpt-5.4",
            stderr_level: "info",
            insecure_tls: false,
            snapshot_ref: "refs/vibe/snapshots/run",
            user: &user,
            pi_agent_dir: None,
            user_skills_dir: None,
            repository_skills_dir: Some(&repository_skills_dir),
        });

        assert!(args.iter().any(|arg| {
            arg == &format!(
                "type=bind,src={},dst={},readonly",
                repository_skills_dir.display(),
                repository_skills_dir.display()
            )
        }));
        assert!(!args
            .iter()
            .any(|arg| arg.contains("dst=/vibe-home/.agents/skills")));
    }

    #[test]
    fn pi_state_rejects_auth_directory() {
        let _guard = auth_env_lock().lock().expect("lock auth env");
        let home = tempfile::tempdir().expect("tempdir");
        let auth_path = home.path().join(".pi/agent/auth.json");
        fs::create_dir_all(&auth_path).expect("mkdir fake auth dir");
        let saved = save_auth_env();
        clear_auth_env();

        let result = prepare_provider_auth(home.path().to_str(), "openai-codex/gpt-5.4");

        restore_env(saved);
        assert!(result.is_err());
    }

    #[test]
    fn pi_state_rejects_missing_auth() {
        let _guard = auth_env_lock().lock().expect("lock auth env");
        let home = tempfile::tempdir().expect("tempdir");
        let saved = save_auth_env();
        clear_auth_env();

        let result = prepare_provider_auth(home.path().to_str(), "openai-codex/gpt-5.4");

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
        let original_mode = std::os::unix::fs::PermissionsExt::mode(&permissions);
        permissions.set_readonly(true);
        fs::set_permissions(&auth_dir, permissions).expect("readonly auth dir");
        let saved = save_auth_env();
        clear_auth_env();

        let result = prepare_provider_auth(home.path().to_str(), "openai-codex/gpt-5.4");

        let mut permissions = fs::metadata(&auth_dir).expect("metadata").permissions();
        std::os::unix::fs::PermissionsExt::set_mode(&mut permissions, original_mode);
        fs::set_permissions(&auth_dir, permissions).expect("restore auth dir");
        restore_env(saved);

        assert!(result.is_err());
    }
}
