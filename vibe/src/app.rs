use crate::{
    adapters::docker,
    cli::RunArgs,
    ledger,
    ledger::{RunPhase, TerminalOutcome},
    observe, prompts,
    result::{RunResult, Status},
    sandbox, snapshot,
    target::RunTarget,
    worktree,
};
use std::{
    fs,
    fs::OpenOptions,
    io::Write,
    path::{Path, PathBuf},
};

const COMBINED_PROMPT_MISSING_EXIT: i32 = 97;

/// Read the supervisor prompt as UTF-8 so the rendered contract is deterministic.
pub fn read_supervisor_prompt(path: &Path) -> Result<String, String> {
    fs::read_to_string(path).map_err(|e| format!("read prompt file as UTF-8: {e}"))
}

fn append_wrapper_log(path: &Path, message: &str) -> Result<(), String> {
    let mut log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| format!("open wrapper log: {e}"))?;
    writeln!(log, "{message}").map_err(|e| format!("write wrapper log: {e}"))
}

fn persist_phase(
    artifacts: &observe::ArtifactPaths,
    phase: RunPhase,
    note: &str,
) -> Result<(), String> {
    ledger::persist_phase(&artifacts.run_json, phase)?;
    append_wrapper_log(&artifacts.vibe_log, note)
}

struct ActiveRun {
    args: RunArgs,
    supervisor_prompt: String,
    prepared_auth: Option<PathBuf>,
    prepared_skills: docker::PreparedSkills,
    asset_root: PathBuf,
    session: worktree::WorktreeSession,
    artifacts: observe::ArtifactPaths,
    created_at: u64,
}

/// Box stage failures so the success path stays stack-sized.
type StageResult<T> = Result<T, Box<TerminalOutcome>>;

fn stage_failure(outcome: TerminalOutcome) -> Box<TerminalOutcome> {
    Box::new(outcome)
}

struct PreRun {
    pre_run_commit: String,
    mounts: worktree::SandboxMounts,
}

struct AgentRun {
    pre_run_commit: String,
    agent_exit: i32,
}

struct SnapshotRun {
    pre_run_commit: String,
    agent_exit: i32,
    snapshot_commits: Vec<String>,
    dirty_after: bool,
}

fn build_result(run: &ActiveRun, outcome: TerminalOutcome) -> RunResult {
    RunResult {
        run_id: Some(run.artifacts.run_id.clone()),
        status: outcome.status,
        branch: Some(run.session.branch.clone()),
        worktree: Some(run.session.worktree.display().to_string()),
        model: Some(run.args.model.clone()),
        pre_run_commit: outcome.pre_run_commit,
        commit: outcome.commit,
        snapshot_commits: outcome.snapshot_commits,
        artifacts_dir: Some(run.artifacts.dir.display().to_string()),
        events_log_path: Some(run.artifacts.events_jsonl.display().to_string()),
        stderr_path: Some(run.artifacts.stderr_log.display().to_string()),
        run_path: Some(run.artifacts.run_json.display().to_string()),
        summary_path: Some(run.artifacts.summary_json.display().to_string()),
        changed_files: outcome.changed_files,
        persistence_error: outcome.persistence_error,
        error_message: outcome.error_message,
    }
}

fn finish_result(run: &ActiveRun, outcome: TerminalOutcome) -> RunResult {
    match ledger::persist_terminal_run(&run.artifacts, &outcome) {
        Ok(result) => result,
        Err(err) => fallback_result(build_result(run, outcome), err),
    }
}

fn fallback_result(mut result: RunResult, persistence_error: String) -> RunResult {
    let _ = ledger::record_late_persistence_error(&mut result, persistence_error);
    result
}

fn collect_changed_files(
    worktree: &std::path::Path,
    pre_run_commit: &str,
    commit: Option<&str>,
) -> Result<Vec<String>, String> {
    worktree::changed_files_since(worktree, pre_run_commit, commit)
}

// Final result metadata should stay truthful even when git inspection fails.
fn finalize_changed_files(
    worktree: &std::path::Path,
    pre_run_commit: &str,
    commit: Option<&str>,
    dirty_after: bool,
) -> (Vec<String>, Option<String>) {
    if !dirty_after {
        return (Vec::new(), None);
    }

    match commit {
        Some(commit) => match collect_changed_files(worktree, pre_run_commit, Some(commit)) {
            Ok(files) => (files, None),
            Err(err) => (Vec::new(), Some(format!("collect changed_files: {err}"))),
        },
        None => match worktree::changed_files(worktree) {
            Ok(files) => (files, None),
            Err(err) => (Vec::new(), Some(format!("collect changed_files: {err}"))),
        },
    }
}

