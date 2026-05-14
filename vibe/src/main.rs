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

fn main() {
    match cli::parse() {
        ParsedCommand::Run(args) => {
            let mut result = app::execute(args);
            if let Err(err) = persist_result_json(&result) {
                let _ = ledger::record_late_persistence_error(
                    &mut result,
                    format!("write result.json: {err}"),
                );
            }
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
