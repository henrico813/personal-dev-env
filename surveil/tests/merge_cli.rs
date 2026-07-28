use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/merge")
        .join(name)
}

fn run_merge(arguments: &[String]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_surveil"))
        .arg("merge")
        .args(arguments)
        .output()
        .expect("run surveil merge")
}

fn argument(label: &str, name: &str) -> String {
    format!("{label}={}", fixture(name).display())
}

#[test]
fn merges_reports_deterministically() {
    let arguments = [
        argument("architecture", "architecture-report.json"),
        argument("tests", "tests-report.json"),
    ];
    let first = run_merge(&arguments);
    let second = run_merge(&arguments);
    assert!(first.status.success());
    assert!(first.stderr.is_empty());
    assert!(second.status.success());
    assert!(second.stderr.is_empty());
    assert_eq!(first.stdout, second.stdout);

    let value: Value = serde_json::from_slice(&first.stdout).expect("parse evidence JSON");
    assert_eq!(value["schema_version"], "surveil.evidence.v1");
    assert_eq!(
        value["reports"],
        json!([
            {
                "perspective": "architecture",
                "summary": "architecture report"
            },
            {
                "perspective": "tests",
                "summary": "tests report"
            }
        ])
    );
    assert_eq!(value["findings"].as_array().expect("findings").len(), 1);
    assert_eq!(
        value["findings"][0],
        json!({
            "path": "surveil/src/cli.rs",
            "line": 84,
            "excerpt": "Command::Merge(args) => merge::run(&args.reports),",
            "occurrences": [
                {
                    "perspective": "architecture",
                    "query": "Where?",
                    "rank": 1,
                    "source": "lexical",
                    "matched_from": "merge",
                    "symbol_kind": "function",
                    "symbol_name": "run",
                    "symbol_start_line": 75,
                    "symbol_end_line": 86
                },
                {
                    "perspective": "tests",
                    "query": "How?",
                    "rank": 1,
                    "source": "explicit_file",
                    "matched_from": "merge",
                    "symbol_kind": "function",
                    "symbol_name": "run",
                    "symbol_start_line": 75,
                    "symbol_end_line": 86
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
    assert!(stdout.contains("<PERSPECTIVE=REPORT>..."));
}

#[test]
fn rejects_duplicate_perspectives() {
    let output = run_merge(&[
        argument("tests", "architecture-report.json"),
        argument("tests", "tests-report.json"),
    ]);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("duplicate report perspective"));
}

#[test]
fn rejects_old_report_version() {
    let output = run_merge(&[argument("old", "old-schema-report.json")]);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("expected surveil.v6, got surveil.v5"));
}

#[test]
fn rejects_unknown_report_field() {
    let output = run_merge(&[argument("unknown", "unknown-field-report.json")]);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("unknown-field-report.json"));
    assert!(stderr.contains("unexpected"));
}

#[test]
fn rejects_malformed_report_json() {
    let output = run_merge(&[argument("broken", "malformed-report.json")]);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("malformed-report.json"));
    assert!(stderr.contains("EOF while parsing a value"));
}
