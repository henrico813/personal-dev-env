use std::path::{Path, PathBuf};

use crate::{
    adapters::{docker, runtime},
    observe::ArtifactPaths,
    worktree::SandboxMounts,
};

pub fn require_run_auth() -> Result<(), String> {
    let home = std::env::var("HOME").ok();
    let config = docker::discovery_config(home.as_deref())?;
    docker::require_run_auth(&config)
}

pub fn prepare_discovery() -> Result<PathBuf, String> {
    docker::require_docker()?;
    let runtime_root = runtime::ensure_runtime_assets()?;
    docker::ensure_image(&runtime_root)?;
    Ok(runtime_root)
}

pub fn run_agent(
    _runtime_root: &Path,
    mounts: &SandboxMounts,
    artifacts: &ArtifactPaths,
    model: &str,
    stderr_level: &str,
    insecure_tls: bool,
) -> Result<i32, String> {
    docker::run_task(mounts, artifacts, model, stderr_level, insecure_tls)
}
