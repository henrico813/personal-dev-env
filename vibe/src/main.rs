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
    let Some(run_path) = result.run_path.as_deref() else {
        return Ok(());
    };
    let Some(dir) = result.artifacts_dir.as_deref() else {
        return Ok(());
    };
    let path = std::path::Path::new(dir).join("result.json");
    ledger::persist_result_from_run(std::path::Path::new(run_path), &path)
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
            let json = if args.long {
                let record = state::latest_record_json_for_key(&repo.repo_root, &args.key)
                    .unwrap_or_else(|err| {
                        eprintln!("{err}");
                        process::exit(2);
                    });
                serde_json::to_string_pretty(&record).expect("serialize record")
            } else {
                let summary = state::latest_summary_for_key(&repo.repo_root, &args.key)
                    .unwrap_or_else(|err| {
                        eprintln!("{err}");
                        process::exit(2);
                    });
                serde_json::to_string_pretty(&summary).expect("serialize summary")
            };
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

    fn write_sample_run_record(summary_path: &std::path::Path, artifacts_dir: &std::path::Path) {
        let record = serde_json::json!({
            "run_id": "run-id",
            "key": "pdev-099b",
            "slug": "pdev-099b",
            "created_at": 1778781975_u64,
            "phase": "finished",
            "terminal_status": "completed",
            "branch": "vibe/pdev-099b",
            "worktree": "/tmp/worktree",
            "model": "openai-codex/gpt-5.4-mini",
            "pre_run_commit": "abc",
            "commit": "def",
            "snapshot_commits": ["snap"],
            "changed_files": ["vibe/src/main.rs"],
            "artifacts_dir": artifacts_dir.display().to_string(),
            "run_path": artifacts_dir.join("run.json").display().to_string(),
            "summary_path": summary_path.display().to_string(),
            "result_path": artifacts_dir.join("result.json").display().to_string(),
            "events_log_path": artifacts_dir.join("events.jsonl").display().to_string(),
            "stderr_path": artifacts_dir.join("agent.stderr.log").display().to_string(),
            "error_message": null,
            "persistence_error": null,
        });
        std::fs::write(
            artifacts_dir.join("run.json"),
            serde_json::to_string_pretty(&record).expect("serialize run record"),
        )
        .expect("write run record");
    }

    #[test]
    fn result_json_failure_keeps_status_and_repairs_durable_error() {
        let temp = tempdir().expect("tempdir");
        let artifacts_dir = temp.path().join("run");
        std::fs::create_dir_all(&artifacts_dir).expect("artifacts dir");
        let summary_path = artifacts_dir.join("summary.json");
        write_sample_run_record(&summary_path, &artifacts_dir);

        let summary = RunSummary {
            run_id: "run-id".to_string(),
            key: "pdev-099b".to_string(),
            slug: "pdev-099b".to_string(),
            created_at: 1778781975,
            phase: crate::state::RunPhase::Finished,
            status: Some(Status::Completed),
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
        result.run_path = Some(artifacts_dir.join("run.json").display().to_string());

        persist_emitted_result(&mut result);

        assert_eq!(result.status, Status::Completed);
        assert!(result
            .persistence_error
            .as_deref()
            .expect("persistence error")
            .starts_with("write result.json:"));

        let repaired = serde_json::from_str::<serde_json::Value>(
            &std::fs::read_to_string(artifacts_dir.join("run.json")).expect("read repaired record"),
        )
        .expect("parse repaired record");
        assert_eq!(repaired["terminal_status"], "completed");
        assert_eq!(
            repaired["persistence_error"],
            result.persistence_error.unwrap()
        );
    }
}
