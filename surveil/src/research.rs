use crate::schema::{Answer, ExplicitFile, Finding, GatherOutput, ResearchOutput, TraceOutput, SCHEMA_VERSION};
use std::collections::{BTreeSet, HashSet};
use std::error::Error;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use tree_sitter::Parser;

pub fn run(context: &Path, trace_out: &Path) -> Result<(), Box<dyn Error>> {
    let context_text = fs::read_to_string(context)?;
    let gather: GatherOutput = serde_json::from_str(&context_text)?;
    if gather.schema_version != SCHEMA_VERSION {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "context version mismatch: expected {}, got {}",
                SCHEMA_VERSION, gather.schema_version
            ),
        )
        .into());
    }

    let repo_root = Path::new(&gather.repo_root).to_path_buf();
    let mut trace = TraceState::default();
    let mut answers = Vec::with_capacity(gather.questions.len());

    for question in &gather.questions {
        let (findings, negative_evidence) = answer_question(
            &repo_root,
            question,
            &gather.terms,
            &gather.search_areas,
            &gather.explicit_files,
            &mut trace,
        )?;
        if findings.is_empty() {
            trace.unmatched_questions.push(question.clone());
        }
        answers.push(Answer {
            question: question.clone(),
            findings,
            negative_evidence,
        });
    }

    let open_questions = trace.unmatched_questions.clone();
    let report = ResearchOutput {
        schema_version: SCHEMA_VERSION.to_string(),
        summary: gather.summary,
        answers,
        blockers: gather.blockers,
        open_questions,
    };

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
    path: PathBuf,
    explicit: bool,
    findings: Vec<Finding>,
}

