use std::path::{Path, PathBuf};

use crate::{
    adapters::{docker, runtime},
    observe::ArtifactPaths,
    worktree::WorktreeSession,
};

pub fn prepare() -> Result<PathBuf, String> {
    docker::require_docker()?;
    let runtime_root = runtime::ensure_runtime_assets()?;
    docker::ensure_image(&runtime_root)?;
    Ok(runtime_root)
}

pub fn run_agent(
    runtime_root: &Path,
    session: &WorktreeSession,
    artifacts: &ArtifactPaths,
    model: &str,
) -> Result<i32, String> {
    docker::run_task(
        runtime_root,
        &session.canonical_repo_root,
        &session.git_common_dir,
        &session.worktree,
        artifacts,
        model,
    )
}
