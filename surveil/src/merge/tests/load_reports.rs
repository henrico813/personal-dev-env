use super::super::{load_reports, MergeError, ReportInput};
use std::error::Error;
use std::path::{Path, PathBuf};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/merge")
        .join(name)
}

fn input(name: &str) -> ReportInput {
    ReportInput {
        perspective: "tests".to_string(),
        path: fixture(name),
    }
}

#[test]
fn loads_fixture_report() {
    let reports = load_reports(vec![input("architecture-report.json")]).expect("load report");
    assert_eq!(reports[0].perspective, "tests");
    assert_eq!(reports[0].report.summary, "architecture report");
}

#[test]
fn rejects_old_version_first() {
    let error = load_reports(vec![input("old-schema-report.json")])
        .expect_err("reject old version")
        .to_string();
    assert!(error.contains("old-schema-report.json"));
    assert!(error.contains("expected surveil.v6, got surveil.v5"));
}

#[test]
fn rejects_unknown_report_field() {
    let error =
        load_reports(vec![input("unknown-field-report.json")]).expect_err("reject unknown field");
    assert!(error.to_string().contains("unknown-field-report.json"));
    assert!(error.to_string().contains("unknown field `unexpected`"));
    assert!(error.source().is_some());
}

#[test]
fn rejects_malformed_report() {
    let error =
        load_reports(vec![input("malformed-report.json")]).expect_err("reject malformed report");
    assert!(error.to_string().contains("malformed-report.json"));
    assert!(error.to_string().contains("EOF while parsing"));
    assert!(error.source().is_some());
}

#[test]
fn reports_read_source() {
    let path = fixture("missing-report.json");
    let error = load_reports(vec![ReportInput {
        perspective: "missing".to_string(),
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
