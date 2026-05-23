use crate::schema::Finding;
use crate::source::SourceFile;
use std::collections::{BTreeSet, HashMap};
use std::path::PathBuf;

mod output;
mod rank;
mod scan;
mod setup;
mod tokenize;

pub(crate) use output::run;

#[derive(Default)]
pub(super) struct TraceState {
    pub(super) files_considered: BTreeSet<PathBuf>,
    pub(super) files_matched: BTreeSet<PathBuf>,
    pub(super) skipped_paths: Vec<String>,
    pub(super) unmatched_questions: Vec<String>,
}

pub(super) const MAX_FINDINGS_PER_FILE: usize = 3;
pub(super) const RANKED_FILE_LIMIT: usize = 8;

pub(super) struct RankedFileFindings {
    pub(super) display_path: String,
    pub(super) explicit: bool,
    pub(super) best_chunk_score: Option<f32>,
    pub(super) findings: Vec<Finding>,
}

pub(super) struct LoadedFile {
    pub(super) source: SourceFile,
    pub(super) text: String,
    pub(super) lines: Vec<CorpusLine>,
}

pub(super) struct CorpusLine {
    pub(super) number: u32,
    pub(super) start: usize,
    pub(super) end: usize,
    pub(super) lower_text: String,
}

pub(super) type LiveFileCache = HashMap<PathBuf, LoadedFile>;

#[derive(Debug)]
pub(super) struct MatchedFinding {
    pub(super) finding: Finding,
    pub(super) byte_offset: usize,
}

#[cfg(test)]
mod tests {
    use super::output;
    use crate::schema::{ExplicitFile, GatherOutput, SCHEMA_VERSION};
    use std::fs;
    use std::io::Write;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_repo(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("surveil-{name}-{stamp}"));
        fs::create_dir_all(&path).expect("create temp repo");
        path
    }

    fn write_file(path: &PathBuf, content: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent dirs");
        }
        let mut file = fs::File::create(path).expect("create file");
        file.write_all(content.as_bytes()).expect("write file");
    }

    #[derive(Clone)]
    struct RunCase {
        name: &'static str,
        files: Vec<(&'static str, &'static str)>,
        context: GatherOutput,
        expected_result_count: usize,
        expected_open_questions: Vec<&'static str>,
        expected_skipped_paths: Vec<&'static str>,
    }

    fn run_case(case: &RunCase) {
        let repo = temp_repo(case.name);
        for (path, content) in &case.files {
            write_file(&repo.join(path), content);
        }

        let mut gather = case.context.clone();
        gather.repo_root = repo.to_string_lossy().into_owned();
        let (report, trace) = output::run_for_test(gather).expect("run for test");

        assert_eq!(report.result.len(), case.expected_result_count);
        assert_eq!(
            report.open_questions,
            case.expected_open_questions
                .iter()
                .map(|item| item.to_string())
                .collect::<Vec<_>>()
        );
        assert_eq!(
            trace.skipped_paths,
            case.expected_skipped_paths
                .iter()
                .map(|item| item.to_string())
                .collect::<Vec<_>>()
        );

        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn run_case_tables() {
        let cases = vec![
            RunCase {
                name: "trace-dedupes-skipped-paths",
                files: vec![("surveil/src/lib.rs", "// tree-sitter verified\n")],
                context: GatherOutput {
                    schema_version: SCHEMA_VERSION.to_string(),
                    repo_root: String::new(),
                    summary: "summary".to_string(),
                    explicit_files: vec![
                        ExplicitFile {
                            path: ".surveil/index.sqlite".to_string(),
                            found: true,
                        },
                        ExplicitFile {
                            path: ".surveil/index.sqlite".to_string(),
                            found: true,
                        },
                    ],
                    search_areas: vec!["surveil/".to_string()],
                    query: vec![
                        "Where should Tree-sitter attach?".to_string(),
                        "How should this change be verified?".to_string(),
                    ],
                    terms: vec!["tree-sitter".to_string(), "verified".to_string()],
                    blockers: Vec::new(),
                },
                expected_result_count: 2,
                expected_open_questions: vec![],
                expected_skipped_paths: vec![".surveil/index.sqlite"],
            },
            RunCase {
                name: "multi-query-parity",
                files: vec![
                    ("notes/design.md", "// tree-sitter attach\n"),
                    (
                        "surveil/src/lib.rs",
                        "fn attach() {\r\n    // tree-sitter attach one\r\n    // tree-sitter attach two\r\n    // tree-sitter attach three\r\n    // tree-sitter attach four\r\n}\r\n",
                    ),
                ],
                context: GatherOutput {
                    schema_version: SCHEMA_VERSION.to_string(),
                    repo_root: String::new(),
                    summary: "summary".to_string(),
                    explicit_files: vec![ExplicitFile {
                        path: "notes/design.md".to_string(),
                        found: true,
                    }],
                    search_areas: vec!["surveil/".to_string()],
                    query: vec![
                        "Where should Tree-sitter attach?".to_string(),
                        "How should attach be verified?".to_string(),
                        "Where should missing live?".to_string(),
                    ],
                    terms: vec![
                        "tree-sitter".to_string(),
                        "attach".to_string(),
                        "missing".to_string(),
                    ],
                    blockers: Vec::new(),
                },
                expected_result_count: 3,
                expected_open_questions: vec!["Where should missing live?"],
                expected_skipped_paths: vec![],
            },
        ];

        for case in &cases {
            run_case(case);
        }
    }
}
