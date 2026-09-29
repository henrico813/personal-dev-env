#![cfg(target_os = "linux")]

mod common;

use std::fs;

fn assert_status(mode: &str, expected_status: &str, expected_exit: i32) -> serde_json::Value {
    let key = format!("status-{}", mode.replace('_', "-"));
    let fixture = common::setup_fixture(&key);
    let output = common::run_vibe(&fixture, &key, mode);
    let result = common::parse_result(&output);

    assert_eq!(output.status.code(), Some(expected_exit), "{mode}");
    assert_eq!(result["status"], expected_status, "{mode}");
    result
}

#[test]
fn dirty_worktree_is_refused() {
    let key = "status-refused-dirty";
    let fixture = common::setup_fixture(key);
    fs::write(fixture.worktree.join("dirty.txt"), "dirty\n").expect("write dirty file");

    let output = common::run_vibe(&fixture, key, "noop");
    let result = common::parse_result(&output);

    assert_eq!(output.status.code(), Some(4));
    assert_eq!(result["status"], "refused_dirty");
    assert!(result["pre_run_commit"].is_null());
}

#[test]
fn clean_agent_run_is_noop() {
    let result = assert_status("noop", "noop", 1);

    assert!(result["pre_run_commit"].is_string());
    assert_eq!(
        result["snapshot_commits"],
        serde_json::json!(["snapshot-sha"])
    );
    assert!(result["commit"].is_null());
}

#[test]
fn changed_agent_run_is_completed() {
    let result = assert_status("completed", "completed", 0);

    assert!(result["pre_run_commit"].is_string());
    assert_eq!(
        result["snapshot_commits"],
        serde_json::json!(["snapshot-sha"])
    );
    assert!(result["commit"].is_string());
    assert_eq!(
        result["changed_files"],
        serde_json::json!(["agent-output.txt"])
    );
}

#[test]
fn failing_agent_run_reports_failure() {
    let result = assert_status("agent_failed", "agent_failed", 2);

    assert!(result["pre_run_commit"].is_string());
    assert_eq!(
        result["snapshot_commits"],
        serde_json::json!(["snapshot-sha"])
    );
    assert!(result["commit"].is_null());
}

#[test]
fn missing_snapshots_reports_failure() {
    let result = assert_status("snapshot_failed", "snapshot_failed", 5);

    assert!(result["pre_run_commit"].is_string());
    assert_eq!(result["snapshot_commits"], serde_json::json!([]));
}
