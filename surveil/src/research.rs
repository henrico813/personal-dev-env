use crate::index;
use crate::schema::{Answer, ExplicitFile, Finding, GatherOutput, ResearchOutput, TraceOutput, SCHEMA_VERSION};
use crate::source;
use std::collections::{BTreeSet, HashSet};
use std::error::Error;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use tree_sitter::Parser;

pub fn run(context: &Path, trace_out: &Path) -> Result<(), Box<dyn Error>> {
    let context_text = fs::read_to_string(context)?;
    let gather: GatherOutput = serde_json::from_str(&context_text)?;

    let repo_root = Path::new(&gather.repo_root).to_path_buf();
    let mut trace = TraceState::default();
    let loaded_files = load_candidate_files(
        &repo_root,
        &gather.search_areas,
        &gather.explicit_files,
        &mut trace,
    )?;
    let corpus = build_corpus(loaded_files);
    let mut result = Vec::with_capacity(gather.query.len());

    for query in &gather.query {
        let (findings, negative_evidence) =
            answer_question_from_corpus(query, &gather.terms, &gather.search_areas, &corpus, &mut trace)?;
        if findings.is_empty() {
            trace.unmatched_questions.push(query.clone());
        }
        result.push(Answer {
            query: query.clone(),
            findings,
            negative_evidence,
        });
    }

    let open_questions = trace.unmatched_questions.clone();
    let report = ResearchOutput {
        schema_version: SCHEMA_VERSION.to_string(),
        summary: gather.summary,
        result,
        blockers: gather.blockers,
        open_questions,
    };

    dedupe_in_place(&mut trace.skipped_paths);
    let trace_output = TraceOutput {
        schema_version: SCHEMA_VERSION.to_string(),
        searched_areas: gather.search_areas,
        skipped_paths: trace.skipped_paths,
        files_considered: trace.files_considered.len(),
        files_matched: trace.files_matched.len(),
        unmatched_questions: trace.unmatched_questions,
    };

    if let Some(parent) = trace_out.parent() {
        fs::create_dir_all(parent)?;
    }
    let trace_file = fs::File::create(trace_out)?;
    serde_json::to_writer(trace_file, &trace_output)?;

    let stdout = io::stdout();
    let mut handle = stdout.lock();
    serde_json::to_writer(&mut handle, &report)?;
    handle.write_all(b"\n")?;
    Ok(())
}

#[derive(Default)]
struct TraceState {
    files_considered: BTreeSet<PathBuf>,
    files_matched: BTreeSet<PathBuf>,
    skipped_paths: Vec<String>,
    unmatched_questions: Vec<String>,
}

const MAX_FINDINGS_PER_FILE: usize = 3;

struct RankedFileFindings {
    display_path: String,
    explicit: bool,
    findings: Vec<Finding>,
}

struct LoadedFile {
    path: PathBuf,
    display_path: String,
    explicit: bool,
    text: String,
}

struct CorpusFile {
    path: PathBuf,
    display_path: String,
    explicit: bool,
    text: String,
    lines: Vec<CorpusLine>,
}

struct CorpusLine {
    number: u32,
    start: usize,
    end: usize,
    lower_text: String,
}

