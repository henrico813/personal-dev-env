use super::rank::compare_best_chunk_score;
use super::{
    CorpusLine, LiveFileCache, LoadedFile, MatchedFinding, RankedFileFindings,
    TraceState, MAX_FINDINGS_PER_FILE,
};
use crate::schema::Finding;
use crate::source::{self, SourceFile};
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use tree_sitter::Parser;

pub(super) fn answer_question_from_sources(
    repo_root: &Path,
    search_areas: &[String],
    ordered_candidates: &[SourceFile],
    all_candidates: &[SourceFile],
    tokens: &[String],
    ranked_scores: &HashMap<PathBuf, f32>,
    live_cache: &mut LiveFileCache,
    trace: &mut TraceState,
) -> Result<(Vec<Finding>, Vec<String>), Box<dyn Error>> {
    let mut ranked_files = Vec::new();
    let mut loaded_for_query = HashSet::new();

    for source in ordered_candidates {
        loaded_for_query.insert(source.path().to_path_buf());
        if let Some(file_findings) = collect_file_findings(
            repo_root,
            source,
            tokens,
            ranked_scores.get(source.path()).copied(),
            live_cache,
            trace,
        ) {
            ranked_files.push(file_findings);
        }
    }

    if ranked_files.is_empty() {
        for source in all_candidates {
            if loaded_for_query.contains(source.path()) {
                continue;
            }
            if let Some(file_findings) = collect_file_findings(
                repo_root,
                source,
                tokens,
                ranked_scores.get(source.path()).copied(),
                live_cache,
                trace,
            ) {
                ranked_files.push(file_findings);
            }
        }
    }

    ranked_files.sort_by(|a, b| {
        b.explicit
            .cmp(&a.explicit)
            .then_with(|| compare_best_chunk_score(b.best_chunk_score, a.best_chunk_score))
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

fn collect_file_findings(
    repo_root: &Path,
    source: &SourceFile,
    tokens: &[String],
    best_chunk_score: Option<f32>,
    live_cache: &mut LiveFileCache,
    trace: &mut TraceState,
) -> Option<RankedFileFindings> {
    let file = load_live_file(repo_root, source, live_cache, trace)?;
    let mut file_findings = Vec::new();

    for line in &file.lines {
        let line_text = &file.text[line.start..line.end];
        if let Some(matched_from) = tokens
            .iter()
            .find(|token| line.lower_text.contains(token.as_str()))
        {
            if let Some(match_offset) = case_insensitive_byte_offset(line_text, matched_from) {
                file_findings.push(MatchedFinding {
                    finding: Finding {
                        path: file.source.display_path().to_string(),
                        line: line.number,
                        excerpt: line_text.trim().to_string(),
                        source: if file.source.is_explicit() {
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

    if file_findings.is_empty() {
        return None;
    }

    if should_enrich_symbol_metadata(file.source.path()) {
        enrich_symbol_metadata(&file.text, &mut file_findings);
    }
    trace.files_matched.insert(file.source.path().to_path_buf());
    Some(RankedFileFindings {
        display_path: file.source.display_path().to_string(),
        explicit: file.source.is_explicit(),
        best_chunk_score,
        findings: file_findings.into_iter().map(|hit| hit.finding).collect(),
    })
}

fn load_live_file<'a>(
    repo_root: &Path,
    source: &SourceFile,
    live_cache: &'a mut LiveFileCache,
    trace: &mut TraceState,
) -> Option<&'a LoadedFile> {
    if !live_cache.contains_key(source.path()) {
        let text = load_candidate_text(repo_root, source.path(), trace)?;
        live_cache.insert(
            source.path().to_path_buf(),
            LoadedFile {
                source: source.clone(),
                lines: prepare_lines(&text),
                text,
            },
        );
    }

    live_cache.get(source.path())
}

fn load_candidate_text(repo_root: &Path, file: &Path, trace: &mut TraceState) -> Option<String> {
    match fs::read_to_string(file) {
        Ok(text) => Some(text),
        Err(_) => {
            trace.skipped_paths.push(source::display_path(repo_root, file));
            None
        }
    }
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

fn attach_symbol_metadata(
    text: &str,
    tree: &tree_sitter::Tree,
    findings: &mut [MatchedFinding],
) -> bool {
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
    let mut node = root.descendant_for_byte_range(
        byte_offset,
        byte_offset.saturating_add(1).min(source.len()),
    )?;
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

#[cfg(test)]
mod tests {
    use super::prepare_lines;
    use crate::chunk::{temp_repo, write_file};
    use crate::index;
    use crate::research::{output, TraceState};
    use std::fs;

    struct ScanCase {
        name: &'static str,
        files: Vec<(&'static str, &'static str)>,
        build_index: bool,
        search_areas: Vec<&'static str>,
        terms: Vec<&'static str>,
        query: &'static str,
        expected_paths: Vec<&'static str>,
    }

    #[test]
    fn scan_case_tables() {
        let cases = vec![
            ScanCase {
                name: "caps-findings-per-file",
                files: vec![(
                    "surveil/src/lib.rs",
                    "// tree-sitter attach one\n// tree-sitter attach two\n// tree-sitter attach three\n// tree-sitter attach four\n",
                )],
                build_index: false,
                search_areas: vec!["surveil/"],
                terms: vec!["tree-sitter"],
                query: "Where should Tree-sitter attach?",
                expected_paths: vec!["surveil/src/lib.rs", "surveil/src/lib.rs", "surveil/src/lib.rs"],
            },
            ScanCase {
                name: "ranked-fallback-keeps-live-findings",
                files: vec![("notes/design.md", "attach here\n")],
                build_index: false,
                search_areas: vec!["notes/"],
                terms: vec!["attach"],
                query: "Where should attach live?",
                expected_paths: vec!["notes/design.md"],
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

            let mut trace = TraceState::default();
            let search_areas = case
                .search_areas
                .iter()
                .map(|item| item.to_string())
                .collect::<Vec<_>>();
            let terms = case.terms.iter().map(|item| item.to_string()).collect::<Vec<_>>();
            let (findings, _) = output::answer_question_for_test(
                &repo,
                case.query,
                &terms,
                &search_areas,
                &[],
                &mut trace,
            )
            .expect("scan findings");

            assert_eq!(
                findings.iter().map(|item| item.path.as_str()).collect::<Vec<_>>(),
                case.expected_paths,
                "case: {}",
                case.name
            );

            let _ = fs::remove_dir_all(repo);
        }
    }

    #[test]
    fn prepare_lines_case_tables() {
        struct LineCase {
            name: &'static str,
            text: &'static str,
            expected_count: usize,
        }

        let cases = vec![
            LineCase {
                name: "single-line",
                text: "attach",
                expected_count: 1,
            },
            LineCase {
                name: "crlf-lines",
                text: "a\r\nb\r\n",
                expected_count: 3,
            },
        ];

        for case in &cases {
            assert_eq!(prepare_lines(case.text).len(), case.expected_count, "case: {}", case.name);
        }
    }
}
