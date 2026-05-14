use crate::{
    cli::RunArgs,
    ledger, observe, prompts,
    result::{RunResult, Status},
    sandbox, snapshot,
    state::{self, PersistedRunState, RunPhase},
    worktree,
};
use std::{fs, fs::OpenOptions, io::Write, path::Path};

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
    persisted: &mut PersistedRunState,
    phase: RunPhase,
    note: &str,
) -> Result<(), String> {
    persisted.phase = phase;
    state::write(&artifacts.state_json, persisted)?;
    append_wrapper_log(&artifacts.vibe_log, note)
}

struct ResultParts {
    pre_run_commit: Option<String>,
    status: Status,
    commit: Option<String>,
    snapshot_commits: Vec<String>,
    changed_files: Vec<String>,
    error_message: Option<String>,
}

impl ResultParts {
    fn failure(
        pre_run_commit: Option<String>,
        status: Status,
        snapshot_commits: Vec<String>,
        error_message: Option<String>,
    ) -> Self {
        Self {
            pre_run_commit,
            status,
            commit: None,
            snapshot_commits,
            changed_files: Vec::new(),
            error_message,
        }
    }
}

fn build_result(
    session: &worktree::WorktreeSession,
    artifacts: &observe::ArtifactPaths,
    model: &str,
    parts: ResultParts,
) -> RunResult {
    RunResult {
        run_id: Some(artifacts.run_id.clone()),
        status: parts.status,
        branch: Some(session.branch.clone()),
        worktree: Some(session.worktree.display().to_string()),
        model: Some(model.to_string()),
        pre_run_commit: parts.pre_run_commit,
        commit: parts.commit,
        snapshot_commits: parts.snapshot_commits,
        artifacts_dir: Some(artifacts.dir.display().to_string()),
        events_log_path: Some(artifacts.events_jsonl.display().to_string()),
        stderr_path: Some(artifacts.stderr_log.display().to_string()),
        summary_path: Some(artifacts.summary_json.display().to_string()),
        changed_files: parts.changed_files,
        persistence_error: None,
        error_message: parts.error_message,
    }
}

fn finish_result(
    artifacts: &observe::ArtifactPaths,
    persisted: &mut PersistedRunState,
    mut result: RunResult,
) -> RunResult {
    if let Err(err) = ledger::persist_terminal_run(artifacts, persisted, &mut result) {
        let _ = ledger::record_late_persistence_error(&mut result, err);
    }
    result
}

fn collect_changed_files(
    worktree: &std::path::Path,
    pre_run_commit: &str,
    commit: Option<&str>,
) -> Result<Vec<String>, String> {
    worktree::changed_files_since(worktree, pre_run_commit, commit)
}

