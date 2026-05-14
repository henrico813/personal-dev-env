mod adapters;
mod app;
mod cli;
mod ledger;
mod observe;
mod prompts;
mod result;
mod sandbox;
mod snapshot;
mod state;
mod worktree;

use std::process;

use crate::{cli::ParsedCommand, result::RunResult};

fn persist_result_json(result: &RunResult) -> Result<(), String> {
    let Some(dir) = result.artifacts_dir.as_deref() else {
        return Ok(());
    };
    let path = std::path::Path::new(dir).join("result.json");
    let json =
        serde_json::to_string_pretty(result).map_err(|e| format!("serialize result: {e}"))?;
    std::fs::write(path, json).map_err(|e| format!("write result.json: {e}"))
}

fn emit_and_exit(result: &RunResult) -> ! {
    let json = serde_json::to_string_pretty(result).expect("serialize result");
    println!("{json}");
    process::exit(result.exit_code());
}

// Emitted result JSON is useful for callers, but ledger-backed status must survive if it fails.
fn persist_emitted_result(result: &mut RunResult) {
    if let Err(err) = persist_result_json(result) {
        let _ = ledger::record_late_persistence_error(result, format!("write result.json: {err}"));
    }
}

fn main() {
    match cli::parse() {
        ParsedCommand::Run(args) => {
            let mut result = app::execute(args);
            persist_emitted_result(&mut result);
            emit_and_exit(&result);
        }
        ParsedCommand::Status(args) => {
            let repo = adapters::git::repo_layout().unwrap_or_else(|err| {
                eprintln!("vibe status requires a target repo checkout: {err}");
                process::exit(2);
            });
            let state = state::latest_for_key(&repo.repo_root, &args.key).unwrap_or_else(|err| {
                eprintln!("{err}");
                process::exit(2);
            });
            let json = serde_json::to_string_pretty(&state).expect("serialize state");
            println!("{json}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::persist_emitted_result;
    use crate::{
        ledger::RunSummary,
        result::{RunResult, Status},
        state::{self, PersistedRunState, RunPhase},
    };
    use tempfile::tempdir;

    fn sample_result(artifacts_dir: &std::path::Path, summary_path: &std::path::Path) -> RunResult {
        RunResult {
            run_id: Some("run-id".to_string()),
            status: Status::Completed,
            branch: Some("vibe/pdev-099b".to_string()),
            worktree: Some("/tmp/worktree".to_string()),
            model: Some("openai-codex/gpt-5.4-mini".to_string()),
            pre_run_commit: Some("abc".to_string()),
            commit: Some("def".to_string()),
            snapshot_commits: vec!["snap".to_string()],
            artifacts_dir: Some(artifacts_dir.display().to_string()),
            events_log_path: Some(artifacts_dir.join("events.jsonl").display().to_string()),
            stderr_path: Some(artifacts_dir.join("agent.stderr.log").display().to_string()),
            run_path: Some(artifacts_dir.join("run.json").display().to_string()),
            summary_path: Some(summary_path.display().to_string()),
            changed_files: vec!["vibe/src/main.rs".to_string()],
            persistence_error: None,
            error_message: None,
        }
    }

    fn sample_state(
        summary_path: &std::path::Path,
        artifacts_dir: &std::path::Path,
    ) -> PersistedRunState {
        PersistedRunState {
            run_id: "run-id".to_string(),
            key: "pdev-099b".to_string(),
            slug: "pdev-099b".to_string(),
            created_at: 1778781975,
            branch: Some("vibe/pdev-099b".to_string()),
            worktree: Some("/tmp/worktree".to_string()),
            model: Some("openai-codex/gpt-5.4-mini".to_string()),
            phase: RunPhase::Finished,
            terminal_status: Some(Status::Completed),
            pre_run_commit: Some("abc".to_string()),
            commit: Some("def".to_string()),
            snapshot_commits: vec!["snap".to_string()],
            artifacts_dir: Some(artifacts_dir.display().to_string()),
            events_log_path: Some(artifacts_dir.join("events.jsonl").display().to_string()),
            stderr_path: Some(artifacts_dir.join("agent.stderr.log").display().to_string()),
            run_path: Some(artifacts_dir.join("run.json").display().to_string()),
            result_path: Some(artifacts_dir.join("result.json").display().to_string()),
            wrapper_log_path: Some(artifacts_dir.join("vibe.log").display().to_string()),
            summary_path: Some(summary_path.display().to_string()),
            changed_files: vec!["vibe/src/main.rs".to_string()],
            error_message: None,
            persistence_error: None,
        }
    }

    #[test]
    fn result_json_failure_keeps_status_and_repairs_durable_error() {
        let temp = tempdir().expect("tempdir");
        let artifacts_dir = temp.path().join("run");
        std::fs::create_dir_all(&artifacts_dir).expect("artifacts dir");
        let summary_path = artifacts_dir.join("summary.json");
        let state_path = artifacts_dir.join("run-state.json");

        let state = sample_state(&summary_path, &artifacts_dir);
        state::write(&state_path, &state).expect("write state");

        let summary = RunSummary {
            run_id: "run-id".to_string(),
            key: "pdev-099b".to_string(),
            slug: "pdev-099b".to_string(),
            created_at: 1778781975,
            status: Status::Completed,
            branch: Some("vibe/pdev-099b".to_string()),
            worktree: Some("/tmp/worktree".to_string()),
            model: Some("openai-codex/gpt-5.4-mini".to_string()),
            pre_run_commit: Some("abc".to_string()),
            commit: Some("def".to_string()),
            snapshot_commits: vec!["snap".to_string()],
            changed_files: vec!["vibe/src/main.rs".to_string()],
            artifacts_dir: artifacts_dir.display().to_string(),
            summary_path: summary_path.display().to_string(),
            result_path: artifacts_dir.join("result.json").display().to_string(),
            events_log_path: artifacts_dir.join("events.jsonl").display().to_string(),
            stderr_path: artifacts_dir.join("agent.stderr.log").display().to_string(),
            error_message: None,
            persistence_error: None,
        };
        std::fs::write(
            &summary_path,
            serde_json::to_string_pretty(&summary).expect("serialize summary"),
        )
        .expect("write summary");

        let invalid_artifacts_dir = temp.path().join("not-a-dir");
        std::fs::write(&invalid_artifacts_dir, "file").expect("write file path");
        let mut result = sample_result(&invalid_artifacts_dir, &summary_path);

        persist_emitted_result(&mut result);

        assert_eq!(result.status, Status::Completed);
        assert!(result
            .persistence_error
            .as_deref()
            .expect("persistence error")
            .starts_with("write result.json:"));

        let repaired_state = state::read(&state_path).expect("read repaired state");
        assert_eq!(repaired_state.terminal_status, Some(Status::Completed));
        assert_eq!(repaired_state.persistence_error, result.persistence_error);
    }
}
