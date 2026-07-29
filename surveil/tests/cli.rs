use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/merge")
        .join(name)
}

fn run_merge(arguments: &[PathBuf]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_surveil"))
        .arg("merge")
        .args(arguments)
        .output()
        .expect("run surveil merge")
}

fn temp_root(name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    std::env::temp_dir().join(format!("surveil-cli-{name}-{stamp}"))
}

fn run_managed_create(state_home: &Path, task: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_surveil"))
        .args(["new", "task", "--task", task])
        .env("XDG_STATE_HOME", state_home)
        .output()
        .expect("run managed create")
}

fn stdout_path(output: &Output) -> PathBuf {
    PathBuf::from(
        String::from_utf8(output.stdout.clone())
            .expect("UTF-8 path")
            .trim(),
    )
}

#[test]
fn merges_reports_deterministically() {
    let arguments = [
        fixture("architecture-report.json"),
        fixture("tests-report.json"),
    ];
    let first = run_merge(&arguments);
    let second = run_merge(&arguments);
    assert!(first.status.success());
    assert!(first.stderr.is_empty());
    assert!(second.status.success());
    assert!(second.stderr.is_empty());
    assert_eq!(first.stdout, second.stdout);

    let value: Value = serde_json::from_slice(&first.stdout).expect("parse evidence JSON");
    assert_eq!(value["schema_version"], "surveil.evidence.v2");
    assert_eq!(
        value["reports"],
        json!([
            {
                "task_name": "architecture",
                "summary": "architecture report"
            },
            {
                "task_name": "tests",
                "summary": "tests report"
            }
        ])
    );
    assert_eq!(value["findings"].as_array().expect("findings").len(), 1);
    assert_eq!(
        value["findings"][0],
        json!({
            "path": "surveil/src/cli.rs",
            "line": 143,
            "excerpt": "Command::Merge(args) => merge::run(&args.reports),",
            "occurrences": [
                {
                    "task_name": "architecture",
                    "query": "Where?",
                    "rank": 1,
                    "source": "lexical",
                    "matched_from": "merge",
                    "symbol_kind": "function",
                    "symbol_name": "run",
                    "symbol_start_line": 120,
                    "symbol_end_line": 146
                },
                {
                    "task_name": "tests",
                    "query": "How?",
                    "rank": 1,
                    "source": "explicit_file",
                    "matched_from": "merge",
                    "symbol_kind": "function",
                    "symbol_name": "run",
                    "symbol_start_line": 120,
                    "symbol_end_line": 146
                }
            ]
        })
    );
    assert_eq!(value["negative_evidence"][0]["text"], "no existing route");
    assert_eq!(value["blockers"][0]["text"], "strict input required");
    assert_eq!(
        value["open_questions"][0]["text"],
        "which caller adopts this?"
    );
    let output_text = String::from_utf8(first.stdout).expect("UTF-8 output");
    assert!(!output_text.contains(fixture("").to_string_lossy().as_ref()));
}

#[test]
fn merge_help_describes_reports() {
    let output = Command::new(env!("CARGO_BIN_EXE_surveil"))
        .args(["merge", "--help"])
        .output()
        .expect("run merge help");
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let stdout = String::from_utf8(output.stdout).expect("UTF-8 help");
    assert!(stdout.contains("<TASK_REPORT>..."));
}

#[test]
fn rejects_duplicate_task_names() {
    let report = fixture("architecture-report.json");
    let output = run_merge(&[report.clone(), report]);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("duplicate task name"));
}

#[test]
fn rejects_empty_task_name() {
    let output = run_merge(&[fixture("empty-task-name-report.json")]);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("invalid task_name"));
}

#[test]
fn rejects_missing_task_name() {
    let output = run_merge(&[fixture("missing-task-name-report.json")]);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("missing field `task_name`"));
}

#[test]
fn rejects_old_report_version() {
    let output = run_merge(&[fixture("old-schema-report.json")]);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("expected surveil.v7, got surveil.v6"));
}

#[test]
fn rejects_unknown_report_field() {
    let output = run_merge(&[fixture("unknown-field-report.json")]);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("unknown-field-report.json"));
    assert!(stderr.contains("unexpected"));
}

#[test]
fn rejects_malformed_report_json() {
    let output = run_merge(&[fixture("malformed-report.json")]);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("malformed-report.json"));
    assert!(stderr.contains("EOF while parsing a value"));
}

#[test]
fn managed_create_prints_xdg_root() {
    let state_home = temp_root("managed-create");
    let output = run_managed_create(&state_home, "architecture");
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let root = stdout_path(&output);
    assert!(root.starts_with(state_home.join("surveil/runs")));
    assert!(root.join(".surveil-managed").is_file());
    assert!(root.join("architecture/task.md").is_file());
    let _ = fs::remove_dir_all(state_home);
}

#[test]
fn managed_append_creates_task() {
    let state_home = temp_root("managed-append");
    let created = run_managed_create(&state_home, "architecture");
    let root = stdout_path(&created);
    let output = Command::new(env!("CARGO_BIN_EXE_surveil"))
        .args(["new", "task", "--root"])
        .arg(&root)
        .args(["--task", "tests verification"])
        .env("XDG_STATE_HOME", &state_home)
        .output()
        .expect("run managed append");
    assert!(output.status.success());
    assert_eq!(stdout_path(&output), root);
    assert!(root.join("tests verification/task.md").is_file());
    let _ = fs::remove_dir_all(state_home);
}

#[test]
fn managed_duplicate_fails_cleanly() {
    let state_home = temp_root("managed-duplicate");
    let created = run_managed_create(&state_home, "architecture");
    let root = stdout_path(&created);
    let output = Command::new(env!("CARGO_BIN_EXE_surveil"))
        .args(["new", "task", "--root"])
        .arg(&root)
        .args(["--task", "architecture"])
        .env("XDG_STATE_HOME", &state_home)
        .output()
        .expect("run duplicate append");
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(root.join("architecture/task.md").is_file());
    let _ = fs::remove_dir_all(state_home);
}

#[test]
fn managed_rejects_foreign_root() {
    let state_home = temp_root("foreign-root");
    let foreign = state_home.join("foreign");
    fs::create_dir_all(&foreign).expect("create foreign root");
    let output = Command::new(env!("CARGO_BIN_EXE_surveil"))
        .args(["new", "task", "--root"])
        .arg(&foreign)
        .args(["--task", "architecture"])
        .env("XDG_STATE_HOME", &state_home)
        .output()
        .expect("run foreign append");
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(!foreign.join("architecture").exists());
    let _ = fs::remove_dir_all(state_home);
}

#[test]
fn explicit_mode_stays_unchanged() {
    let root = temp_root("explicit");
    let output_dir = root.join("nested/task");
    let output = Command::new(env!("CARGO_BIN_EXE_surveil"))
        .args(["new", "task"])
        .arg(&output_dir)
        .output()
        .expect("run explicit create");
    assert!(output.status.success());
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
    assert!(output_dir.join("task.md").is_file());
    assert!(!root.join(".surveil-managed").exists());
    let _ = fs::remove_dir_all(root);
}
