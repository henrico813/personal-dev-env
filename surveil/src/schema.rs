use serde::{Deserialize, Serialize};

pub const SCHEMA_VERSION: &str = "surveil.v6";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GatherOutput {
    pub schema_version: String,
    pub repo_root: String,
    pub summary: String,
    pub explicit_files: Vec<ExplicitFile>,
    pub missing_explicit_files: Vec<String>,
    pub skipped_explicit_files: Vec<String>,
    pub search_areas: Vec<String>,
    pub query: Vec<String>,
    pub terms: Vec<String>,
    pub blockers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExplicitFile {
    pub path: String,
    pub found: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResearchOutput {
    pub schema_version: String,
    pub summary: String,
    pub result: Vec<Answer>,
    pub blockers: Vec<String>,
    pub open_questions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Answer {
    pub query: String,
    pub findings: Vec<Finding>,
    pub negative_evidence: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Finding {
    pub path: String,
    pub line: u32,
    pub excerpt: String,
    pub source: String,
    pub matched_from: String,
    pub symbol_kind: Option<String>,
    pub symbol_name: Option<String>,
    pub symbol_start_line: Option<u32>,
    pub symbol_end_line: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QueryTrace {
    pub query: String,
    pub retrieval_mode: String,
    pub ranked_files: Vec<String>,
    pub findings_returned: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TraceOutput {
    pub schema_version: String,
    pub searched_areas: Vec<String>,
    pub skipped_paths: Vec<String>,
    pub files_considered: usize,
    pub files_matched: usize,
    pub missing_explicit_files: Vec<String>,
    pub skipped_explicit_files: Vec<String>,
    pub index_state: String,
    pub unmatched_questions: Vec<String>,
    pub queries: Vec<QueryTrace>,
}

#[cfg(test)]
mod tests {
    use super::{GatherOutput, QueryTrace, SCHEMA_VERSION, TraceOutput};

    struct GatherSchemaCase {
        name: &'static str,
        json: String,
        expected_missing: Option<Vec<&'static str>>,
        expected_skipped: Option<Vec<&'static str>>,
        expected_error: Option<&'static str>,
    }

    #[test]
    fn gather_schema_case_tables() {
        let cases = vec![
            GatherSchemaCase {
                name: "parses-explicit-lists",
                json: format!(
                    r#"{{
                        "schema_version":"{SCHEMA_VERSION}",
                        "repo_root":"/repo",
                        "summary":"summary",
                        "explicit_files":[],
                        "missing_explicit_files":["docs/future.md"],
                        "skipped_explicit_files":[".surveil/index.sqlite"],
                        "search_areas":[],
                        "query":[],
                        "terms":[],
                        "blockers":[]
                    }}"#
                ),
                expected_missing: Some(vec!["docs/future.md"]),
                expected_skipped: Some(vec![".surveil/index.sqlite"]),
                expected_error: None,
            },
            GatherSchemaCase {
                name: "rejects-unknown-fields",
                json: format!(
                    r#"{{
                        "schema_version":"{SCHEMA_VERSION}",
                        "repo_root":"/repo",
                        "summary":"summary",
                        "explicit_files":[],
                        "missing_explicit_files":[],
                        "skipped_explicit_files":[],
                        "search_areas":[],
                        "query":[],
                        "terms":[],
                        "blockers":[],
                        "unexpected":true
                    }}"#
                ),
                expected_missing: None,
                expected_skipped: None,
                expected_error: Some("unexpected"),
            },
        ];

        for case in &cases {
            match case.expected_error {
                Some(expected_error) => {
                    let err = serde_json::from_str::<GatherOutput>(&case.json)
                        .expect_err("unknown field rejected");
                    assert!(err.to_string().contains(expected_error), "case: {}", case.name);
                }
                None => {
                    let gather = serde_json::from_str::<GatherOutput>(&case.json)
                        .expect("parse gather output");
                    assert_eq!(
                        gather.missing_explicit_files,
                        case.expected_missing
                            .as_ref()
                            .expect("expected missing list")
                            .iter()
                            .map(|item| item.to_string())
                            .collect::<Vec<_>>(),
                        "case: {}",
                        case.name
                    );
                    assert_eq!(
                        gather.skipped_explicit_files,
                        case.expected_skipped
                            .as_ref()
                            .expect("expected skipped list")
                            .iter()
                            .map(|item| item.to_string())
                            .collect::<Vec<_>>(),
                        "case: {}",
                        case.name
                    );
                }
            }
        }
    }

    struct TraceSchemaCase {
        name: &'static str,
        json: String,
        expected_query: Option<QueryTrace>,
        expected_index_state: Option<&'static str>,
        expected_error: Option<&'static str>,
    }

    #[test]
    fn trace_schema_case_tables() {
        let cases = vec![
            TraceSchemaCase {
                name: "parses-query-trace",
                json: format!(
                    r#"{{
                        "schema_version":"{SCHEMA_VERSION}",
                        "searched_areas":["src/"],
                        "skipped_paths":[],
                        "files_considered":1,
                        "files_matched":1,
                        "missing_explicit_files":["docs/future.md"],
                        "skipped_explicit_files":[".surveil/index.sqlite"],
                        "index_state":"usable",
                        "unmatched_questions":[],
                        "queries":[{{
                            "query":"Where should attach live?",
                            "retrieval_mode":"ranked_only",
                            "ranked_files":["src/lib.rs"],
                            "findings_returned":1
                        }}]
                    }}"#
                ),
                expected_query: Some(QueryTrace {
                    query: "Where should attach live?".to_string(),
                    retrieval_mode: "ranked_only".to_string(),
                    ranked_files: vec!["src/lib.rs".to_string()],
                    findings_returned: 1,
                }),
                expected_index_state: Some("usable"),
                expected_error: None,
            },
            TraceSchemaCase {
                name: "rejects-unknown-fields",
                json: format!(
                    r#"{{
                        "schema_version":"{SCHEMA_VERSION}",
                        "searched_areas":[],
                        "skipped_paths":[],
                        "files_considered":0,
                        "files_matched":0,
                        "missing_explicit_files":[],
                        "skipped_explicit_files":[],
                        "index_state":"missing",
                        "unmatched_questions":[],
                        "queries":[],
                        "unexpected":true
                    }}"#
                ),
                expected_query: None,
                expected_index_state: None,
                expected_error: Some("unexpected"),
            },
        ];

        for case in &cases {
            match case.expected_error {
                Some(expected_error) => {
                    let err = serde_json::from_str::<TraceOutput>(&case.json)
                        .expect_err("unknown field rejected");
                    assert!(err.to_string().contains(expected_error), "case: {}", case.name);
                }
                None => {
                    let trace = serde_json::from_str::<TraceOutput>(&case.json)
                        .expect("parse trace output");
                    assert_eq!(
                        trace.index_state,
                        case.expected_index_state.expect("expected index state"),
                        "case: {}",
                        case.name
                    );
                    assert_eq!(
                        trace.queries[0],
                        case.expected_query.clone().expect("expected query trace"),
                        "case: {}",
                        case.name
                    );
                }
            }
        }
    }
}
