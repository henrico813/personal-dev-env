use crate::{
    cli::RunArgs,
    observe,
    result::{RunResult, SetupErrorContext, Status},
    sandbox, snapshot, worktree,
};

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
    if let Err(err) = observe::copy_prompt(&args.prompt_file, &artifacts.prompt_txt) {
        return RunResult::setup_error_with_context(
            err,
            SetupErrorContext {
                branch: Some(session.branch.clone()),
                worktree: Some(session.worktree.display().to_string()),
                model: Some(args.model.clone()),
                artifacts_dir: Some(artifacts.dir.display().to_string()),
                events_log_path: Some(artifacts.events_jsonl.display().to_string()),
                stderr_path: Some(artifacts.stderr_log.display().to_string()),
                ..SetupErrorContext::default()
            },
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
                SetupErrorContext {
                    branch: Some(session.branch.clone()),
                    worktree: Some(session.worktree.display().to_string()),
                    model: Some(args.model.clone()),
                    artifacts_dir: Some(artifacts.dir.display().to_string()),
                    events_log_path: Some(artifacts.events_jsonl.display().to_string()),
                    stderr_path: Some(artifacts.stderr_log.display().to_string()),
                    ..SetupErrorContext::default()
                },
            )
        }
    };
    let runtime_root = match sandbox::prepare() {
        Ok(path) => path,
        Err(err) => {
            return RunResult::setup_error_with_context(
                err,
                SetupErrorContext {
                    branch: Some(session.branch.clone()),
                    worktree: Some(session.worktree.display().to_string()),
                    model: Some(args.model.clone()),
                    pre_run_commit: Some(pre_run_commit.clone()),
                    artifacts_dir: Some(artifacts.dir.display().to_string()),
                    events_log_path: Some(artifacts.events_jsonl.display().to_string()),
                    stderr_path: Some(artifacts.stderr_log.display().to_string()),
                    ..SetupErrorContext::default()
                },
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
                SetupErrorContext {
                    branch: Some(session.branch.clone()),
                    worktree: Some(session.worktree.display().to_string()),
                    model: Some(args.model.clone()),
                    pre_run_commit: Some(pre_run_commit.clone()),
                    artifacts_dir: Some(artifacts.dir.display().to_string()),
                    events_log_path: Some(artifacts.events_jsonl.display().to_string()),
                    stderr_path: Some(artifacts.stderr_log.display().to_string()),
                    ..SetupErrorContext::default()
                },
            )
        }
    };

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