fn answer_question_from_corpus(
    question: &str,
    terms: &[String],
    search_areas: &[String],
    corpus: &[CorpusFile],
    trace: &mut TraceState,
) -> Result<(Vec<Finding>, Vec<String>), Box<dyn Error>> {
    let tokens = search_tokens(terms, question);
    let mut ranked_files = Vec::new();

    for file in corpus {
        let mut file_findings = Vec::new();
        for line in &file.lines {
            let line_text = &file.text[line.start..line.end];
            if let Some(matched_from) = tokens.iter().find(|token| line.lower_text.contains(token.as_str())) {
                if let Some(match_offset) = case_insensitive_byte_offset(line_text, matched_from) {
                    file_findings.push(MatchedFinding {
                        finding: Finding {
                            path: file.display_path.clone(),
                            line: line.number,
                            excerpt: line_text.trim().to_string(),
                            source: if file.explicit {
                                "explicit_file".to_string()
                            } else {
                                "lexical".to_string()
                            },
                            matched_from: matched_from.clone(),
                            symbol_kind: None,
                            symbol_name: None,
                            symbol_start_line: None,
                            symbol_end_line: None,
                        },
                        byte_offset: line.start + match_offset,
                    });
                }
            }
        }

        if !file_findings.is_empty() {
            if should_enrich_symbol_metadata(&file.path) {
                enrich_symbol_metadata(&file.text, &mut file_findings);
            }
            trace.files_matched.insert(file.path.clone());
            ranked_files.push(RankedFileFindings {
                display_path: file.display_path.clone(),
                explicit: file.explicit,
                findings: file_findings.into_iter().map(|hit| hit.finding).collect(),
            });
        }
    }

    ranked_files.sort_by(|a, b| {
        b.explicit
            .cmp(&a.explicit)
            .then_with(|| b.findings.len().cmp(&a.findings.len()))
            .then_with(|| a.display_path.cmp(&b.display_path))
    });

    let findings: Vec<Finding> = ranked_files
        .into_iter()
        .flat_map(|mut file| {
            file.findings.sort_by_key(|finding| finding.line);
            file.findings.truncate(MAX_FINDINGS_PER_FILE);
            file.findings
        })
        .collect();

    let negative_evidence = if findings.is_empty() {
        vec![format!("searched declared areas: {}", search_areas.join(", "))]
    } else {
        Vec::new()
    };

    Ok((findings, negative_evidence))
}

fn load_candidate_text(
    repo_root: &Path,
    file: &Path,
    cache: Option<&rusqlite::Connection>,
    trace: &mut TraceState,
) -> Option<String> {
    if let Some(cache) = cache {
        if let Ok(Some(cached)) = index::load_text(cache, repo_root, file) {
            if index::is_fresh(file, &cached).ok() == Some(true) {
                return Some(cached.text);
            }
        }
    }

    match fs::read_to_string(file) {
        Ok(text) => Some(text),
        Err(_) => {
            trace.skipped_paths.push(source::display_path(repo_root, file));
            None
        }
    }
}

fn load_candidate_files(
    repo_root: &Path,
    search_areas: &[String],
    explicit_files: &[ExplicitFile],
    trace: &mut TraceState,
) -> Result<Vec<LoadedFile>, Box<dyn Error>> {
    let candidates = source::collect_candidate_files(repo_root, search_areas, explicit_files, &mut trace.skipped_paths)?;
    let cache = index::open(repo_root).ok().flatten();
    let mut loaded_files = Vec::with_capacity(candidates.len());

    for candidate in candidates {
        trace.files_considered.insert(candidate.path().to_path_buf());
        let text = match load_candidate_text(repo_root, candidate.path(), cache.as_ref(), trace) {
            Some(text) => text,
            None => continue,
        };

        loaded_files.push(LoadedFile {
            display_path: candidate.display_path().to_string(),
            path: candidate.path().to_path_buf(),
            explicit: candidate.is_explicit(),
            text,
        });
    }

    Ok(loaded_files)
}

fn build_corpus(loaded_files: Vec<LoadedFile>) -> Vec<CorpusFile> {
    loaded_files
        .into_iter()
        .map(|file| CorpusFile {
            lines: prepare_lines(&file.text),
            path: file.path,
            display_path: file.display_path,
            explicit: file.explicit,
            text: file.text,
        })
        .collect()
}

fn prepare_lines(text: &str) -> Vec<CorpusLine> {
    let mut lines = Vec::new();
    let mut line_start = 0usize;
    let mut line_number = 1u32;

    while line_start <= text.len() {
        let line_end = text[line_start..]
            .find('\n')
            .map(|offset| line_start + offset)
            .unwrap_or(text.len());
        let mut content_end = line_end;
        if content_end > line_start && text.as_bytes()[content_end - 1] == b'\r' {
            content_end -= 1;
        }

        lines.push(CorpusLine {
            number: line_number,
            start: line_start,
            end: content_end,
            lower_text: text[line_start..content_end].to_lowercase(),
        });

        if line_end == text.len() {
            break;
        }
        line_start = line_end + 1;
        line_number += 1;
    }

    lines
}

