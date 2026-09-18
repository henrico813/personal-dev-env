use std::path::{Path, PathBuf};

use crate::{
    adapters::{docker, runtime},
    observe::ArtifactPaths,
    worktree::SandboxMounts,
};

pub fn prepare_agent_image() -> Result<PathBuf, String> {
    docker::require_docker()?;
    let runtime_root = runtime::ensure_runtime_assets()?;
    docker::ensure_image(&runtime_root)?;
    Ok(runtime_root)
}

pub fn run_agent(
    mounts: &SandboxMounts,
    artifacts: &ArtifactPaths,
    model: &str,
    stderr_level: &str,
    insecure_tls: bool,
    pi_agent_dir: Option<&Path>,
    shared_skills_dir: Option<&Path>,
) -> Result<i32, String> {
    docker::run_task(
        mounts,
        artifacts,
        model,
        stderr_level,
        insecure_tls,
        pi_agent_dir,
        shared_skills_dir,
    )
}
