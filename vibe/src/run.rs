use std::fs;
use std::path::Path;

use crate::{
    cli::RunArgs,
    docker, git,
    output::{RunResult, Status},
    paths, planner, prompt, runtime,
};

/// Execute one planner step end-to-end and return the stable JSON result.
pub fn execute(args: RunArgs) -> RunResult {
    let planner_bin = match planner::locate_planner() {
        Some(path) => path,
        None => return RunResult::setup_error("planner not found"),
    };
    let repo = match git::repo_layout() {
        Ok(layout) => layout,
        Err(err) => return RunResult::setup_error(err),
    };
    let branch = paths::branch_slug(&args.plan);
    let worktree = paths::worktree_path(&repo.canonical_repo_root, &branch);
    let runtime_root = match runtime::ensure_runtime_assets() {
        Ok(path) => path,
        Err(err) => return RunResult::setup_error(format!("vibe runtime setup failed: {err}")),
    };
    let run_paths = match paths::create_run_paths(&repo.canonical_repo_root, &branch, args.step) {
        Ok(paths) => paths,
        Err(err) => return RunResult::setup_error(err),
    };

    let plan_json = match planner::inspect(&planner_bin, &args.plan) {
        Ok(json) => json,
        Err(err) => return RunResult::setup_error(err),
    };
    let step_json = match planner::extract_step(&plan_json, args.step) {
        Ok(json) => json,
        Err(err) => return RunResult::setup_error(err),
    };
    let prompt = match prompt::compose(&step_json) {
        Ok(text) => text,
        Err(err) => return RunResult::setup_error(err),
    };
    if let Err(err) = fs::write(
        &run_paths.step_json,
        serde_json::to_string_pretty(&step_json).unwrap_or_default(),
    ) {
        return RunResult::setup_error(format!("write step.json: {err}"));
    }
    if let Err(err) = fs::write(&run_paths.prompt_txt, prompt) {
        return RunResult::setup_error(format!("write prompt.txt: {err}"));
    }

    let (remote, base_branch) = match git::resolve_base(&repo.canonical_repo_root) {
        Ok(value) => value,
        Err(err) => return RunResult::setup_error(err),
    };
    if let Err(err) = git::ensure_worktree(
        &repo.canonical_repo_root,
        &worktree,
        &branch,
        &remote,
        &base_branch,
    ) {
        return RunResult::setup_error(err);
    }
    match git::is_dirty(&worktree) {
        Ok(true) => {
            return RunResult {
                status: Status::RefusedDirty,
                step: Some(args.step),
                branch: Some(branch),
                worktree: Some(worktree.display().to_string()),
                model: Some(args.model),
                pre_step_commit: None,
                commit: None,
                snapshot_commits: Vec::new(),
                events_log_path: Some(run_paths.events_jsonl.display().to_string()),
                stderr_path: Some(run_paths.stderr_log.display().to_string()),
                error_message: Some("worktree has uncommitted changes".to_string()),
            };
        }
        Err(err) => return RunResult::setup_error(err),
        Ok(false) => {}
    }

    let pre_step_commit = match git::head_sha(&worktree) {
        Ok(sha) => sha,
        Err(err) => return RunResult::setup_error(err),
    };
    if let Err(err) = docker::require_docker() {
        return RunResult::setup_error(err);
    }
    if let Err(err) = docker::ensure_image(&runtime_root) {
        return RunResult::setup_error(err);
    }
    let agent_exit = match docker::run_step(
        &repo.canonical_repo_root,
        &repo.git_common_dir,
        &worktree,
        &run_paths,
        &args.model,
    ) {
        Ok(code) => code,
        Err(err) => return RunResult::setup_error(err),
    };

    let snapshot_commits = read_snapshot_shas(&run_paths.snapshots_jsonl);
    let dirty_after = git::is_dirty(&worktree).unwrap_or(false);
    let title = planner::step_title(&step_json, args.step);
    let mut status = if agent_exit == 0 {
        Status::Noop
    } else {
        Status::AgentFailed
    };
    let mut commit = None;
    let mut error_message = None;
    let changed = dirty_after;
    if changed {
        match git::commit_all(
            &worktree,
            &format!("vibe: step {} {}", args.step, title),
            &runtime_root.join("hooks"),
            "final",
        ) {
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
        step: Some(args.step),
        branch: Some(branch),
        worktree: Some(worktree.display().to_string()),
        model: Some(args.model),
        pre_step_commit: Some(pre_step_commit),
        commit,
        snapshot_commits,
        events_log_path: Some(run_paths.events_jsonl.display().to_string()),
        stderr_path: Some(run_paths.stderr_log.display().to_string()),
        error_message,
    }
}

fn read_snapshot_shas(path: &Path) -> Vec<String> {
    let Ok(text) = fs::read_to_string(path) else {
        return Vec::new();
    };
    parse_snapshot_shas(&text)
}

fn parse_snapshot_shas(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter_map(|line| {
            line.get("sha")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::parse_snapshot_shas;

    #[test]
    fn snapshot_parser_reads_shas() {
        let shas = parse_snapshot_shas("{\"sha\":\"abc\"}\n{\"sha\":\"def\"}\n");

        assert_eq!(shas, vec!["abc", "def"]);
    }

    #[test]
    fn snapshot_parser_skips_bad_lines() {
        let shas = parse_snapshot_shas("not json\n{\"event\":\"skip\"}\n{\"sha\":\"abc\"}\n");

        assert_eq!(shas, vec!["abc"]);
    }
}
