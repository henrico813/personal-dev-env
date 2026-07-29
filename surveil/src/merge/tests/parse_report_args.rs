use super::super::{parse_task_reports, TaskReport};
use std::path::PathBuf;

#[test]
fn parses_ordered_inputs() {
    let values = vec![PathBuf::from("one.json"), PathBuf::from("two.json")];
    assert_eq!(
        parse_task_reports(&values),
        vec![
            TaskReport {
                path: PathBuf::from("one.json"),
            },
            TaskReport {
                path: PathBuf::from("two.json"),
            },
        ]
    );
}