pub fn validate_inputs(inputs: &[PathBuf]) -> Result<(), String> {
    for input in inputs {
        if !input.is_file() {
            return Err(format!(
                "input must be an existing file: {}",
                input.display()
            ));
        }
        if input.to_string_lossy().contains(',') {
            return Err(format!(
                "input path cannot contain commas: {}",
                input.display()
            ));
        }
    }
    Ok(())
}

fn prepare_active_run(target: &RunTarget, args: RunArgs) -> Result<ActiveRun, String> {
    validate_inputs(&args.inputs)?;
    let supervisor_prompt = read_supervisor_prompt(&args.prompt_file)?;
    worktree::validate_base_target(target, args.base.as_deref())?;
    let prepared_auth =
        docker::prepare_provider_auth(std::env::var("HOME").ok().as_deref(), &args.model)?;
    let home = std::env::var_os("HOME");
    let user_skills = docker::prepare_user_skills(home.as_deref())?;
    let asset_root = sandbox::prepare_agent_image()?;
    let session = worktree::prepare(target, args.base.as_deref())?;
    let repository_skills = docker::prepare_repository_skills(&session.worktree)?;
    if repository_skills.is_some() {
        worktree::validate_repository_skills(&session.worktree)?;
    }
    let prepared_skills = docker::PreparedSkills {
        user: user_skills,
        repository: repository_skills,
    };
    let run_id = ledger::run_id();
    let created_at = ledger::created_at().unwrap_or(0);
    let artifacts = observe::create_artifacts(target, &run_id)?;
    Ok(ActiveRun {
        args,
        supervisor_prompt,
        prepared_auth,
        prepared_skills,
        asset_root,
        session,
        artifacts,
        created_at,
    })
}

fn prepare_stage(run: &ActiveRun) -> StageResult<PreRun> {
    let failure = |error| {
        stage_failure(TerminalOutcome::failure(
            None,
            Status::WrapperFailed,
            Vec::new(),
            Some(error),
        ))
    };
    append_wrapper_log(&run.artifacts.vibe_log, "artifacts prepared").map_err(failure)?;
    persist_phase(&run.artifacts, RunPhase::CopyingPrompt, "copy prompt").map_err(failure)?;
    observe::write_prompt_artifact(&run.artifacts.prompt_txt, &run.supervisor_prompt)
        .map_err(failure)?;
    let rendered_prompt = prompts::render_executor_prompt(&run.supervisor_prompt);
    observe::write_rendered_prompt(&run.artifacts, &rendered_prompt).map_err(failure)?;
    persist_phase(&run.artifacts, RunPhase::CheckingDirty, "check dirty").map_err(failure)?;
    worktree::refuse_if_dirty(&run.session.worktree).map_err(|error| {
        stage_failure(TerminalOutcome::failure(
            None,
            Status::RefusedDirty,
            Vec::new(),
            Some(error),
        ))
    })?;
    persist_phase(
        &run.artifacts,
        RunPhase::ReadingPreRunCommit,
        "read pre-run commit",
    )
    .map_err(failure)?;
    let pre_run_commit = worktree::pre_run_commit(&run.session.worktree).map_err(failure)?;
    ledger::persist_pre_run_commit(&run.artifacts.run_json, &pre_run_commit).map_err(failure)?;
    persist_phase(
        &run.artifacts,
        RunPhase::PreparingSandbox,
        "prepare sandbox",
    )
    .map_err(|error| {
        TerminalOutcome::failure(
            Some(pre_run_commit.clone()),
            Status::WrapperFailed,
            Vec::new(),
            Some(error),
        )
    })?;
    if run.prepared_skills.repository.is_some() {
        worktree::validate_repository_skills(&run.session.worktree).map_err(|error| {
            stage_failure(TerminalOutcome::failure(
                Some(pre_run_commit.clone()),
                Status::WrapperFailed,
                Vec::new(),
                Some(error),
            ))
        })?;
    }
    Ok(PreRun {
        pre_run_commit,
        mounts: run.session.sandbox_mounts(&run.args.inputs),
    })
}

