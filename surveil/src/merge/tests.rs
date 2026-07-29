mod load_reports;
mod merge_reports;
mod parse_report_args;

use crate::schema::{Answer, Finding, ResearchOutput, SCHEMA_VERSION};

fn finding(source: &str) -> Finding {
    Finding {
        path: "surveil/src/cli.rs".to_string(),
        line: 143,
        excerpt: "Command::Merge(args) => merge::run(&args.reports),".to_string(),
        source: source.to_string(),
        matched_from: "merge".to_string(),
        symbol_kind: Some("function".to_string()),
        symbol_name: Some("run".to_string()),
        symbol_start_line: Some(120),
        symbol_end_line: Some(146),
    }
}

fn report(task_name: &str, query: &str, findings: Vec<Finding>) -> ResearchOutput {
    ResearchOutput {
        schema_version: SCHEMA_VERSION.to_string(),
        task_name: task_name.to_string(),
        summary: "merge command".to_string(),
        result: vec![Answer {
            query: query.to_string(),
            findings,
            negative_evidence: vec!["no existing route".to_string()],
        }],
        blockers: vec!["strict input required".to_string()],
        open_questions: vec!["which caller adopts this?".to_string()],
    }
}
