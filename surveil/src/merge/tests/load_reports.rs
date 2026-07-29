use super::super::{load_task_reports, MergeError, TaskReport};
use std::error::Error;
use std::path::{Path, PathBuf};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/merge")
        .join(name)
}

fn input(name: &str) -> TaskReport {
    TaskReport { path: fixture(name) }
}

#[test]
fn loads_fixture_report() {
    let reports = load_task_reports(vec![input("architecture-report.json")]).expect("load report");
    assert_eq!(reports[0].report.task_name, "architecture");
    assert_eq!(reports[0].report.summary, "architecture report");
}

#[test]
fn rejects_old_version_first() {
    let error = load_task_reports(vec![input("old-schema-report.json")])
        .expect_err("reject old version")
        .to_string();
    assert!(error.contains("old-schema-report.json"));
    assert!(error.contains("expected surveil.v7, got surveil.v6"));
}

#[test]
fn rejects_unknown_report_field() {
    let error =
        load_task_reports(vec![input("unknown-field-report.json")]).expect_err("reject unknown field");
    assert!(error.to_string().contains("unknown-field-report.json"));
    assert!(error.to_string().contains("unknown field `unexpected`"));
    assert!(error.source().is_some());
}

#[test]
fn rejects_malformed_report() {
    let error =
        load_task_reports(vec![input("malformed-report.json")]).expect_err("reject malformed report");
    assert!(error.to_string().contains("malformed-report.json"));
    assert!(error.to_string().contains("EOF while parsing"));
    assert!(error.source().is_some());
}

#[test]
fn reports_read_source() {
    let path = std::env::temp_dir().join("surveil-missing-report.json");
    let _ = std::fs::remove_file(&path);
    let error = load_task_reports(vec![TaskReport {
        path: path.clone(),
    }])
    .expect_err("reject missing report");
    assert!(error.to_string().contains(path.to_string_lossy().as_ref()));
    assert!(matches!(
        error,
        MergeError::ReadReport { source, .. }
            if source.kind() == std::io::ErrorKind::NotFound
    ));
}

#[test]
fn rejects_empty_task_name() {
    let error = load_task_reports(vec![input("empty-task-name-report.json")])
        .expect_err("reject empty task name");
    assert!(error.to_string().contains("invalid task_name"));
}

#[test]
fn rejects_missing_task_name() {
    let error = load_task_reports(vec![input("missing-task-name-report.json")])
        .expect_err("reject missing task name");
    assert!(error.to_string().contains("missing field `task_name`"));
}

#[test]
fn rejects_duplicate_task_names() {
    let error = load_task_reports(vec![
        input("architecture-report.json"),
        input("architecture-report.json"),
    ])
    .expect_err("reject duplicate task name");
    assert_eq!(error.to_string(), "duplicate task name: architecture");
}
