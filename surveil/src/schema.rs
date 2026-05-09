use serde::{Deserialize, Serialize};

pub const SCHEMA_VERSION: &str = "surveil.v5";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GatherOutput {
    pub schema_version: String,
    pub repo_root: String,
    pub summary: String,
    pub explicit_files: Vec<ExplicitFile>,
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
pub struct TraceOutput {
    pub schema_version: String,
    pub searched_areas: Vec<String>,
    pub skipped_paths: Vec<String>,
    pub files_considered: usize,
    pub files_matched: usize,
    pub unmatched_questions: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::{Answer, Finding, GatherOutput, ResearchOutput, SCHEMA_VERSION};

    #[test]
    fn rejects_unknown_fields_for_gather_output() {
        let json = format!(
            r#"{{
                "schema_version":"{SCHEMA_VERSION}",
                "repo_root":"/repo",
                "summary":"summary",
                "explicit_files":[],
                "search_areas":[],
                "query":[],
                "terms":[],
                "blockers":[],
                "unexpected":true
            }}"#
        );

        let err = serde_json::from_str::<GatherOutput>(&json).expect_err("unknown field rejected");
        assert!(err.to_string().contains("unexpected"));
    }

    #[test]
    fn serializes_query_result_and_symbol_fields_without_legacy_names() {
        let output = ResearchOutput {
            schema_version: SCHEMA_VERSION.to_string(),
            summary: "summary".to_string(),
            result: vec![Answer {
                query: "Where should Tree-sitter attach?".to_string(),
                findings: vec![Finding {
                    path: "src/lib.rs".to_string(),
                    line: 12,
                    excerpt: "fn attach() {}".to_string(),
                    source: "lexical".to_string(),
                    matched_from: "attach".to_string(),
                    symbol_kind: Some("function".to_string()),
                    symbol_name: Some("attach".to_string()),
                    symbol_start_line: Some(10),
                    symbol_end_line: Some(14),
                }],
                negative_evidence: vec![],
            }],
            blockers: vec![],
            open_questions: vec![],
        };

        let json = serde_json::to_value(output).expect("serialize");
        let obj = json.as_object().expect("object");
        assert!(obj.contains_key("result"));
        assert!(!obj.contains_key("answers"));

        let answer = obj["result"].as_array().expect("result array")[0].as_object().expect("answer object");
        assert!(answer.contains_key("query"));
        assert!(!answer.contains_key("question"));

        let finding = answer["findings"].as_array().expect("findings array")[0].as_object().expect("finding object");
        assert!(finding.contains_key("symbol_kind"));
        assert!(finding.contains_key("symbol_name"));
        assert!(finding.contains_key("symbol_start_line"));
        assert!(finding.contains_key("symbol_end_line"));
        assert!(!finding.contains_key("answers"));
        assert!(!finding.contains_key("question"));
    }
}