#[cfg(test)]
fn answer_question(
    repo_root: &Path,
    question: &str,
    terms: &[String],
    search_areas: &[String],
    explicit_files: &[ExplicitFile],
    trace: &mut TraceState,
) -> Result<(Vec<Finding>, Vec<String>), Box<dyn Error>> {
    let loaded_files = load_candidate_files(repo_root, search_areas, explicit_files, trace)?;
    let corpus = build_corpus(loaded_files);
    answer_question_from_corpus(question, terms, search_areas, &corpus, trace)
}

#[derive(Debug)]
struct MatchedFinding {
    finding: Finding,
    byte_offset: usize,
}

fn should_enrich_symbol_metadata(path: &Path) -> bool {
    if path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with('.'))
    {
        return false;
    }

    if path.extension().is_none() {
        return false;
    }

    let extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase());

    if matches!(
        extension.as_deref(),
        Some(
            "md" | "markdown" | "rst" | "txt" | "toml" | "json" | "yaml" | "yml" | "ini"
                | "cfg" | "conf" | "env"
        )
    ) {
        return false;
    }

    if path.components().any(|component| {
        let component = component.as_os_str().to_string_lossy();
        matches!(
            component.as_ref(),
            "docs" | "doc" | "config" | "configs" | "configuration"
        )
    }) {
        return false;
    }

    true
}

fn parse_tree(text: &str, language: tree_sitter::Language) -> Option<tree_sitter::Tree> {
    let mut parser = Parser::new();
    parser.set_language(&language).ok()?;
    parser.parse(text, None)
}

fn enrich_symbol_metadata(text: &str, findings: &mut [MatchedFinding]) {
    for language in [
        tree_sitter_rust::LANGUAGE.into(),
        tree_sitter_go::LANGUAGE.into(),
        tree_sitter_python::LANGUAGE.into(),
        tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        tree_sitter_typescript::LANGUAGE_TSX.into(),
    ] {
        if let Some(tree) = parse_tree(text, language) {
            if tree.root_node().has_error() {
                continue;
            }
            if attach_symbol_metadata(text, &tree, findings) {
                return;
            }
        }
    }
}

fn attach_symbol_metadata(text: &str, tree: &tree_sitter::Tree, findings: &mut [MatchedFinding]) -> bool {
    let mut attached = false;
    for hit in findings {
        if let Some(symbol) = enclosing_symbol(tree.root_node(), text.as_bytes(), hit.byte_offset) {
            hit.finding.symbol_kind = Some(symbol.kind);
            hit.finding.symbol_name = Some(symbol.name);
            hit.finding.symbol_start_line = Some(symbol.start_line);
            hit.finding.symbol_end_line = Some(symbol.end_line);
            attached = true;
        }
    }
    attached
}

struct SymbolInfo {
    kind: String,
    name: String,
    start_line: u32,
    end_line: u32,
}

fn enclosing_symbol(
    root: tree_sitter::Node,
    source: &[u8],
    byte_offset: usize,
) -> Option<SymbolInfo> {
    let mut node = root.descendant_for_byte_range(byte_offset, byte_offset.saturating_add(1).min(source.len()))?;
    loop {
        if is_symbol_node(node) {
            let name_node = symbol_name_node(node)?;
            let name = name_node.utf8_text(source).ok()?.to_string();
            return Some(SymbolInfo {
                kind: normalized_symbol_kind(node.kind()).to_string(),
                name,
                start_line: node.start_position().row as u32 + 1,
                end_line: node.end_position().row as u32 + 1,
            });
        }
        node = node.parent()?;
    }
}

fn symbol_name_node(node: tree_sitter::Node) -> Option<tree_sitter::Node> {
    node.child_by_field_name("name").or_else(|| node.named_child(0))
}

fn is_symbol_node(node: tree_sitter::Node) -> bool {
    let kind = node.kind();
    node.child_by_field_name("name").is_some()
        && (kind.ends_with("_item")
            || kind.ends_with("_declaration")
            || kind.ends_with("_definition")
            || kind.ends_with("_declarator"))
}

