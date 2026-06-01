use crate::schema::Finding;
use crate::source::SourceFile;
use std::collections::{BTreeSet, HashMap};
use std::path::PathBuf;

mod output;
mod rank;
mod scan;
mod setup;
mod tokenize;

pub(crate) use output::write_research_output as run;

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
    use crate::index;
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
        build_index: bool,
        rewrite_files: Vec<(&'static str, &'static str)>,
        context: GatherOutput,
        expected_first_paths: Vec<&'static str>,
        expected_first_sources: Vec<&'static str>,
        expected_first_excerpts: Vec<&'static str>,
        expected_negative_evidence: Vec<Vec<&'static str>>,
        expected_open_questions: Vec<&'static str>,
        expected_skipped_paths: Vec<&'static str>,
    }

    fn run_case(case: &RunCase) {
        let repo = temp_repo(case.name);
        for (path, content) in &case.files {
            write_file(&repo.join(path), content);
        }
        if case.build_index {
            index::build_chunk_index(&repo).expect("build index");
        }
        for (path, content) in &case.rewrite_files {
            write_file(&repo.join(path), content);
        }

        let mut gather = case.context.clone();
        gather.repo_root = repo.to_string_lossy().into_owned();
        let (report, trace) = output::create_test_outputs(gather).expect("run for test");

        assert_eq!(report.schema_version, SCHEMA_VERSION);
        assert_eq!(trace.schema_version, SCHEMA_VERSION);
        assert_eq!(report.result.len(), case.expected_first_paths.len());
        for (index, answer) in report.result.iter().enumerate() {
            if case.expected_first_paths[index].is_empty() {
                assert!(answer.findings.is_empty(), "case: {} answer: {}", case.name, index);
            } else {
                assert_eq!(
                    answer.findings[0].path,
                    case.expected_first_paths[index],
                    "case: {} answer: {}",
                    case.name,
                    index
                );
                assert_eq!(
                    answer.findings[0].source,
                    case.expected_first_sources[index],
                    "case: {} answer: {}",
                    case.name,
                    index
                );
                assert_eq!(
                    answer.findings[0].excerpt,
                    case.expected_first_excerpts[index],
                    "case: {} answer: {}",
                    case.name,
                    index
                );
            }
            assert_eq!(
                answer.negative_evidence,
                case.expected_negative_evidence[index]
                    .iter()
                    .map(|item| item.to_string())
                    .collect::<Vec<_>>(),
                "case: {} answer: {}",
                case.name,
                index
            );
        }
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
                build_index: false,
                rewrite_files: vec![],
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
                    missing_explicit_files: Vec::new(),
                    skipped_explicit_files: vec![".surveil/index.sqlite".to_string()],
                    search_areas: vec!["surveil/".to_string()],
                    query: vec![
                        "Where should Tree-sitter attach?".to_string(),
                        "How should this change be verified?".to_string(),
                    ],
                    terms: vec!["tree-sitter".to_string(), "verified".to_string()],
                    blockers: Vec::new(),
                },
                expected_first_paths: vec!["surveil/src/lib.rs", "surveil/src/lib.rs"],
                expected_first_sources: vec!["lexical", "lexical"],
                expected_first_excerpts: vec!["// tree-sitter verified", "// tree-sitter verified"],
                expected_negative_evidence: vec![vec![], vec![]],
                expected_open_questions: vec![],
                expected_skipped_paths: vec![".surveil/index.sqlite"],
            },
            RunCase {
                name: "usable-index-prefers-ranked-file-across-queries",
                files: vec![
                    (
                        "docs/guide.md",
                        "attach attach attach attach\nattach attach attach attach\n",
                    ),
                    (
                        "src/lib.rs",
                        "fn attach_handler() {\n    // attach handler\n}\n",
                    ),
                ],
                build_index: true,
                rewrite_files: vec![],
                context: GatherOutput {
                    schema_version: SCHEMA_VERSION.to_string(),
                    repo_root: String::new(),
                    summary: "summary".to_string(),
                    explicit_files: vec![],
                    missing_explicit_files: Vec::new(),
                    skipped_explicit_files: Vec::new(),
                    search_areas: vec!["docs/".to_string(), "src/".to_string()],
                    query: vec![
                        "Where should attach handler live?".to_string(),
                        "How should handler attach?".to_string(),
                    ],
                    terms: vec!["attach".to_string(), "handler".to_string()],
                    blockers: Vec::new(),
                },
                expected_first_paths: vec!["src/lib.rs", "src/lib.rs"],
                expected_first_sources: vec!["lexical", "lexical"],
                expected_first_excerpts: vec!["fn attach_handler() {", "fn attach_handler() {"],
                expected_negative_evidence: vec![vec![], vec![]],
                expected_open_questions: vec![],
                expected_skipped_paths: vec![],
            },
            RunCase {
                name: "stale-index-falls-back-across-queries",
                files: vec![("notes/design.md", "old text\n")],
                build_index: true,
                rewrite_files: vec![("notes/design.md", "attach here\n")],
                context: GatherOutput {
                    schema_version: SCHEMA_VERSION.to_string(),
                    repo_root: String::new(),
                    summary: "summary".to_string(),
                    explicit_files: vec![],
                    missing_explicit_files: Vec::new(),
                    skipped_explicit_files: Vec::new(),
                    search_areas: vec!["notes/".to_string()],
                    query: vec![
                        "Where should attach live?".to_string(),
                        "How should attach be verified?".to_string(),
                    ],
                    terms: vec!["attach".to_string()],
                    blockers: Vec::new(),
                },
                expected_first_paths: vec!["notes/design.md", "notes/design.md"],
                expected_first_sources: vec!["lexical", "lexical"],
                expected_first_excerpts: vec!["attach here", "attach here"],
                expected_negative_evidence: vec![vec![], vec![]],
                expected_open_questions: vec![],
                expected_skipped_paths: vec![],
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
                build_index: false,
                rewrite_files: vec![],
                context: GatherOutput {
                    schema_version: SCHEMA_VERSION.to_string(),
                    repo_root: String::new(),
                    summary: "summary".to_string(),
                    explicit_files: vec![ExplicitFile {
                        path: "notes/design.md".to_string(),
                        found: true,
                    }],
                    missing_explicit_files: Vec::new(),
                    skipped_explicit_files: Vec::new(),
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
                expected_first_paths: vec!["notes/design.md", "notes/design.md", ""],
                expected_first_sources: vec!["explicit_file", "explicit_file", ""],
                expected_first_excerpts: vec!["// tree-sitter attach", "// tree-sitter attach", ""],
                expected_negative_evidence: vec![
                    vec![],
                    vec![],
                    vec!["searched declared areas: surveil/"],
                ],
                expected_open_questions: vec!["Where should missing live?"],
                expected_skipped_paths: vec![],
            },
        ];

        for case in &cases {
            run_case(case);
        }
    }

    struct TraceCase {
        name: &'static str,
        files: Vec<(&'static str, &'static str)>,
        build_index: bool,
        context: GatherOutput,
        expected_missing: Vec<&'static str>,
        expected_skipped: Vec<&'static str>,
        expected_index_state: &'static str,
        expected_retrieval_mode: &'static str,
        expected_ranked_files: Vec<&'static str>,
    }

    #[test]
    fn trace_case_tables() {
        let cases = vec![
            TraceCase {
                name: "records-missing-and-skipped-explicit-paths",
                files: vec![("surveil/src/lib.rs", "// tree-sitter attach\n")],
                build_index: false,
                context: GatherOutput {
                    schema_version: SCHEMA_VERSION.to_string(),
                    repo_root: String::new(),
                    summary: "summary".to_string(),
                    explicit_files: vec![
                        ExplicitFile {
                            path: "docs/future.md".to_string(),
                            found: false,
                        },
                        ExplicitFile {
                            path: ".surveil/index.sqlite".to_string(),
                            found: false,
                        },
                        ExplicitFile {
                            path: "surveil/src/lib.rs".to_string(),
                            found: true,
                        },
                    ],
                    missing_explicit_files: vec!["docs/future.md".to_string()],
                    skipped_explicit_files: vec![".surveil/index.sqlite".to_string()],
                    search_areas: vec!["surveil/".to_string()],
                    query: vec!["Where should Tree-sitter attach?".to_string()],
                    terms: vec!["tree-sitter".to_string()],
                    blockers: Vec::new(),
                },
                expected_missing: vec!["docs/future.md"],
                expected_skipped: vec![".surveil/index.sqlite"],
                expected_index_state: "missing",
                expected_retrieval_mode: "full_lexical_scan",
                expected_ranked_files: vec![],
            },
            TraceCase {
                name: "records-ranked-only-query-trace",
                files: vec![
                    ("docs/guide.md", "attach attach attach attach\nattach attach attach attach\n"),
                    ("src/lib.rs", "fn attach_handler() {\n    // attach handler\n}\n"),
                ],
                build_index: true,
                context: GatherOutput {
                    schema_version: SCHEMA_VERSION.to_string(),
                    repo_root: String::new(),
                    summary: "summary".to_string(),
                    explicit_files: vec![],
                    missing_explicit_files: Vec::new(),
                    skipped_explicit_files: Vec::new(),
                    search_areas: vec!["src/".to_string(), "docs/".to_string()],
                    query: vec!["Where should attach handler live?".to_string()],
                    terms: vec!["attach".to_string(), "handler".to_string()],
                    blockers: Vec::new(),
                },
                expected_missing: vec![],
                expected_skipped: vec![],
                expected_index_state: "usable",
                expected_retrieval_mode: "ranked_only",
                expected_ranked_files: vec!["src/lib.rs", "docs/guide.md"],
            },
        ];

        for case in &cases {
            let repo = temp_repo(case.name);
            for (path, content) in &case.files {
                write_file(&repo.join(path), content);
            }
            if case.build_index {
                index::build_chunk_index(&repo).expect("build index");
            }

            let mut gather = case.context.clone();
            gather.repo_root = repo.to_string_lossy().into_owned();
            let (_report, trace) = output::create_test_outputs(gather).expect("run for test");

            assert_eq!(
                trace.missing_explicit_files,
                case.expected_missing
                    .iter()
                    .map(|item| item.to_string())
                    .collect::<Vec<_>>(),
                "case: {}",
                case.name
            );
            assert_eq!(
                trace.skipped_explicit_files,
                case.expected_skipped
                    .iter()
                    .map(|item| item.to_string())
                    .collect::<Vec<_>>(),
                "case: {}",
                case.name
            );
            assert_eq!(trace.index_state, case.expected_index_state, "case: {}", case.name);
            assert_eq!(
                trace.queries[0].retrieval_mode,
                case.expected_retrieval_mode,
                "case: {}",
                case.name
            );
            assert_eq!(
                trace.queries[0].ranked_files,
                case.expected_ranked_files
                    .iter()
                    .map(|item| item.to_string())
                    .collect::<Vec<_>>(),
                "case: {}",
                case.name
            );

            let _ = fs::remove_dir_all(repo);
        }
    }
}
