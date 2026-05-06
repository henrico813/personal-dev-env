use std::path::{Path, PathBuf};

use crate::{
    adapters::{docker, runtime},
    observe::ArtifactPaths,
    worktree::SandboxMounts,
};

pub fn prepare() -> Result<PathBuf, String> {
    docker::require_docker()?;
    docker::require_auth()?;
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
    docker::run_task(
        &mounts.repo_root,
        &mounts.git_common_dir,
        &mounts.worktree,
        artifacts,
        model,
        stderr_level,
        insecure_tls,
    )
}