fn normalized_symbol_kind(kind: &str) -> &'static str {
    if kind.contains("function") {
        "function"
    } else if kind.contains("method") {
        "method"
    } else if kind.contains("class")
        || kind.contains("struct")
        || kind.contains("enum")
        || kind.contains("trait")
        || kind.contains("interface")
        || kind.contains("type")
    {
        "type"
    } else if kind.contains("module") || kind.contains("mod") || kind.contains("package") {
        "module"
    } else {
        "symbol"
    }
}

fn dedupe_in_place(values: &mut Vec<String>) {
    let mut seen = HashSet::new();
    values.retain(|value| seen.insert(value.clone()));
}

fn case_insensitive_byte_offset(line: &str, needle: &str) -> Option<usize> {
    let line_bytes = line.as_bytes();
    let needle_bytes = needle.as_bytes();
    if needle_bytes.is_empty() || needle_bytes.len() > line_bytes.len() {
        return None;
    }

    line_bytes.windows(needle_bytes.len()).position(|window| {
        window
            .iter()
            .zip(needle_bytes.iter())
            .all(|(a, b)| a.to_ascii_lowercase() == b.to_ascii_lowercase())
    })
}

fn search_tokens(terms: &[String], question: &str) -> Vec<String> {
    let question_tokens = question_tokens(question);
    let question_token_set: HashSet<String> = question_tokens
        .iter()
        .flat_map(|token| token_variants(token))
        .collect();

    let matching_terms: Vec<String> = terms
        .iter()
        .filter_map(|term| {
            let token = term.trim().to_lowercase();
            if token.is_empty() {
                return None;
            }
            let variants = token_variants(&token);
            if variants.iter().any(|variant| question_token_set.contains(variant)) {
                Some(token)
            } else {
                None
            }
        })
        .collect();

    let source = if matching_terms.is_empty() {
        question_tokens
    } else {
        matching_terms
    };

    let mut tokens = Vec::new();
    for token in source {
        push_token(&mut tokens, &token);
        if token.contains('-') {
            push_token(&mut tokens, &token.replace('-', "_"));
        } else if token.contains('_') {
            push_token(&mut tokens, &token.replace('_', "-"));
        }
    }

    tokens
}

fn question_tokens(question: &str) -> Vec<String> {
    question
        .split_whitespace()
        .filter_map(|raw| {
            let token = raw
                .trim_matches(|ch: char| !ch.is_ascii_alphanumeric() && ch != '-' && ch != '_')
                .to_lowercase();
            if token.is_empty() || is_generic_question_token(&token) {
                None
            } else {
                Some(token)
            }
        })
        .collect()
}

fn token_variants(token: &str) -> Vec<String> {
    let mut variants = vec![token.to_string()];

    if token.contains('-') {
        let variant = token.replace('-', "_");
        if variant != token {
            variants.push(variant);
        }
    } else if token.contains('_') {
        let variant = token.replace('_', "-");
        if variant != token {
            variants.push(variant);
        }
    }

    variants
}

fn push_token(tokens: &mut Vec<String>, token: &str) {
    if token.len() < 3 {
        return;
    }
    if !tokens.iter().any(|existing| existing == token) {
        tokens.push(token.to_string());
    }
}

fn is_generic_question_token(token: &str) -> bool {
    matches!(
        token,
        "what" | "where" | "when" | "why" | "how" | "who" | "whom" | "which" | "whose"
            | "should" | "would" | "could" | "can" | "may" | "might" | "do" | "does"
            | "did" | "is" | "are" | "was" | "were" | "be" | "been" | "being" | "the"
            | "a" | "an" | "to" | "of" | "and" | "or" | "for" | "in" | "on" | "at"
            | "by" | "with" | "from" | "into" | "this" | "that" | "these" | "those"
    )
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

        index::run(&repo).expect("build index");

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
        index::run(&repo).expect("build index");
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
        index::run(&repo).expect("build index");

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
}