fn agent_stage(run: &ActiveRun, pre_run: PreRun) -> StageResult<AgentRun> {
    let failure = |error| {
        stage_failure(TerminalOutcome::failure(
            Some(pre_run.pre_run_commit.clone()),
            Status::WrapperFailed,
            Vec::new(),
            Some(error),
        ))
    };
    persist_phase(&run.artifacts, RunPhase::RunningAgent, "run agent").map_err(failure)?;
    let agent_exit = sandbox::run_agent(
        &pre_run.mounts,
        &run.artifacts,
        &run.args.model,
        run.args.stderr_level.as_str(),
        run.args.insecure_tls,
        run.prepared_auth.as_deref(),
        &run.prepared_skills,
    )
    .map_err(failure)?;
    if agent_exit == COMBINED_PROMPT_MISSING_EXIT {
        return Err(failure(
            "combined prompt artifact unavailable inside sandbox".to_string(),
        ));
    }
    Ok(AgentRun {
        pre_run_commit: pre_run.pre_run_commit,
        agent_exit,
    })
}

fn snapshot_stage(run: &ActiveRun, agent: AgentRun) -> StageResult<SnapshotRun> {
    let failure = |snapshot_commits, error| {
        stage_failure(TerminalOutcome::failure(
            Some(agent.pre_run_commit.clone()),
            Status::WrapperFailed,
            snapshot_commits,
            Some(error),
        ))
    };
    persist_phase(&run.artifacts, RunPhase::ReadingSnapshots, "read snapshots")
        .map_err(|error| failure(Vec::new(), error))?;
    let snapshot_commits =
        snapshot::read_snapshot_shas(&run.artifacts.snapshots_jsonl).map_err(|error| {
            stage_failure(TerminalOutcome::failure(
                Some(agent.pre_run_commit.clone()),
                Status::SnapshotFailed,
                Vec::new(),
                Some(error),
            ))
        })?;
    let dirty_after = worktree::is_dirty(&run.session.worktree)
        .map_err(|error| failure(snapshot_commits.clone(), error))?;
    Ok(SnapshotRun {
        pre_run_commit: agent.pre_run_commit,
        agent_exit: agent.agent_exit,
        snapshot_commits,
        dirty_after,
    })
}

fn finish_stage(run: &ActiveRun, snapshot: SnapshotRun) -> StageResult<TerminalOutcome> {
    let mut status = if snapshot.agent_exit == 0 {
        Status::Noop
    } else {
        Status::AgentFailed
    };
    let mut commit = None;
    let mut error_message = None;
    if snapshot.dirty_after {
        persist_phase(&run.artifacts, RunPhase::CommittingResult, "commit result").map_err(
            |error| {
                stage_failure(TerminalOutcome::failure(
                    Some(snapshot.pre_run_commit.clone()),
                    Status::WrapperFailed,
                    snapshot.snapshot_commits.clone(),
                    Some(error),
                ))
            },
        )?;
        let message = run
            .args
            .commit_message
            .clone()
            .unwrap_or_else(|| format!("vibe: run {}", run.session.key));
        match worktree::commit_result(
            &run.session.worktree,
            &message,
            &run.asset_root.join("hooks"),
        ) {
            Ok(sha) => {
                commit = Some(sha);
                status = if snapshot.agent_exit == 0 {
                    Status::Completed
                } else {
                    Status::AgentFailed
                };
            }
            Err(error) => {
                status = Status::CommitFailed;
                error_message = Some(error);
            }
        }
    }
    let (changed_files, persistence_error) = finalize_changed_files(
        &run.session.worktree,
        &snapshot.pre_run_commit,
        commit.as_deref(),
        snapshot.dirty_after,
    );
    Ok(TerminalOutcome {
        pre_run_commit: Some(snapshot.pre_run_commit),
        status,
        commit,
        snapshot_commits: snapshot.snapshot_commits,
        changed_files,
        error_message,
        persistence_error,
    })
}

fn run_stages(run: &ActiveRun) -> StageResult<TerminalOutcome> {
    let pre_run = prepare_stage(run)?;
    let agent = agent_stage(run, pre_run)?;
    let snapshot = snapshot_stage(run, agent)?;
    finish_stage(run, snapshot)
}