/// Execute one Vibe task end-to-end and return the stable JSON result.
pub fn execute(args: RunArgs) -> RunResult {
    let session = match worktree::prepare(&args.key) {
        Ok(session) => session,
        Err(err) => return RunResult::setup_error(err),
    };
    let run_id = ledger::run_id();
    let created_at = ledger::created_at().unwrap_or(0);
    let artifacts = match observe::create_artifacts(session.repo_root(), &session.slug, &run_id) {
        Ok(paths) => paths,
        Err(err) => return RunResult::setup_error(err),
    };
    let mut persisted = PersistedRunState {
        run_id,
        key: session.key.clone(),
        slug: session.slug.clone(),
        created_at,
        branch: Some(session.branch.clone()),
        worktree: Some(session.worktree.display().to_string()),
        model: Some(args.model.clone()),
        phase: RunPhase::PreparingArtifacts,
        terminal_status: None,
        pre_run_commit: None,
        commit: None,
        snapshot_commits: Vec::new(),
        artifacts_dir: Some(artifacts.dir.display().to_string()),
        events_log_path: Some(artifacts.events_jsonl.display().to_string()),
        stderr_path: Some(artifacts.stderr_log.display().to_string()),
        result_path: Some(artifacts.result_json.display().to_string()),
        wrapper_log_path: Some(artifacts.vibe_log.display().to_string()),
        summary_path: Some(artifacts.summary_json.display().to_string()),
        changed_files: Vec::new(),
        error_message: None,
        persistence_error: None,
    };
    if let Err(err) = state::write(&artifacts.state_json, &persisted) {
        return build_result(
            &session,
            &artifacts,
            &args.model,
            ResultParts::failure(None, Status::WrapperFailed, Vec::new(), Some(err)),
        );
    }
    if let Err(err) = append_wrapper_log(&artifacts.vibe_log, "artifacts prepared") {
        return finish_result(
            &artifacts,
            &mut persisted,
            build_result(
                &session,
                &artifacts,
                &args.model,
                ResultParts::failure(None, Status::WrapperFailed, Vec::new(), Some(err)),
            ),
        );
    }
    if let Err(err) = persist_phase(
        &artifacts,
        &mut persisted,
        RunPhase::CopyingPrompt,
        "copy prompt",
    ) {
        return finish_result(
            &artifacts,
            &mut persisted,
            build_result(
                &session,
                &artifacts,
                &args.model,
                ResultParts::failure(None, Status::WrapperFailed, Vec::new(), Some(err)),
            ),
        );
    }
    let supervisor_prompt = match read_supervisor_prompt(&args.prompt_file) {
        Ok(prompt) => prompt,
        Err(err) => {
            return finish_result(
                &artifacts,
                &mut persisted,
                build_result(
                    &session,
                    &artifacts,
                    &args.model,
                    ResultParts::failure(None, Status::WrapperFailed, Vec::new(), Some(err)),
                ),
            );
        }
    };
    if let Err(err) = observe::write_prompt_artifact(&artifacts.prompt_txt, &supervisor_prompt) {
        return finish_result(
            &artifacts,
            &mut persisted,
            build_result(
                &session,
                &artifacts,
                &args.model,
                ResultParts::failure(None, Status::WrapperFailed, Vec::new(), Some(err)),
            ),
        );
    }
    let rendered_prompt = prompts::render_executor_prompt(&supervisor_prompt);
    if let Err(err) = observe::write_rendered_prompt(&artifacts, &rendered_prompt) {
        return finish_result(
            &artifacts,
            &mut persisted,
            build_result(
                &session,
                &artifacts,
                &args.model,
                ResultParts::failure(None, Status::WrapperFailed, Vec::new(), Some(err)),
            ),
        );
    }
    if let Err(err) = persist_phase(
        &artifacts,
        &mut persisted,
        RunPhase::CheckingDirty,
        "check dirty",
    ) {
        return finish_result(
            &artifacts,
            &mut persisted,
            build_result(
                &session,
                &artifacts,
                &args.model,
                ResultParts::failure(None, Status::WrapperFailed, Vec::new(), Some(err)),
            ),
        );
    }
    if let Err(err) = worktree::refuse_if_dirty(&session.worktree) {
        return finish_result(
            &artifacts,
            &mut persisted,
            build_result(
                &session,
                &artifacts,
                &args.model,
                ResultParts::failure(None, Status::RefusedDirty, Vec::new(), Some(err)),
            ),
        );
    }

    if let Err(err) = persist_phase(
        &artifacts,
        &mut persisted,
        RunPhase::ReadingPreRunCommit,
        "read pre-run commit",
    ) {
        return finish_result(
            &artifacts,
            &mut persisted,
            build_result(
                &session,
                &artifacts,
                &args.model,
                ResultParts::failure(None, Status::WrapperFailed, Vec::new(), Some(err)),
            ),
        );
    }
    let pre_run_commit = match worktree::pre_run_commit(&session.worktree) {
        Ok(sha) => sha,
        Err(err) => {
            return finish_result(
                &artifacts,
                &mut persisted,
                build_result(
                    &session,
                    &artifacts,
                    &args.model,
                    ResultParts::failure(None, Status::WrapperFailed, Vec::new(), Some(err)),
                ),
            )
        }
    };
    persisted.pre_run_commit = Some(pre_run_commit.clone());
    if let Err(err) = persist_phase(
        &artifacts,
        &mut persisted,
        RunPhase::PreparingSandbox,
        "prepare sandbox",
    ) {
        return finish_result(
            &artifacts,
            &mut persisted,
            build_result(
                &session,
                &artifacts,
                &args.model,
                ResultParts::failure(
                    Some(pre_run_commit.clone()),
                    Status::WrapperFailed,
                    Vec::new(),
                    Some(err),
                ),
            ),
        );
    }
    let runtime_root = match sandbox::prepare() {
        Ok(path) => path,
        Err(err) => {
            return finish_result(
                &artifacts,
                &mut persisted,
                build_result(
                    &session,
                    &artifacts,
                    &args.model,
                    ResultParts::failure(
                        Some(pre_run_commit.clone()),
                        Status::WrapperFailed,
                        Vec::new(),
                        Some(err),
                    ),
                ),
            )
        }
    };
    let mounts = session.sandbox_mounts();
    if let Err(err) = persist_phase(
        &artifacts,
        &mut persisted,
        RunPhase::RunningAgent,
        "run agent",
    ) {
        return finish_result(
            &artifacts,
            &mut persisted,
            build_result(
                &session,
                &artifacts,
                &args.model,
                ResultParts::failure(
                    Some(pre_run_commit.clone()),
                    Status::WrapperFailed,
                    Vec::new(),
                    Some(err),
                ),
            ),
        );
    }
    let agent_exit = match sandbox::run_agent(
        &runtime_root,
        &mounts,
        &artifacts,
        &args.model,
        args.stderr_level.as_str(),
        args.insecure_tls,
    ) {
        Ok(code) => code,
        Err(err) => {
            return finish_result(
                &artifacts,
                &mut persisted,
                build_result(
                    &session,
                    &artifacts,
                    &args.model,
                    ResultParts::failure(
                        Some(pre_run_commit.clone()),
                        Status::WrapperFailed,
                        Vec::new(),
                        Some(err),
                    ),
                ),
            )
        }
    };
    if agent_exit == COMBINED_PROMPT_MISSING_EXIT {
        return finish_result(
            &artifacts,
            &mut persisted,
            build_result(
                &session,
                &artifacts,
                &args.model,
                ResultParts::failure(
                    Some(pre_run_commit.clone()),
                    Status::WrapperFailed,
                    Vec::new(),
                    Some("combined prompt artifact unavailable inside sandbox".to_string()),
                ),
            ),
        );
    }

    if let Err(err) = persist_phase(
        &artifacts,
        &mut persisted,
        RunPhase::ReadingSnapshots,
        "read snapshots",
    ) {
        return finish_result(
            &artifacts,
            &mut persisted,
            build_result(
                &session,
                &artifacts,
                &args.model,
                ResultParts::failure(
                    Some(pre_run_commit.clone()),
                    Status::WrapperFailed,
                    Vec::new(),
                    Some(err),
                ),
            ),
        );
    }
    let snapshot_commits = match snapshot::read_snapshot_shas(&artifacts.snapshots_jsonl) {
        Ok(shas) => shas,
        Err(err) => {
            return finish_result(
                &artifacts,
                &mut persisted,
                build_result(
                    &session,
                    &artifacts,
                    &args.model,
                    ResultParts::failure(
                        Some(pre_run_commit.clone()),
                        Status::SnapshotFailed,
                        Vec::new(),
                        Some(err),
                    ),
                ),
            );
        }
    };
    persisted.snapshot_commits = snapshot_commits.clone();
    let dirty_after = match worktree::is_dirty(&session.worktree) {
        Ok(dirty) => dirty,
        Err(err) => {
            return finish_result(
                &artifacts,
                &mut persisted,
                build_result(
                    &session,
                    &artifacts,
                    &args.model,
                    ResultParts::failure(
                        Some(pre_run_commit),
                        Status::WrapperFailed,
                        snapshot_commits,
                        Some(err),
                    ),
                ),
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
        if let Err(err) = persist_phase(
            &artifacts,
            &mut persisted,
            RunPhase::CommittingResult,
            "commit result",
        ) {
            return finish_result(
                &artifacts,
                &mut persisted,
                build_result(
                    &session,
                    &artifacts,
                    &args.model,
                    ResultParts::failure(
                        Some(pre_run_commit.clone()),
                        Status::WrapperFailed,
                        snapshot_commits.clone(),
                        Some(err),
                    ),
                ),
            );
        }
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

    let mut persistence_error = None;
    let changed_files = if dirty_after {
        match commit.as_deref() {
            Some(commit) => {
                match collect_changed_files(&session.worktree, &pre_run_commit, Some(commit)) {
                    Ok(files) => files,
                    Err(err) => {
                        persistence_error = Some(format!("collect changed_files: {err}"));
                        Vec::new()
                    }
                }
            }
            None => match worktree::changed_files(&session.worktree) {
                Ok(files) => files,
                Err(err) => {
                    persistence_error = Some(format!("collect changed_files: {err}"));
                    Vec::new()
                }
            },
        }
    } else {
        Vec::new()
    };

    let mut result = build_result(
        &session,
        &artifacts,
        &args.model,
        ResultParts {
            pre_run_commit: Some(pre_run_commit),
            status,
            commit,
            snapshot_commits,
            changed_files,
            error_message,
        },
    );
    result.persistence_error = persistence_error;
    finish_result(&artifacts, &mut persisted, result)
}

#[cfg(test)]
mod tests {
    use super::read_supervisor_prompt;
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
