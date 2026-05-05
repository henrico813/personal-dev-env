use crate::{
    cli::RunArgs,
    observe,
    prompts,
    result::{RunResult, SetupErrorContext, Status},
    sandbox, snapshot, worktree,
};
use std::{fs, path::Path};

const COMBINED_PROMPT_MISSING_EXIT: i32 = 97;

/// Read the supervisor prompt as UTF-8 so the rendered contract is deterministic.
pub fn read_supervisor_prompt(path: &Path) -> Result<String, String> {
    fs::read_to_string(path).map_err(|e| format!("read prompt file as UTF-8: {e}"))
}

fn setup_error_context(
    session: &worktree::WorktreeSession,
    args: &RunArgs,
    artifacts: &observe::ArtifactPaths,
    pre_run_commit: Option<String>,
) -> SetupErrorContext {
    SetupErrorContext {
        branch: Some(session.branch.clone()),
        worktree: Some(session.worktree.display().to_string()),
        model: Some(args.model.clone()),
        pre_run_commit,
        artifacts_dir: Some(artifacts.dir.display().to_string()),
        events_log_path: Some(artifacts.events_jsonl.display().to_string()),
        stderr_path: Some(artifacts.stderr_log.display().to_string()),
        ..SetupErrorContext::default()
    }
}

/// Execute one Vibe task end-to-end and return the stable JSON result.
pub fn execute(args: RunArgs) -> RunResult {
    let session = match worktree::prepare(&args.key) {
        Ok(session) => session,
        Err(err) => return RunResult::setup_error(err),
    };
    let artifacts = match observe::create_artifacts(session.repo_root(), &session.slug) {
        Ok(paths) => paths,
        Err(err) => return RunResult::setup_error(err),
    };
    let supervisor_prompt = match read_supervisor_prompt(&args.prompt_file) {
        Ok(prompt) => prompt,
        Err(err) => {
            return RunResult::setup_error_with_context(
                err,
                setup_error_context(&session, &args, &artifacts, None),
            );
        }
    };
    if let Err(err) = observe::write_prompt_artifact(&artifacts.prompt_txt, &supervisor_prompt) {
        return RunResult::setup_error_with_context(
            err,
            setup_error_context(&session, &args, &artifacts, None),
        );
    }
    let rendered_prompt = prompts::render_executor_prompt(&supervisor_prompt);
    if let Err(err) = observe::write_rendered_prompt(&artifacts, &rendered_prompt) {
        return RunResult::setup_error_with_context(
            err,
            setup_error_context(&session, &args, &artifacts, None),
        );
    }
    if let Err(err) = worktree::refuse_if_dirty(&session.worktree) {
        return RunResult {
            status: Status::RefusedDirty,
            branch: Some(session.branch),
            worktree: Some(session.worktree.display().to_string()),
            model: Some(args.model),
            pre_run_commit: None,
            commit: None,
            snapshot_commits: Vec::new(),
            artifacts_dir: Some(artifacts.dir.display().to_string()),
            events_log_path: Some(artifacts.events_jsonl.display().to_string()),
            stderr_path: Some(artifacts.stderr_log.display().to_string()),
            error_message: Some(err),
        };
    }

    let pre_run_commit = match worktree::pre_run_commit(&session.worktree) {
        Ok(sha) => sha,
        Err(err) => {
            return RunResult::setup_error_with_context(
                err,
                setup_error_context(&session, &args, &artifacts, None),
            )
        }
    };
    let runtime_root = match sandbox::prepare() {
        Ok(path) => path,
        Err(err) => {
            return RunResult::setup_error_with_context(
                err,
                setup_error_context(&session, &args, &artifacts, Some(pre_run_commit.clone())),
            )
        }
    };
    let mounts = session.sandbox_mounts();
    let agent_exit = match sandbox::run_agent(
        &runtime_root,
        &mounts,
        &artifacts,
        &args.model,
        args.stderr_level.as_str(),
    ) {
        Ok(code) => code,
        Err(err) => {
            return RunResult::setup_error_with_context(
                err,
                setup_error_context(&session, &args, &artifacts, Some(pre_run_commit.clone())),
            )
        }
    };
    if agent_exit == COMBINED_PROMPT_MISSING_EXIT {
        return RunResult::setup_error_with_context(
            "combined prompt artifact unavailable inside sandbox",
            setup_error_context(&session, &args, &artifacts, Some(pre_run_commit.clone())),
        );
    }

    let snapshot_commits = snapshot::read_snapshot_shas(&artifacts.snapshots_jsonl);
    let dirty_after = match worktree::is_dirty(&session.worktree) {
        Ok(dirty) => dirty,
        Err(err) => {
            return RunResult::setup_error_with_context(
                err,
                SetupErrorContext {
                    branch: Some(session.branch),
                    worktree: Some(session.worktree.display().to_string()),
                    model: Some(args.model),
                    pre_run_commit: Some(pre_run_commit),
                    snapshot_commits,
                    artifacts_dir: Some(artifacts.dir.display().to_string()),
                    events_log_path: Some(artifacts.events_jsonl.display().to_string()),
                    stderr_path: Some(artifacts.stderr_log.display().to_string()),
                },
            );
        }
    };
    let mut status = if agent_exit == 0 {
        Status::Noop
    } else {
        Status::AgentFailed
    };
    let mut commit = None;
    let mut error_message = None;

    if dirty_after {
        let message = args
            .commit_message
            .clone()
            .unwrap_or_else(|| format!("vibe: run {}", session.key));
        match worktree::commit_result(&session.worktree, &message, &runtime_root.join("hooks")) {
            Ok(sha) => {
                commit = Some(sha);
                status = if agent_exit == 0 {
                    Status::Completed
                } else {
                    Status::AgentFailed
                };
            }
            Err(err) => {
                status = Status::CommitFailed;
                error_message = Some(err);
            }
        }
    }

    RunResult {
        status,
        branch: Some(session.branch),
        worktree: Some(session.worktree.display().to_string()),
        model: Some(args.model),
        pre_run_commit: Some(pre_run_commit),
        commit,
        snapshot_commits,
        artifacts_dir: Some(artifacts.dir.display().to_string()),
        events_log_path: Some(artifacts.events_jsonl.display().to_string()),
        stderr_path: Some(artifacts.stderr_log.display().to_string()),
        error_message,
    }
}

#[cfg(test)]
mod tests {
    use super::read_supervisor_prompt;
    use std::path::Path;
    use tempfile::tempdir;

    #[test]
    fn rejects_non_utf8_prompt_file() {
        let temp = tempdir().expect("tempdir");
        let path = temp.path().join("prompt.txt");
        std::fs::write(&path, [0xff_u8, 0xfe_u8]).expect("write prompt");

        let err = read_supervisor_prompt(&path).expect_err("invalid UTF-8 should fail");

        assert!(err.starts_with("read prompt file as UTF-8:"));
    }
}
