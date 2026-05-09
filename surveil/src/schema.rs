use serde::{Deserialize, Serialize};

pub const SCHEMA_VERSION: &str = "surveil.v5";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GatherOutput {
    pub schema_version: String,
    pub repo_root: String,
    pub summary: String,
    pub explicit_files: Vec<ExplicitFile>,
    pub search_areas: Vec<String>,
    pub questions: Vec<String>,
    pub terms: Vec<String>,
    pub blockers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExplicitFile {
    pub path: String,
    pub found: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResearchOutput {
    pub schema_version: String,
    pub summary: String,
    pub answers: Vec<Answer>,
    pub blockers: Vec<String>,
    pub open_questions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Answer {
    pub question: String,
    pub findings: Vec<Finding>,
    pub negative_evidence: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
pub struct TraceOutput {
    pub schema_version: String,
    pub searched_areas: Vec<String>,
    pub skipped_paths: Vec<String>,
    pub files_considered: usize,
    pub files_matched: usize,
    pub unmatched_questions: Vec<String>,
}
