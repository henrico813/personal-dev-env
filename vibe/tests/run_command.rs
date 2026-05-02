mod support;

use std::fs;
use std::path::PathBuf;
use std::process::Output;

use serde_json::{json, Value};

use support::{git_stdout, TestEnv};

fn parse_result(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).expect("parse stdout json")
}

fn assert_base_result_fields(result: &Value) {
    assert_eq!(result["step"], 1);
    assert_eq!(result["model"], "openai-codex/gpt-5.5");
    assert!(result["branch"].as_str().is_some());
    assert!(result["worktree"].as_str().is_some());
    assert!(result["events_log_path"].as_str().is_some());
    assert!(result["stderr_path"].as_str().is_some());
    assert!(result["snapshot_commits"].is_array());
}

#[test]
fn changed_run_completes() {
    let env = TestEnv::new();
    env.set_identity();

    let output = env.run("write_success");
    let result = parse_result(&output);

    assert_base_result_fields(&result);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(result["status"], "completed");
    assert!(result["pre_step_commit"].as_str().is_some());
    assert!(result["commit"].as_str().is_some());
}

#[test]
fn clean_run_noops() {
    let env = TestEnv::new();

    let output = env.run("noop");
    let result = parse_result(&output);

    assert_base_result_fields(&result);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(result["status"], "noop");
    assert!(result["pre_step_commit"].as_str().is_some());
    assert!(result["commit"].is_null());
}

#[test]
fn dirty_worktree_refuses() {
    let env = TestEnv::new();

    let first = parse_result(&env.run("noop"));
    let worktree = PathBuf::from(first["worktree"].as_str().unwrap());
    fs::write(worktree.join("dirty.txt"), "dirty\n").expect("dirty file");

    let output = env.run("write_success");
    let result = parse_result(&output);

    assert_base_result_fields(&result);
    assert_eq!(output.status.code(), Some(4));
    assert_eq!(result["status"], "refused_dirty");
    assert!(result["pre_step_commit"].is_null());
    assert_eq!(
        result["error_message"].as_str(),
        Some("worktree has uncommitted changes")
    );
}

#[test]
fn failed_clean_run_reports() {
    let env = TestEnv::new();

    let output = env.run("fail_clean");
    let result = parse_result(&output);

    assert_base_result_fields(&result);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(result["status"], "agent_failed");
    assert!(result["pre_step_commit"].as_str().is_some());
    assert!(result["commit"].is_null());
}

#[test]
fn failed_changed_run_commits() {
    let env = TestEnv::new();
    env.set_identity();

    let output = env.run("write_fail");
    let result = parse_result(&output);

    assert_base_result_fields(&result);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(result["status"], "agent_failed");
    assert!(result["pre_step_commit"].as_str().is_some());
    assert!(result["commit"].as_str().is_some());
}

#[test]
fn commit_failure_reports() {
    let env = TestEnv::new();

    let output = env.run("write_success");
    let result = parse_result(&output);

    assert_base_result_fields(&result);
    assert_eq!(output.status.code(), Some(3));
    assert_eq!(result["status"], "commit_failed");
    assert!(result["pre_step_commit"].as_str().is_some());
    assert!(result["commit"].is_null());
    assert_eq!(result["error_message"].as_str(), Some("git commit failed"));
}

#[test]
fn stdout_stays_json() {
    let env = TestEnv::new();

    let output = env.run("noop");
    let stderr = String::from_utf8_lossy(&output.stderr);

    parse_result(&output);
    assert!(stderr.contains("fake docker build stdout"));
    assert!(stderr.contains("fake docker build stderr"));
}

#[test]
fn artifacts_are_written() {
    let env = TestEnv::new();
    env.set_identity();

    let output = env.run("write_success");
    let result = parse_result(&output);

    let events = PathBuf::from(result["events_log_path"].as_str().unwrap());
    let stderr = PathBuf::from(result["stderr_path"].as_str().unwrap());
    let run_dir = events.parent().expect("run dir");

    assert!(events.exists());
    assert!(stderr.exists());
    assert!(run_dir.join("step.json").exists());
    assert!(run_dir.join("prompt.txt").exists());
}

#[test]
fn snapshot_commits_are_reported() {
    let env = TestEnv::new();
    env.set_identity();

    let output = env.run("write_success_with_snapshots");
    let result = parse_result(&output);

    assert_base_result_fields(&result);
    assert_eq!(result["snapshot_commits"], json!(["abc", "def"]));
}

#[test]
fn worktree_reuses_checkout() {
    let env = TestEnv::new();

    let first = parse_result(&env.run("noop"));
    let second = parse_result(&env.run("noop"));
    let worktree = first["worktree"].as_str().unwrap();
    let listing = git_stdout(&env.repo, &["worktree", "list"]);

    assert_eq!(first["worktree"], second["worktree"]);
    assert_eq!(listing.matches(worktree).count(), 1);
}

#[test]
fn docker_missing_reports() {
    let env = TestEnv::new();

    let output = env.run_with("noop", &[("FAKE_DOCKER_VERSION_FAIL", "1")]);
    let result = parse_result(&output);

    assert_eq!(output.status.code(), Some(5));
    assert_eq!(result["status"], "setup_error");
    assert_eq!(
        result["error_message"].as_str(),
        Some("docker not available")
    );
}

#[test]
fn planner_missing_reports() {
    let env = TestEnv::new();

    std::fs::remove_file(env.home.join(".claude/bin/planner")).expect("remove planner");
    let output = env.run("noop");
    let result = parse_result(&output);

    assert_eq!(output.status.code(), Some(5));
    assert_eq!(result["status"], "setup_error");
    assert_eq!(result["error_message"].as_str(), Some("planner not found"));
}

#[test]
fn planner_inspect_failure_reports() {
    let env = TestEnv::new();

    std::fs::write(
        env.home.join(".claude/bin/planner"),
        "#!/bin/sh\necho planner inspect failed >&2\nexit 1\n",
    )
    .expect("rewrite planner");
    let output = env.run("noop");
    let result = parse_result(&output);

    assert_eq!(output.status.code(), Some(5));
    assert_eq!(result["status"], "setup_error");
    assert_eq!(
        result["error_message"].as_str(),
        Some("planner inspect failed")
    );
}

#[test]
fn step_out_of_range_reports() {
    let env = TestEnv::new();

    let output = env.run_with("noop", &[("VIBE_TEST_STEP", "2")]);
    let result = parse_result(&output);

    assert_eq!(output.status.code(), Some(5));
    assert_eq!(result["status"], "setup_error");
    assert_eq!(
        result["error_message"].as_str(),
        Some("step 2 out of range")
    );
}
