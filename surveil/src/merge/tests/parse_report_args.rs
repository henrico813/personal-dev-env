use super::super::{parse_report_args, ReportInput};
use std::path::PathBuf;

#[test]
fn parses_ordered_inputs() {
    let values = vec![
        "architecture=one.json".to_string(),
        "tests=two.json".to_string(),
    ];
    assert_eq!(
        parse_report_args(&values).expect("parse inputs"),
        vec![
            ReportInput {
                perspective: "architecture".to_string(),
                path: PathBuf::from("one.json"),
            },
            ReportInput {
                perspective: "tests".to_string(),
                path: PathBuf::from("two.json"),
            },
        ]
    );
}

#[test]
fn rejects_invalid_inputs() {
    for values in [
        vec!["report.json".to_string()],
        vec!["=report.json".to_string()],
        vec!["tests=".to_string()],
    ] {
        assert!(parse_report_args(&values).is_err());
    }
}

#[test]
fn rejects_duplicate_perspectives() {
    let values = vec!["tests=one.json".to_string(), "tests=two.json".to_string()];
    assert_eq!(
        parse_report_args(&values)
            .expect_err("reject duplicate")
            .to_string(),
        "duplicate report perspective: tests"
    );
}