fn answer_question(
    repo_root: &Path,
    question: &str,
    terms: &[String],
    search_areas: &[String],
    explicit_files: &[ExplicitFile],
    trace: &mut TraceState,
) -> Result<(Vec<Finding>, Vec<String>), Box<dyn Error>> {
    let tokens = search_tokens(terms, question);
    let mut ranked_files = Vec::new();

    for (file, from_explicit) in collect_candidate_files(repo_root, search_areas, explicit_files, trace)? {
        trace.files_considered.insert(file.clone());
        let text = match fs::read_to_string(&file) {
            Ok(text) => text,
            Err(_) => {
                trace.skipped_paths.push(display_path(repo_root, &file));
                continue;
            }
        };

        let mut file_findings = Vec::new();
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
            let line = &text[line_start..content_end];
            let lower_line = line.to_lowercase();
            if let Some(matched_from) = tokens.iter().find(|token| lower_line.contains(token.as_str())) {
                if let Some(match_offset) = case_insensitive_byte_offset(line, matched_from) {
                    file_findings.push(MatchedFinding {
                        finding: Finding {
                            path: display_path(repo_root, &file),
                            line: line_number,
                            excerpt: line.trim().to_string(),
                            source: if from_explicit {
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
                        byte_offset: line_start + match_offset,
                    });
                }
            }

            if line_end == text.len() {
                break;
            }
            line_start = line_end + 1;
            line_number += 1;
        }

        if !file_findings.is_empty() {
            if let Some((language, symbol_language)) = symbol_language_for_path(&file) {
                if let Some(tree) = parse_tree(&text, language) {
                    if !tree.root_node().has_error() {
                        enrich_symbol_metadata(&text, &tree, symbol_language, &mut file_findings);
                    }
                }
            }
            trace.files_matched.insert(file.clone());
            ranked_files.push(RankedFileFindings {
                path: file,
                explicit: from_explicit,
                findings: file_findings.into_iter().map(|hit| hit.finding).collect(),
            });
        }
    }

    ranked_files.sort_by(|a, b| {
        b.explicit
            .cmp(&a.explicit)
            .then_with(|| b.findings.len().cmp(&a.findings.len()))
            .then_with(|| display_path(repo_root, &a.path).cmp(&display_path(repo_root, &b.path)))
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

#[derive(Debug)]
struct MatchedFinding {
    finding: Finding,
    byte_offset: usize,
}

#[derive(Clone, Copy)]
enum SymbolLanguage {
    Rust,
    Go,
    Python,
    TypeScript,
    Tsx,
}

fn symbol_language_for_path(path: &Path) -> Option<(tree_sitter::Language, SymbolLanguage)> {
    let extension = path.extension()?.to_string_lossy().to_ascii_lowercase();
    match extension.as_str() {
        "rs" => Some((tree_sitter_rust::LANGUAGE.into(), SymbolLanguage::Rust)),
        "go" => Some((tree_sitter_go::LANGUAGE.into(), SymbolLanguage::Go)),
        "py" => Some((tree_sitter_python::LANGUAGE.into(), SymbolLanguage::Python)),
        "ts" => Some((tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(), SymbolLanguage::TypeScript)),
        "tsx" => Some((tree_sitter_typescript::LANGUAGE_TSX.into(), SymbolLanguage::Tsx)),
        _ => None,
    }
}

fn parse_tree(text: &str, language: tree_sitter::Language) -> Option<tree_sitter::Tree> {
    let mut parser = Parser::new();
    parser.set_language(&language).ok()?;
    parser.parse(text, None)
}

fn enrich_symbol_metadata(
    text: &str,
    tree: &tree_sitter::Tree,
    language: SymbolLanguage,
    findings: &mut [MatchedFinding],
) {
    for hit in findings {
        if let Some(symbol) = enclosing_symbol(tree.root_node(), text.as_bytes(), hit.byte_offset, language) {
            hit.finding.symbol_kind = Some(symbol.kind);
            hit.finding.symbol_name = Some(symbol.name);
            hit.finding.symbol_start_line = Some(symbol.start_line);
            hit.finding.symbol_end_line = Some(symbol.end_line);
        }
    }
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
    language: SymbolLanguage,
) -> Option<SymbolInfo> {
    let mut node = root.descendant_for_byte_range(byte_offset, byte_offset.saturating_add(1).min(source.len()))?;
    loop {
        if is_symbol_node(node, language) {
            let name_node = node
                .child_by_field_name("name")
                .or_else(|| node.named_child(0))?;
            let name = name_node.utf8_text(source).ok()?.to_string();
            return Some(SymbolInfo {
                kind: normalized_symbol_kind(node, language).to_string(),
                name,
                start_line: node.start_position().row as u32 + 1,
                end_line: node.end_position().row as u32 + 1,
            });
        }
        node = node.parent()?;
    }
}

fn is_symbol_node(node: tree_sitter::Node, language: SymbolLanguage) -> bool {
    matches!(
        (language, node.kind()),
        (SymbolLanguage::Rust, "function_item")
            | (SymbolLanguage::Rust, "struct_item")
            | (SymbolLanguage::Rust, "enum_item")
            | (SymbolLanguage::Rust, "trait_item")
            | (SymbolLanguage::Rust, "impl_item")
            | (SymbolLanguage::Rust, "mod_item")
            | (SymbolLanguage::Rust, "const_item")
            | (SymbolLanguage::Rust, "static_item")
            | (SymbolLanguage::Rust, "type_item")
            | (SymbolLanguage::Go, "function_declaration")
            | (SymbolLanguage::Go, "method_declaration")
            | (SymbolLanguage::Go, "type_declaration")
            | (SymbolLanguage::Python, "function_definition")
            | (SymbolLanguage::Python, "class_definition")
            | (SymbolLanguage::TypeScript, "function_declaration")
            | (SymbolLanguage::TypeScript, "method_definition")
            | (SymbolLanguage::TypeScript, "class_declaration")
            | (SymbolLanguage::TypeScript, "interface_declaration")
            | (SymbolLanguage::TypeScript, "type_alias_declaration")
            | (SymbolLanguage::Tsx, "function_declaration")
            | (SymbolLanguage::Tsx, "method_definition")
            | (SymbolLanguage::Tsx, "class_declaration")
            | (SymbolLanguage::Tsx, "interface_declaration")
            | (SymbolLanguage::Tsx, "type_alias_declaration")
    )
}

fn normalized_symbol_kind(node: tree_sitter::Node, language: SymbolLanguage) -> &'static str {
    match (language, node.kind()) {
        (SymbolLanguage::Rust, "function_item")
        | (SymbolLanguage::Go, "function_declaration")
        | (SymbolLanguage::Python, "function_definition")
        | (SymbolLanguage::TypeScript, "function_declaration")
        | (SymbolLanguage::Tsx, "function_declaration") => "function",
        (SymbolLanguage::Go, "method_declaration")
        | (SymbolLanguage::TypeScript, "method_definition")
        | (SymbolLanguage::Tsx, "method_definition") => "method",
        _ => "type",
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

fn collect_candidate_files(
    repo_root: &Path,
    search_areas: &[String],
    explicit_files: &[ExplicitFile],
    trace: &mut TraceState,
) -> Result<Vec<(PathBuf, bool)>, Box<dyn Error>> {
    let mut candidates = Vec::new();
    let mut seen = HashSet::new();

    let mut explicit_paths: Vec<PathBuf> = explicit_files
        .iter()
        .filter(|file| file.found)
        .map(|file| resolve_path(repo_root, &file.path))
        .collect();
    explicit_paths.sort();
    for file in explicit_paths {
        if seen.insert(file.clone()) {
            candidates.push((file, true));
        }
    }

    for area in search_areas {
        let area_path = resolve_path(repo_root, area);
        for file in collect_files(repo_root, &area_path, trace)? {
            if seen.insert(file.clone()) {
                candidates.push((file, false));
            }
        }
    }

    Ok(candidates)
}

fn collect_files(repo_root: &Path, dir: &Path, trace: &mut TraceState) -> Result<Vec<PathBuf>, Box<dyn Error>> {
    if is_skipped_path(repo_root, dir) {
        trace.skipped_paths.push(display_path(repo_root, dir));
        return Ok(Vec::new());
    }

    if dir.is_file() {
        return Ok(vec![dir.to_path_buf()]);
    }

    if !dir.is_dir() {
        trace.skipped_paths.push(display_path(repo_root, dir));
        return Ok(Vec::new());
    }

    let mut entries = Vec::new();
    let read_dir = match fs::read_dir(dir) {
        Ok(read_dir) => read_dir,
        Err(_) => {
            trace.skipped_paths.push(display_path(repo_root, dir));
            return Ok(Vec::new());
        }
    };
    for entry in read_dir {
        match entry {
            Ok(entry) => entries.push(entry.path()),
            Err(_) => trace.skipped_paths.push(display_path(repo_root, dir)),
        }
    }
    entries.sort();

    let mut files = Vec::new();
    for path in entries {
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(_) => {
                trace.skipped_paths.push(display_path(repo_root, &path));
                continue;
            }
        };
        if is_skipped_path(repo_root, &path) {
            trace.skipped_paths.push(display_path(repo_root, &path));
            continue;
        }
        if metadata.is_dir() {
            files.extend(collect_files(repo_root, &path, trace)?);
        } else if metadata.is_file() {
            files.push(path);
        }
    }
    Ok(files)
}

fn resolve_path(repo_root: &Path, raw: &str) -> PathBuf {
    let path = Path::new(raw);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        repo_root.join(path)
    }
}

fn is_skipped_path(repo_root: &Path, path: &Path) -> bool {
    let relative = path.strip_prefix(repo_root).unwrap_or(path);
    relative.components().any(|component| {
        matches!(
            component,
            std::path::Component::Normal(name)
                if matches!(
                    name.to_string_lossy().as_ref(),
                    "target" | "node_modules" | "dist" | "build" | "pack" | ".git"
                )
        )
    })
}

fn display_path(repo_root: &Path, path: &Path) -> String {
    path.strip_prefix(repo_root)
        .map(|relative| relative.to_string_lossy().into_owned())
        .unwrap_or_else(|_| path.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::{answer_question, run, TraceState};
    use crate::schema::{ExplicitFile, GatherOutput, SCHEMA_VERSION};
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
    fn rejects_mismatched_context_version() {
        let repo = temp_repo("version-mismatch");
        let context = repo.join("context.json");
        let trace = repo.join("trace.json");
        write_context(
            &context,
            &GatherOutput {
                schema_version: "surveil.v4".to_string(),
                repo_root: repo.to_string_lossy().into_owned(),
                summary: "summary".to_string(),
                explicit_files: Vec::new(),
                search_areas: Vec::new(),
                questions: Vec::new(),
                terms: Vec::new(),
                blockers: Vec::new(),
            },
        );

        let error = run(&context, &trace).expect_err("version mismatch");
        assert!(error.to_string().contains("context version mismatch"));

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
    fn unsupported_utf8_file_leaves_symbol_fields_empty() {
        let repo = temp_repo("unsupported-symbols");
        write_file(&repo.join("surveil/notes.md"), "// tree-sitter attach\n");

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
                questions: vec!["Where should Tree-sitter attach?".to_string()],
                terms: vec!["tree-sitter".to_string()],
                blockers: Vec::new(),
            },
        );

        run(&context, &trace).expect("research run");

        let _ = fs::remove_dir_all(repo);
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
