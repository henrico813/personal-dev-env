use crate::source::SourceFile;
use tree_sitter::{Node, Parser};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ChunkKind {
    CodeSymbol,
    CodeFallbackWindow,
    MarkdownBlock,
    ConfigStanza,
    GenericFallbackWindow,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Chunk {
    pub(crate) source: SourceFile,
    pub(crate) kind: ChunkKind,
    pub(crate) start_line: u32,
    pub(crate) end_line: u32,
    pub(crate) text: String,
    pub(crate) language: Option<String>,
    pub(crate) symbol_name: Option<String>,
    pub(crate) section_path: Vec<String>,
    pub(crate) key_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SearchQuery {
    pub(crate) tokens: Vec<String>,
    pub(crate) limit: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RankedChunk {
    pub(crate) chunk: Chunk,
    pub(crate) score: f32,
}

const CODE_WINDOW_LINES: u32 = 20;

pub(crate) fn build_chunks(source: &SourceFile, text: &str) -> Vec<Chunk> {
    if text.is_empty() {
        return Vec::new();
    }

    let mut line_starts = vec![0];
    for (offset, ch) in text.char_indices() {
        if ch == '\n' && offset + 1 < text.len() {
            line_starts.push(offset + 1);
        }
    }

    build_code_chunks(source, text, &line_starts)
}

fn build_code_chunks(source: &SourceFile, text: &str, line_starts: &[usize]) -> Vec<Chunk> {
    let total_lines = line_starts.len() as u32;
    let full_file = [(1, total_lines)];

    let Some((language_name, language)) = (match source.path().extension().and_then(|ext| ext.to_str()) {
        Some("rs") => Some(("rust", tree_sitter_rust::LANGUAGE.into())),
        Some("go") => Some(("go", tree_sitter_go::LANGUAGE.into())),
        Some("py") => Some(("python", tree_sitter_python::LANGUAGE.into())),
        Some("ts") => Some(("typescript", tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into())),
        Some("tsx") => Some(("tsx", tree_sitter_typescript::LANGUAGE_TSX.into())),
        _ => None,
    }) else {
        return build_fixed_windows(
            source,
            text,
            line_starts,
            &full_file,
            ChunkKind::CodeFallbackWindow,
            None,
        );
    };

    let mut parser = Parser::new();
    let parsed_tree = parser
        .set_language(&language)
        .ok()
        .and_then(|_| parser.parse(text, None))
        .filter(|tree| !tree.root_node().has_error());
    let Some(tree) = parsed_tree else {
        return build_fixed_windows(
            source,
            text,
            line_starts,
            &full_file,
            ChunkKind::CodeFallbackWindow,
            Some(language_name),
        );
    };

    let mut chunks = collect_symbol_chunks(source, text, line_starts, &tree, language_name);
    let mut uncovered = Vec::new();
    let mut next_start = 1;
    for chunk in &chunks {
        if next_start < chunk.start_line {
            uncovered.push((next_start, chunk.start_line - 1));
        }
        next_start = chunk.end_line + 1;
    }
    if next_start <= total_lines {
        uncovered.push((next_start, total_lines));
    }

    chunks.extend(build_fixed_windows(
        source,
        text,
        line_starts,
        &uncovered,
        ChunkKind::CodeFallbackWindow,
        Some(language_name),
    ));
    chunks.sort_by_key(|chunk| (chunk.start_line, chunk.end_line));
    chunks
}

fn collect_symbol_chunks(
    source: &SourceFile,
    text: &str,
    line_starts: &[usize],
    tree: &tree_sitter::Tree,
    language_name: &str,
) -> Vec<Chunk> {
    fn collect_nodes<'tree>(node: Node<'tree>, out: &mut Vec<Node<'tree>>) {
        let kind = node.kind();
        if node.child_by_field_name("name").is_some()
            && (kind.ends_with("_item")
                || kind.ends_with("_declaration")
                || kind.ends_with("_definition")
                || kind.ends_with("_declarator"))
        {
            out.push(node);
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            collect_nodes(child, out);
        }
    }

    let slice_lines = |start_line: u32, end_line: u32| {
        let start = line_starts[start_line as usize - 1];
        let end = if (end_line as usize) < line_starts.len() {
            line_starts[end_line as usize]
        } else {
            text.len()
        };
        text[start..end].to_string()
    };

    let mut nodes = Vec::new();
    collect_nodes(tree.root_node(), &mut nodes);
    nodes.sort_by_key(|node| (node.start_position().row, node.end_position().row));

    let mut chunks = Vec::new();
    let mut covered_until = 0u32;
    for node in nodes {
        let start_line = node.start_position().row as u32 + 1;
        let end_line = node.end_position().row as u32 + 1;
        if start_line > end_line || start_line <= covered_until {
            continue;
        }

        let Some(name_node) = node.child_by_field_name("name").or_else(|| node.named_child(0)) else {
            continue;
        };
        let Ok(symbol_name) = name_node.utf8_text(text.as_bytes()) else {
            continue;
        };

        chunks.push(Chunk {
            source: source.clone(),
            kind: ChunkKind::CodeSymbol,
            start_line,
            end_line,
            text: slice_lines(start_line, end_line),
            language: Some(language_name.to_string()),
            symbol_name: Some(symbol_name.to_string()),
            section_path: Vec::new(),
            key_path: None,
        });
        covered_until = end_line;
    }

    chunks
}

fn build_fixed_windows(
    source: &SourceFile,
    text: &str,
    line_starts: &[usize],
    ranges: &[(u32, u32)],
    kind: ChunkKind,
    language: Option<&str>,
) -> Vec<Chunk> {
    let slice_lines = |start_line: u32, end_line: u32| {
        let start = line_starts[start_line as usize - 1];
        let end = if (end_line as usize) < line_starts.len() {
            line_starts[end_line as usize]
        } else {
            text.len()
        };
        text[start..end].to_string()
    };

    let mut chunks = Vec::new();
    for &(range_start, range_end) in ranges {
        let mut start_line = range_start;
        while start_line <= range_end {
            let end_line = (start_line + CODE_WINDOW_LINES - 1).min(range_end);
            chunks.push(Chunk {
                source: source.clone(),
                kind,
                start_line,
                end_line,
                text: slice_lines(start_line, end_line),
                language: language.map(str::to_string),
                symbol_name: None,
                section_path: Vec::new(),
                key_path: None,
            });
            start_line = end_line + 1;
        }
    }

    chunks
}

#[cfg(test)]
mod tests {
    use super::{build_chunks, ChunkKind};
    use crate::source::SourceFile;
    use std::fs;
    use std::io::Write;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_repo(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("surveil-chunks-{name}-{stamp}"));
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

    #[test]
    fn symbol_chunks_leave_coverage_for_fallback_windows() {
        let repo = temp_repo("coverage");
        let path = repo.join("src/lib.rs");
        let content = "fn alpha() {\n}\n\nfn beta() {\n}\n";
        write_file(&path, content);

        let source = SourceFile::new(&repo, path, false);
        let chunks = build_chunks(&source, content);

        assert!(chunks.iter().any(|chunk| chunk.kind == ChunkKind::CodeSymbol));
        assert!(chunks.iter().any(|chunk| chunk.kind == ChunkKind::CodeFallbackWindow));

        let line_count = content.lines().count() as u32;
        let mut covered = vec![false; line_count as usize + 1];
        for chunk in &chunks {
            for line in chunk.start_line..=chunk.end_line {
                covered[line as usize] = true;
            }
        }
        assert!(covered[1..].iter().all(|covered| *covered));

        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn fallback_cases_emit_code_windows() {
        struct Case<'a> {
            name: &'a str,
            file_name: &'a str,
            content: &'a str,
            expected_language: Option<&'a str>,
        }

        let cases = [
            Case {
                name: "parse failure keeps language",
                file_name: "surveil/src/lib.rs",
                content: "fn attach( {\n    broken\n}\n",
                expected_language: Some("rust"),
            },
            Case {
                name: "unsupported extension has no language",
                file_name: "surveil/src/lib.coffee",
                content: "attach one\nattach two\n",
                expected_language: None,
            },
        ];

        for case in cases {
            let repo = temp_repo(case.name);
            let path = repo.join(case.file_name);
            write_file(&path, case.content);

            let source = SourceFile::new(&repo, path, false);
            let chunks = build_chunks(&source, &fs::read_to_string(source.path()).expect("read text"));

            assert_eq!(chunks.len(), 1, "case: {}", case.name);
            assert_eq!(chunks[0].kind, ChunkKind::CodeFallbackWindow, "case: {}", case.name);
            assert_eq!(chunks[0].language.as_deref(), case.expected_language, "case: {}", case.name);

            let _ = fs::remove_dir_all(repo);
        }
    }
}
