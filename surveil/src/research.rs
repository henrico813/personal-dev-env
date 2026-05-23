use crate::schema::{ExplicitFile, Finding};
use crate::source::SourceFile;
use std::collections::{BTreeSet, HashMap};
use std::error::Error;
use std::path::{Path, PathBuf};

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

#[cfg(test)]
fn answer_question(
    repo_root: &Path,
    question: &str,
    terms: &[String],
    search_areas: &[String],
    explicit_files: &[ExplicitFile],
    trace: &mut TraceState,
) -> Result<(Vec<Finding>, Vec<String>), Box<dyn Error>> {
    let candidates = setup::collect_candidate_sources(repo_root, search_areas, explicit_files, trace)?;
    let mut live_cache = LiveFileCache::new();
    let tokens = tokenize::search_tokens(terms, question);
    let (ranked_scores, ordered_candidates) = rank::rank_query_candidates(repo_root, &candidates, &tokens)?;
    scan::answer_question_from_sources(
        repo_root,
        search_areas,
        &ordered_candidates,
        &candidates,
        &tokens,
        &ranked_scores,
        &mut live_cache,
        trace,
    )
}

#[derive(Debug)]
pub(super) struct MatchedFinding {
    pub(super) finding: Finding,
    pub(super) byte_offset: usize,
}


#[cfg(test)]
mod tests {
    use super::{answer_question, run, TraceState};
    use crate::index;
    use crate::schema::{
        ExplicitFile, GatherOutput, ResearchOutput, TraceOutput, SCHEMA_VERSION,
    };
    use std::fs;
    use std::io::Write;
    use std::path::PathBuf;
    use std::process::Command;
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