pub fn execute(target: &RunTarget, args: RunArgs) -> RunResult {
    let run = match prepare_active_run(target, args) {
        Ok(run) => run,
        Err(error) => return RunResult::setup_error(error),
    };
    let run_id = run.artifacts.run_id.clone();
    if let Err(error) = ledger::start_run(
        &run.artifacts,
        &run.session.key,
        &run.session.slug,
        &run.session.branch,
        &run.session.worktree,
        &run.args.model,
        run.created_at,
        run_id,
    ) {
        return build_result(
            &run,
            TerminalOutcome::failure(None, Status::WrapperFailed, Vec::new(), Some(error)),
        );
    }
    let outcome = match run_stages(&run) {
        Ok(outcome) => outcome,
        Err(outcome) => *outcome,
    };
    finish_result(&run, outcome)
}
#[cfg(test)]
mod tests {
    use super::{fallback_result, finalize_changed_files, read_supervisor_prompt, validate_inputs};
    use crate::{
        ledger::TerminalOutcome,
        result::{RunResult, Status},
    };
    use tempfile::tempdir;

    #[test]
    fn stage_failures_preserve_statuses() {
        let cases = [
            (Status::WrapperFailed, None, Vec::new()),
            (Status::RefusedDirty, None, Vec::new()),
            (Status::SnapshotFailed, Some("pre"), Vec::new()),
            (
                Status::CommitFailed,
                Some("pre"),
                vec!["snapshot".to_string()],
            ),
        ];

        for (status, pre_run_commit, snapshot_commits) in cases {
            let outcome = TerminalOutcome::failure(
                pre_run_commit.map(str::to_string),
                status.clone(),
                snapshot_commits.clone(),
                Some("stage error".to_string()),
            );

            assert_eq!(outcome.status, status);
            assert_eq!(outcome.pre_run_commit.as_deref(), pre_run_commit);
            assert_eq!(outcome.snapshot_commits, snapshot_commits);
            assert_eq!(outcome.error_message.as_deref(), Some("stage error"));
        }
    }

    #[test]
    fn fallback_merges_existing_persistence_error() {
        let temp = tempdir().expect("tempdir");
        let mut result = RunResult::setup_error("terminal persist failed");
        result.run_path = Some(temp.path().join("missing/run.json").display().to_string());
        result.persistence_error = Some("collect changed_files: x".to_string());

        let result = fallback_result(result, "persist terminal run: missing record".to_string());

        let error = result.persistence_error.expect("persistence error");
        assert!(error.starts_with("collect changed_files: x"));
        assert!(error.contains("persist terminal run: missing record"));
    }

    #[test]
    fn rejects_non_utf8_prompt_file() {
        let temp = tempdir().expect("tempdir");
        let path = temp.path().join("prompt.txt");
        std::fs::write(&path, [0xff_u8, 0xfe_u8]).expect("write prompt");

        let err = read_supervisor_prompt(&path).expect_err("invalid UTF-8 should fail");

        assert!(err.starts_with("read prompt file as UTF-8:"));
    }

    #[test]
    fn rejects_input_with_mount_separator() {
        let temp = tempdir().expect("tempdir");
        let input = temp.path().join("input,notes.txt");
        std::fs::write(&input, "notes").expect("write input");

        let error = validate_inputs(std::slice::from_ref(&input)).expect_err("invalid input path");

        assert_eq!(
            error,
            format!("input path cannot contain commas: {}", input.display())
        );
    }

    #[test]
    fn changed_files_failure_after_commit_becomes_persistence_error() {
        let temp = tempdir().expect("tempdir");
        let missing_repo = temp.path().join("missing");

        let (files, persistence_error) =
            finalize_changed_files(&missing_repo, "abc", Some("def"), true);

        assert!(files.is_empty());
        assert!(persistence_error
            .expect("persistence error")
            .starts_with("collect changed_files:"));
    }

    #[test]
    fn dirty_uncommitted_changed_files_failure_becomes_persistence_error() {
        let temp = tempdir().expect("tempdir");
        let missing_repo = temp.path().join("missing");

        let (files, persistence_error) = finalize_changed_files(&missing_repo, "abc", None, true);

        assert!(files.is_empty());
        assert!(persistence_error
            .expect("persistence error")
            .starts_with("collect changed_files:"));
    }
}
