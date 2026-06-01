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

    #[test]
    fn parses_gather_output_additions_and_rejects_unknown_fields() {
        let json = format!(
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
        );

        let gather = serde_json::from_str::<GatherOutput>(&json).expect("parse gather output");
        assert_eq!(gather.missing_explicit_files, vec!["docs/future.md"]);
        assert_eq!(gather.skipped_explicit_files, vec![".surveil/index.sqlite"]);

        let invalid = format!(
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
        );

        let err = serde_json::from_str::<GatherOutput>(&invalid).expect_err("unknown field rejected");
        assert!(err.to_string().contains("unexpected"));
    }

    #[test]
    fn parses_trace_output_additions() {
        let json = format!(
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
        );

        let trace = serde_json::from_str::<TraceOutput>(&json).expect("parse trace output");
        assert_eq!(trace.index_state, "usable");
        assert_eq!(trace.queries[0], QueryTrace {
            query: "Where should attach live?".to_string(),
            retrieval_mode: "ranked_only".to_string(),
            ranked_files: vec!["src/lib.rs".to_string()],
            findings_returned: 1,
        });

        let invalid = format!(
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
        );

        let err = serde_json::from_str::<TraceOutput>(&invalid).expect_err("unknown field rejected");
        assert!(err.to_string().contains("unexpected"));
    }
}