    fn write_context(path: &PathBuf, context: &GatherOutput) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent dirs");
        }
        fs::write(path, serde_json::to_vec(context).expect("serialize context")).expect("write context");
    }

    fn parse_report_from_stdout(stdout: &str) -> ResearchOutput {
        let line = stdout
            .lines()
            .find(|line| line.trim_start().starts_with("{\"schema_version\""))
            .expect("report line");
        serde_json::from_str(line).expect("parse report")
    }

    #[test]
    fn skips_generated_repo_relative_output_and_prefers_declared_terms() {
        let repo = temp_repo("research");
        write_file(&repo.join("surveil/src/lib.rs"), "// tree-sitter attach\n");
        write_file(&repo.join("surveil/target/generated.rs"), "// tree-sitter attach\n");

        let mut trace = TraceState::default();
        let (findings, _) = answer_question(
            &repo,
            "Where should Tree-sitter attach?",
            &["tree-sitter".to_string()],
            &["surveil/".to_string()],
            &[],
            &mut trace,
        )
        .expect("research answer");

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].path, "surveil/src/lib.rs");
        assert_eq!(findings[0].matched_from, "tree-sitter");
        assert!(trace.skipped_paths.iter().any(|path| path.contains("surveil/target")));

        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn falls_back_to_question_tokens_when_declared_terms_do_not_match() {
        let repo = temp_repo("fallback");
        write_file(&repo.join("surveil/src/build.rs"), "// build only\n");
        write_file(&repo.join("surveil/src/lib.rs"), "// attach here\n");

        let mut trace = TraceState::default();
        let (findings, _) = answer_question(
            &repo,
            "Where should Tree-sitter attach?",
            &["build".to_string()],
            &["surveil/".to_string()],
            &[],
            &mut trace,
        )
        .expect("research answer");

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].path, "surveil/src/lib.rs");
        assert_eq!(findings[0].matched_from, "attach");

        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn indexed_mode_matches_scan_mode_for_scoped_results() {
        let repo = temp_repo("indexed-parity");
        write_file(&repo.join("notes/design.md"), "attach here\n");
        write_file(&repo.join("other/ignore.md"), "attach here too\n");

        let mut scan_trace = TraceState::default();
        let scan = answer_question(
            &repo,
            "Where should attach live?",
            &["attach".to_string()],
            &["notes/".to_string()],
            &[],
            &mut scan_trace,
        )
        .expect("scan result");

        index::build_chunk_index(&repo).expect("build index");

        let mut indexed_trace = TraceState::default();
        let indexed = answer_question(
            &repo,
            "Where should attach live?",
            &["attach".to_string()],
            &["notes/".to_string()],
            &[],
            &mut indexed_trace,
        )
        .expect("indexed result");

        assert_eq!(scan.0, indexed.0);
        assert_eq!(scan.1, indexed.1);

        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn stale_index_falls_back_to_current_file_text() {
        let repo = temp_repo("stale-index");
        write_file(&repo.join("notes/design.md"), "old text\n");
        index::build_chunk_index(&repo).expect("build index");
        write_file(&repo.join("notes/design.md"), "attach here\n");

        let mut trace = TraceState::default();
        let (findings, negative_evidence) = answer_question(
            &repo,
            "Where should attach live?",
            &["attach".to_string()],
            &["notes/".to_string()],
            &[],
            &mut trace,
        )
        .expect("fallback result");

        assert!(negative_evidence.is_empty());
        assert_eq!(findings[0].path, "notes/design.md");
        assert_eq!(findings[0].excerpt, "attach here");

        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn indexed_mode_preserves_crlf_symbol_attachment() {
        let repo = temp_repo("indexed-crlf-symbols");
        write_file(
            &repo.join("surveil/src/lib.rs"),
            "fn attach() {\r\n    let cafe\u{0301} = 1; // tree-sitter attach\r\n}\r\n",
        );
        index::build_chunk_index(&repo).expect("build index");

        let mut trace = TraceState::default();
        let (findings, _) = answer_question(
            &repo,
            "Where should Tree-sitter attach?",
            &["tree-sitter".to_string()],
            &["surveil/".to_string()],
            &[],
            &mut trace,
        )
        .expect("indexed result");

        assert_eq!(findings[0].symbol_kind.as_deref(), Some("function"));
        assert_eq!(findings[0].symbol_name.as_deref(), Some("attach"));
        assert_eq!(findings[0].symbol_start_line, Some(1));
        assert_eq!(findings[0].symbol_end_line, Some(3));

        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn searches_repo_relative_area_inside_worktree() {
        let repo = temp_repo("worktrees").join("worktrees/repo");
        fs::create_dir_all(&repo).expect("create worktree repo");
        write_file(&repo.join("surveil/src/lib.rs"), "// tree-sitter attach\n");

        let mut trace = TraceState::default();
        let (findings, _) = answer_question(
            &repo,
            "Where should Tree-sitter attach?",
            &["tree-sitter".to_string()],
            &["surveil/".to_string()],
            &[],
            &mut trace,
        )
        .expect("research answer");

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].path, "surveil/src/lib.rs");
        assert!(trace.files_considered.len() > 0);
        assert!(trace.skipped_paths.iter().all(|path| !path.contains("worktrees/repo/surveil")));

        let _ = fs::remove_dir_all(repo.parent().expect("parent"));
    }

    #[test]
    fn includes_explicit_files_outside_search_areas() {
        let repo = temp_repo("explicit");
        write_file(&repo.join("notes/design.md"), "// tree-sitter attach\n");
        write_file(&repo.join("surveil/src/lib.rs"), "// unrelated\n");

        let mut trace = TraceState::default();
        let (findings, _) = answer_question(
            &repo,
            "Where should Tree-sitter attach?",
            &["tree-sitter".to_string()],
            &["surveil/".to_string()],
            &[ExplicitFile {
                path: "notes/design.md".to_string(),
                found: true,
            }],
            &mut trace,
        )
        .expect("research answer");

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].path, "notes/design.md");
        assert_eq!(findings[0].source, "explicit_file");

        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn ranks_explicit_files_ahead_of_broad_lexical_hits() {
        let repo = temp_repo("ranking");
        write_file(
            &repo.join("notes/design.md"),
            "// tree-sitter attach\n",
        );
        write_file(
            &repo.join("surveil/src/lib.rs"),
            "// tree-sitter attach one\n// tree-sitter attach two\n// tree-sitter attach three\n// tree-sitter attach four\n",
        );

        let mut trace = TraceState::default();
        let (findings, _) = answer_question(
            &repo,
            "Where should Tree-sitter attach?",
            &["tree-sitter".to_string()],
            &["surveil/".to_string()],
            &[ExplicitFile {
                path: "notes/design.md".to_string(),
                found: true,
            }],
            &mut trace,
        )
        .expect("research answer");

        assert_eq!(findings[0].path, "notes/design.md");
        assert_eq!(findings[0].source, "explicit_file");
        assert!(findings.iter().any(|finding| finding.path == "surveil/src/lib.rs"));

        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn indexed_mode_preserves_explicit_ordering_and_trace_counts() {
        let repo = temp_repo("indexed-explicit-order");
        write_file(&repo.join("notes/design.md"), "// tree-sitter attach\n");
        write_file(
            &repo.join("surveil/src/a.rs"),
            "// tree-sitter attach one\n// tree-sitter attach two\n",
        );
        write_file(
            &repo.join("surveil/src/b.rs"),
            "// tree-sitter attach one\n// tree-sitter attach two\n",
        );

        let explicit = [ExplicitFile {
            path: "notes/design.md".to_string(),
            found: true,
        }];

        let mut scan_trace = TraceState::default();
        let scan = answer_question(
            &repo,
            "Where should Tree-sitter attach?",
            &["tree-sitter".to_string()],
            &["surveil/".to_string()],
            &explicit,
            &mut scan_trace,
        )
        .expect("scan result");

        index::build_chunk_index(&repo).expect("build index");

        let mut indexed_trace = TraceState::default();
        let indexed = answer_question(
            &repo,
            "Where should Tree-sitter attach?",
            &["tree-sitter".to_string()],
            &["surveil/".to_string()],
            &explicit,
            &mut indexed_trace,
        )
        .expect("indexed result");

        assert_eq!(scan.0, indexed.0);
        assert_eq!(scan.1, indexed.1);
        assert_eq!(indexed.0[0].path, "notes/design.md");
        assert_eq!(indexed.0[1].path, "surveil/src/a.rs");
        assert_eq!(indexed_trace.files_considered.len(), scan_trace.files_considered.len());
        assert_eq!(indexed_trace.files_matched.len(), scan_trace.files_matched.len());

        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn caps_findings_per_file_at_three() {
        let repo = temp_repo("caps");
        write_file(
            &repo.join("surveil/src/lib.rs"),
            "// tree-sitter attach one\n// tree-sitter attach two\n// tree-sitter attach three\n// tree-sitter attach four\n// tree-sitter attach five\n",
        );

        let mut trace = TraceState::default();
        let (findings, _) = answer_question(
            &repo,
            "Where should Tree-sitter attach?",
            &["tree-sitter".to_string()],
            &["surveil/".to_string()],
            &[],
            &mut trace,
        )
        .expect("research answer");

        assert_eq!(findings.len(), 3);
        assert_eq!(findings.iter().filter(|finding| finding.path == "surveil/src/lib.rs").count(), 3);

        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn skips_non_utf8_files_and_records_path() {
        let repo = temp_repo("non-utf8");
        let path = repo.join("surveil/src/lib.rs");
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent dirs");
        }
        fs::write(&path, &[0xff_u8, 0xfe_u8, 0xfd_u8]).expect("write invalid utf8");

        let mut trace = TraceState::default();
        let (findings, _) = answer_question(
            &repo,
            "Where should Tree-sitter attach?",
            &["tree-sitter".to_string()],
            &["surveil/".to_string()],
            &[],
            &mut trace,
        )
        .expect("research answer");

        assert!(findings.is_empty());
        assert!(trace.skipped_paths.iter().any(|path| path == "surveil/src/lib.rs"));

        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn preserves_token_priority_when_multiple_terms_match_same_line() {
        let repo = temp_repo("token-priority");
        write_file(&repo.join("surveil/src/lib.rs"), "// tree-sitter attach\n");

        let mut trace = TraceState::default();
        let (findings, _) = answer_question(
            &repo,
            "Where should Tree-sitter attach?",
            &["attach".to_string(), "tree-sitter".to_string()],
            &["surveil/".to_string()],
            &[],
            &mut trace,
        )
        .expect("research answer");

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].matched_from, "attach");

        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn parseable_markdown_docs_remain_lexical_only() {
        let repo = temp_repo("markdown-symbols");
        write_file(&repo.join("docs/notes.md"), "fn attach() { // tree-sitter attach }\n");

        let mut trace = TraceState::default();
        let (findings, _) = answer_question(
            &repo,
            "Where should Tree-sitter attach?",
            &["tree-sitter".to_string()],
            &["docs/".to_string()],
            &[],
            &mut trace,
        )
        .expect("research answer");

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].path, "docs/notes.md");
        assert_eq!(findings[0].symbol_kind, None);
        assert_eq!(findings[0].symbol_name, None);
        assert_eq!(findings[0].symbol_start_line, None);
        assert_eq!(findings[0].symbol_end_line, None);

        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn parseable_root_readme_remains_lexical_only() {
        let repo = temp_repo("readme-symbols");
        write_file(&repo.join("README"), "fn attach() { // tree-sitter attach }\n");

        let mut trace = TraceState::default();
        let (findings, _) = answer_question(
            &repo,
            "Where should Tree-sitter attach?",
            &["tree-sitter".to_string()],
            &[".".to_string()],
            &[],
            &mut trace,
        )
        .expect("research answer");

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].path, "README");
        assert_eq!(findings[0].symbol_kind, None);
        assert_eq!(findings[0].symbol_name, None);
        assert_eq!(findings[0].symbol_start_line, None);
        assert_eq!(findings[0].symbol_end_line, None);

        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn parseable_root_env_dotfiles_remain_lexical_only() {
        let repo = temp_repo("env-symbols");
        write_file(&repo.join(".env"), "fn attach() { // tree-sitter attach }\n");
        write_file(&repo.join(".env.local"), "fn attach() { // tree-sitter attach }\n");

        let mut trace = TraceState::default();
        let (findings, _) = answer_question(
            &repo,
            "Where should Tree-sitter attach?",
            &["tree-sitter".to_string()],
            &[".".to_string()],
            &[],
            &mut trace,
        )
        .expect("research answer");

        assert_eq!(findings.len(), 2);
        for path in [".env", ".env.local"] {
            let finding = findings.iter().find(|finding| finding.path == path).expect("finding");
            assert_eq!(finding.symbol_kind, None);
            assert_eq!(finding.symbol_name, None);
            assert_eq!(finding.symbol_start_line, None);
            assert_eq!(finding.symbol_end_line, None);
        }

        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn parseable_extensionless_source_file_remains_lexical_only() {
        let repo = temp_repo("extensionless-source");
        write_file(&repo.join("surveil/src/lib"), "fn attach() { // tree-sitter attach }\n");

        let mut trace = TraceState::default();
        let (findings, _) = answer_question(
            &repo,
            "Where should Tree-sitter attach?",
            &["tree-sitter".to_string()],
            &["surveil/".to_string()],
            &[],
            &mut trace,
        )
        .expect("research answer");

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].path, "surveil/src/lib");
        assert_eq!(findings[0].symbol_kind, None);
        assert_eq!(findings[0].symbol_name, None);
        assert_eq!(findings[0].symbol_start_line, None);
        assert_eq!(findings[0].symbol_end_line, None);

        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn parse_error_fallback_leaves_symbol_fields_empty() {
        let repo = temp_repo("parse-error");
        write_file(
            &repo.join("surveil/src/lib.rs"),
            "fn attach( {\r\n    // tree-sitter attach\r\n}\r\n",
        );

        let mut trace = TraceState::default();
        let (findings, _) = answer_question(
            &repo,
            "Where should Tree-sitter attach?",
            &["tree-sitter".to_string()],
            &["surveil/".to_string()],
            &[],
            &mut trace,
        )
        .expect("research answer");

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].symbol_kind, None);
        assert_eq!(findings[0].symbol_name, None);
        assert_eq!(findings[0].symbol_start_line, None);
        assert_eq!(findings[0].symbol_end_line, None);

        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn crlf_source_with_combining_character_attaches_correct_enclosing_symbol() {
        let repo = temp_repo("crlf-symbols");
        write_file(
            &repo.join("surveil/src/lib.rs"),
            "fn attach() {\r\n    let cafe\u{0301} = 1; // tree-sitter attach\r\n}\r\n",
        );

        let mut trace = TraceState::default();
        let (findings, _) = answer_question(
            &repo,
            "Where should Tree-sitter attach?",
            &["tree-sitter".to_string()],
            &["surveil/".to_string()],
            &[],
            &mut trace,
        )
        .expect("research answer");

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].symbol_kind.as_deref(), Some("function"));
        assert_eq!(findings[0].symbol_name.as_deref(), Some("attach"));
        assert_eq!(findings[0].symbol_start_line, Some(1));
        assert_eq!(findings[0].symbol_end_line, Some(3));

        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn run_dedupes_skipped_paths() {
        let repo = temp_repo("single-pass-trace");
        fs::create_dir_all(repo.join("surveil")).expect("create search area");
        let context = repo.join("context.json");
        let trace = repo.join("trace.json");
        write_context(
            &context,
            &GatherOutput {
                schema_version: SCHEMA_VERSION.to_string(),
                repo_root: repo.to_string_lossy().into_owned(),
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
        );

        run(&context, &trace).expect("research run");

        let trace_output: TraceOutput = serde_json::from_str(
            &fs::read_to_string(&trace).expect("read trace"),
        )
        .expect("parse trace");
        assert_eq!(trace_output.skipped_paths, vec![".surveil/index.sqlite".to_string()]);

        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn run_keeps_multi_query_parity_child() {
        let repo = temp_repo("single-pass-parity");
        write_file(&repo.join("notes/design.md"), "// tree-sitter attach\n");
        write_file(
            &repo.join("surveil/src/lib.rs"),
            "fn attach() {\r\n    // tree-sitter attach one\r\n    // tree-sitter attach two\r\n    // tree-sitter attach three\r\n    // tree-sitter attach four\r\n}\r\n",
        );

        let context = repo.join("context.json");
        let trace = repo.join("trace.json");
        write_context(
            &context,
            &GatherOutput {
                schema_version: SCHEMA_VERSION.to_string(),
                repo_root: repo.to_string_lossy().into_owned(),
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
                terms: vec!["tree-sitter".to_string(), "attach".to_string(), "missing".to_string()],
                blockers: Vec::new(),
            },
        );

        run(&context, &trace).expect("research run");

        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn run_writes_schema_version_child() {
        let repo = temp_repo("run-version-child");
        write_file(&repo.join("surveil/src/lib.rs"), "// tree-sitter attach\n");
        let context = repo.join("context.json");
        let trace = repo.join("trace.json");
        write_context(
            &context,
            &GatherOutput {
                schema_version: SCHEMA_VERSION.to_string(),
                repo_root: repo.to_string_lossy().into_owned(),
                summary: "summary".to_string(),
                explicit_files: Vec::new(),
                search_areas: vec!["surveil/".to_string()],
                query: vec!["Where should Tree-sitter attach?".to_string()],
                terms: vec!["tree-sitter".to_string()],
                blockers: Vec::new(),
            },
        );

        run(&context, &trace).expect("research run");

        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn run_keeps_multi_query_parity() {
        let output = Command::new(std::env::current_exe().expect("current exe"))
            .arg("run_keeps_multi_query_parity_child")
            .arg("--nocapture")
            .output()
            .expect("spawn test binary");
        assert!(output.status.success(), "child test failed: {}", String::from_utf8_lossy(&output.stderr));

        let report = parse_report_from_stdout(&String::from_utf8(output.stdout).expect("utf8 stdout"));

        assert_eq!(report.result.len(), 3);
        for answer in &report.result[..2] {
            assert_eq!(answer.findings[0].path, "notes/design.md");
            assert_eq!(answer.findings[0].source, "explicit_file");
            assert!(answer.findings.iter().any(|finding| finding.path == "surveil/src/lib.rs"));
            assert!(answer.negative_evidence.is_empty());
        }

        let rust_findings: Vec<_> = report.result[0]
            .findings
            .iter()
            .filter(|finding| finding.path == "surveil/src/lib.rs")
            .collect();
        assert_eq!(rust_findings.len(), 3);
        assert_eq!(
            rust_findings.iter().map(|finding| finding.excerpt.as_str()).collect::<Vec<_>>(),
            vec!["fn attach() {", "// tree-sitter attach one", "// tree-sitter attach two"]
        );
        for finding in &rust_findings {
            assert_eq!(finding.symbol_kind.as_deref(), Some("function"));
            assert_eq!(finding.symbol_name.as_deref(), Some("attach"));
            assert_eq!(finding.symbol_start_line, Some(1));
            assert_eq!(finding.symbol_end_line, Some(6));
        }

        assert!(report.result[2].findings.is_empty());
        assert_eq!(
            report.result[2].negative_evidence,
            vec!["searched declared areas: surveil/".to_string()]
        );
        assert_eq!(report.open_questions, vec!["Where should missing live?".to_string()]);
    }

    #[test]
    fn versioned_run_output_writes_schema_version_and_not_surveil_version() {
        let output = Command::new(std::env::current_exe().expect("current exe"))
            .arg("run_writes_schema_version_child")
            .arg("--nocapture")
            .output()
            .expect("spawn test binary");
        assert!(output.status.success(), "child test failed: {}", String::from_utf8_lossy(&output.stderr));
        let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");
        assert!(stdout.contains("\"schema_version\":\"surveil.v5\""), "stdout: {stdout}");
        assert!(!stdout.contains("\"surveil_version\""), "stdout: {stdout}");
    }

    #[test]
    fn ranked_query_prefers_code_chunk_before_long_docs() {
        let repo = temp_repo("ranked-code");
        write_file(
            &repo.join("docs/guide.md"),
            "attach attach attach attach\nattach attach attach attach\n",
        );
        write_file(
            &repo.join("src/lib.rs"),
            "fn attach_handler() {\n    // attach handler\n}\n",
        );
        index::build_chunk_index(&repo).expect("build index");

        let mut trace = TraceState::default();
        let (findings, _) = answer_question(
            &repo,
            "Where should attach handler live?",
            &["attach".to_string(), "handler".to_string()],
            &["src/".to_string(), "docs/".to_string()],
            &[],
            &mut trace,
        )
        .expect("research answer");

        assert_eq!(findings[0].path, "src/lib.rs");

        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn ranked_query_prefers_config_chunk_before_docs() {
        let repo = temp_repo("ranked-config");
        write_file(&repo.join("docs/config.md"), "server port server port\n");
        write_file(
            &repo.join("config/settings.toml"),
            "[server]\nport = 8080\n",
        );
        index::build_chunk_index(&repo).expect("build index");

        let mut trace = TraceState::default();
        let (findings, _) = answer_question(
            &repo,
            "Where is server port configured?",
            &["server".to_string(), "port".to_string()],
            &["config/".to_string(), "docs/".to_string()],
            &[],
            &mut trace,
        )
        .expect("research answer");

        assert_eq!(findings[0].path, "config/settings.toml");

        let _ = fs::remove_dir_all(repo);
    }
}
