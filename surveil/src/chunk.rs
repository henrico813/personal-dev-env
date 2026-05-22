use crate::source::SourceFile;
use std::path::Path;
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

    let line_starts = line_starts(text);
    if is_markdown(source.path()) {
        return build_markdown_chunks(source, text, &line_starts);
    }

    build_code_chunks(source, text, &line_starts)
}

fn line_starts(text: &str) -> Vec<usize> {
    let mut line_starts = vec![0];
    for (offset, ch) in text.char_indices() {
        if ch == '\n' && offset + 1 < text.len() {
            line_starts.push(offset + 1);
        }
    }
    line_starts
}

fn slice_lines(text: &str, line_starts: &[usize], start_line: u32, end_line: u32) -> String {
    let start = line_starts[start_line as usize - 1];
    let end = if (end_line as usize) < line_starts.len() {
        line_starts[end_line as usize]
    } else {
        text.len()
    };
    text[start..end].to_string()
}

fn make_chunk(
    source: &SourceFile,
    kind: ChunkKind,
    start_line: u32,
    end_line: u32,
    text: String,
    language: Option<&str>,
    symbol_name: Option<&str>,
    section_path: Vec<String>,
    key_path: Option<&str>,
) -> Chunk {
    Chunk {
        source: source.clone(),
        kind,
        start_line,
        end_line,
        text,
        language: language.map(str::to_string),
        symbol_name: symbol_name.map(str::to_string),
        section_path,
        key_path: key_path.map(str::to_string),
    }
}

fn is_markdown(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|ext| ext.to_str()),
        Some("md" | "markdown" | "mdx")
    )
}

fn build_markdown_chunks(source: &SourceFile, text: &str, line_starts: &[usize]) -> Vec<Chunk> {
    let lines: Vec<&str> = text.lines().collect();
    let mut chunks = Vec::new();
    let mut section_path = Vec::new();
    let mut index = 0usize;

    while index < lines.len() {
        let trimmed = lines[index].trim();
        if trimmed.is_empty() {
            index += 1;
            continue;
        }

        if let Some((level, title)) = markdown_heading(trimmed) {
            section_path.truncate(level.saturating_sub(1));
            section_path.push(title.to_string());
            index += 1;
            continue;
        }

        if let Some(fence) = fence_marker(trimmed) {
            let start_line = index as u32 + 1;
            index += 1;
            while index < lines.len() {
                if lines[index].trim_start().starts_with(fence) {
                    index += 1;
                    break;
                }
                index += 1;
            }
            let end_line = index as u32;
            chunks.push(make_chunk(
                source,
                ChunkKind::MarkdownBlock,
                start_line,
                end_line,
                slice_lines(text, line_starts, start_line, end_line),
                None,
                None,
                section_path.clone(),
                None,
            ));
            continue;
        }

        if is_list_line(trimmed) {
            let start_line = index as u32 + 1;
            index += 1;
            while index < lines.len() {
                let next = lines[index].trim();
                if next.is_empty() || markdown_heading(next).is_some() || fence_marker(next).is_some() {
                    break;
                }
                if is_list_line(next) || lines[index].starts_with(' ') || lines[index].starts_with('\t') {
                    index += 1;
                    continue;
                }
                break;
            }
            let end_line = index as u32;
            chunks.push(make_chunk(
                source,
                ChunkKind::MarkdownBlock,
                start_line,
                end_line,
                slice_lines(text, line_starts, start_line, end_line),
                None,
                None,
                section_path.clone(),
                None,
            ));
            continue;
        }

        let start_line = index as u32 + 1;
        index += 1;
        while index < lines.len() {
            let next = lines[index].trim();
            if next.is_empty()
                || markdown_heading(next).is_some()
                || fence_marker(next).is_some()
                || is_list_line(next)
            {
                break;
            }
            index += 1;
        }
        let end_line = index as u32;
        chunks.push(make_chunk(
            source,
            ChunkKind::MarkdownBlock,
            start_line,
            end_line,
            slice_lines(text, line_starts, start_line, end_line),
            None,
            None,
            section_path.clone(),
            None,
        ));
    }

    chunks
}

fn markdown_heading(line: &str) -> Option<(usize, &str)> {
    let level = line.chars().take_while(|ch| *ch == '#').count();
    if level == 0 {
        return None;
    }

    let title = line[level..].trim();
    if title.is_empty() {
        return None;
    }

    Some((level, title))
}

fn fence_marker(line: &str) -> Option<&'static str> {
    if line.starts_with("```") {
        Some("```")
    } else if line.starts_with("~~~") {
        Some("~~~")
    } else {
        None
    }
}

fn is_list_line(line: &str) -> bool {
    line.starts_with("- ")
        || line.starts_with("* ")
        || line.starts_with("+ ")
        || line.split_once('.').is_some_and(|(head, tail)| {
            !head.is_empty() && head.chars().all(|ch| ch.is_ascii_digit()) && tail.starts_with(' ')
        })
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

        chunks.push(make_chunk(
            source,
            ChunkKind::CodeSymbol,
            start_line,
            end_line,
            slice_lines(text, line_starts, start_line, end_line),
            Some(language_name),
            Some(symbol_name),
            Vec::new(),
            None,
        ));
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
    let mut chunks = Vec::new();
    for &(range_start, range_end) in ranges {
        let mut start_line = range_start;
        while start_line <= range_end {
            let end_line = (start_line + CODE_WINDOW_LINES - 1).min(range_end);
            chunks.push(make_chunk(
                source,
                kind,
                start_line,
                end_line,
                slice_lines(text, line_starts, start_line, end_line),
                language,
                None,
                Vec::new(),
                None,
            ));
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
    fn build_chunks_emits_expected_primary_chunks() {
        struct ExpectedPrimary<'a> {
            kind: ChunkKind,
            start_line: u32,
            end_line: u32,
            language: Option<&'a str>,
            symbol_name: Option<&'a str>,
        }

        struct Case<'a> {
            name: &'a str,
            file_name: &'a str,
            content: &'a str,
            expected: Vec<ExpectedPrimary<'a>>,
        }

        let cases = [
            Case {
                name: "rust_symbols_with_gap",
                file_name: "src/lib.rs",
                content: "fn alpha() {\n}\n\nfn beta() {\n}\n",
                expected: vec![
                    ExpectedPrimary {
                        kind: ChunkKind::CodeSymbol,
                        start_line: 1,
                        end_line: 2,
                        language: Some("rust"),
                        symbol_name: Some("alpha"),
                    },
                    ExpectedPrimary {
                        kind: ChunkKind::CodeSymbol,
                        start_line: 4,
                        end_line: 5,
                        language: Some("rust"),
                        symbol_name: Some("beta"),
                    },
                ],
            },
            Case {
                name: "go_symbols_with_gap",
                file_name: "src/lib.go",
                content: "package demo\n\nfunc alpha() {\n}\n\nfunc beta() {\n}\n",
                expected: vec![
                    ExpectedPrimary {
                        kind: ChunkKind::CodeSymbol,
                        start_line: 3,
                        end_line: 4,
                        language: Some("go"),
                        symbol_name: Some("alpha"),
                    },
                    ExpectedPrimary {
                        kind: ChunkKind::CodeSymbol,
                        start_line: 6,
                        end_line: 7,
                        language: Some("go"),
                        symbol_name: Some("beta"),
                    },
                ],
            },
            Case {
                name: "python_symbols_with_gap",
                file_name: "src/lib.py",
                content: "def alpha():\n    pass\n\ndef beta():\n    pass\n",
                expected: vec![
                    ExpectedPrimary {
                        kind: ChunkKind::CodeSymbol,
                        start_line: 1,
                        end_line: 2,
                        language: Some("python"),
                        symbol_name: Some("alpha"),
                    },
                    ExpectedPrimary {
                        kind: ChunkKind::CodeSymbol,
                        start_line: 4,
                        end_line: 5,
                        language: Some("python"),
                        symbol_name: Some("beta"),
                    },
                ],
            },
            Case {
                name: "typescript_symbols_with_gap",
                file_name: "src/lib.ts",
                content: "function alpha() {\n}\n\nfunction beta() {\n}\n",
                expected: vec![
                    ExpectedPrimary {
                        kind: ChunkKind::CodeSymbol,
                        start_line: 1,
                        end_line: 2,
                        language: Some("typescript"),
                        symbol_name: Some("alpha"),
                    },
                    ExpectedPrimary {
                        kind: ChunkKind::CodeSymbol,
                        start_line: 4,
                        end_line: 5,
                        language: Some("typescript"),
                        symbol_name: Some("beta"),
                    },
                ],
            },
            Case {
                name: "tsx_symbols_with_gap",
                file_name: "src/lib.tsx",
                content: "function Alpha() {\n    return <div />;\n}\n\nfunction Beta() {\n    return <span />;\n}\n",
                expected: vec![
                    ExpectedPrimary {
                        kind: ChunkKind::CodeSymbol,
                        start_line: 1,
                        end_line: 3,
                        language: Some("tsx"),
                        symbol_name: Some("Alpha"),
                    },
                    ExpectedPrimary {
                        kind: ChunkKind::CodeSymbol,
                        start_line: 5,
                        end_line: 7,
                        language: Some("tsx"),
                        symbol_name: Some("Beta"),
                    },
                ],
            },
        ];

        for case in cases {
            let repo = temp_repo(case.name);
            let path = repo.join(case.file_name);
            write_file(&path, case.content);

            let source = SourceFile::new(&repo, path, false);
            let chunks = build_chunks(&source, case.content);

            let primary: Vec<_> = chunks
                .into_iter()
                .filter(|chunk| chunk.kind == ChunkKind::CodeSymbol)
                .collect();

            assert_eq!(primary.len(), case.expected.len(), "case: {}", case.name);

            for (chunk, expected) in primary.iter().zip(case.expected.iter()) {
                assert_eq!(chunk.kind, expected.kind, "case: {}", case.name);
                assert_eq!(chunk.start_line, expected.start_line, "case: {}", case.name);
                assert_eq!(chunk.end_line, expected.end_line, "case: {}", case.name);
                assert_eq!(chunk.language.as_deref(), expected.language, "case: {}", case.name);
                assert_eq!(chunk.symbol_name.as_deref(), expected.symbol_name, "case: {}", case.name);
            }

            let _ = fs::remove_dir_all(repo);
        }
    }

    #[test]
    fn build_chunks_emits_expected_fallback_chunks() {
        struct ExpectedFallback<'a> {
            start_line: u32,
            end_line: u32,
            language: Option<&'a str>,
        }

        struct Case<'a> {
            name: &'a str,
            file_name: &'a str,
            content: &'a str,
            expected: Vec<ExpectedFallback<'a>>,
        }

        let cases = [
            Case {
                name: "gap_between_rust_symbols",
                file_name: "src/lib.rs",
                content: "fn alpha() {\n}\n\nfn beta() {\n}\n",
                expected: vec![ExpectedFallback {
                    start_line: 3,
                    end_line: 3,
                    language: Some("rust"),
                }],
            },
            Case {
                name: "parse_failure_full_file_fallback",
                file_name: "src/lib.rs",
                content: "fn attach( {\n    broken\n}\n",
                expected: vec![ExpectedFallback {
                    start_line: 1,
                    end_line: 3,
                    language: Some("rust"),
                }],
            },
            Case {
                name: "unsupported_extension_full_file_fallback",
                file_name: "src/lib.coffee",
                content: "attach one\nattach two\n",
                expected: vec![ExpectedFallback {
                    start_line: 1,
                    end_line: 2,
                    language: None,
                }],
            },
        ];

        for case in cases {
            let repo = temp_repo(case.name);
            let path = repo.join(case.file_name);
            write_file(&path, case.content);

            let source = SourceFile::new(&repo, path, false);
            let chunks = build_chunks(&source, case.content);

            let primary: Vec<_> = chunks
                .iter()
                .filter(|chunk| chunk.kind == ChunkKind::CodeSymbol)
                .collect();
            let fallbacks: Vec<_> = chunks
                .iter()
                .filter(|chunk| chunk.kind == ChunkKind::CodeFallbackWindow)
                .collect();

            assert_eq!(fallbacks.len(), case.expected.len(), "case: {}", case.name);

            for (chunk, expected) in fallbacks.iter().zip(case.expected.iter()) {
                assert_eq!(chunk.start_line, expected.start_line, "case: {}", case.name);
                assert_eq!(chunk.end_line, expected.end_line, "case: {}", case.name);
                assert_eq!(chunk.language.as_deref(), expected.language, "case: {}", case.name);
                assert_eq!(chunk.symbol_name, None, "case: {}", case.name);
            }

            for fallback in &fallbacks {
                for symbol in &primary {
                    assert!(
                        fallback.end_line < symbol.start_line
                            || fallback.start_line > symbol.end_line,
                        "case: {} fallback {:?} overlaps symbol {:?}",
                        case.name,
                        fallback,
                        symbol
                    );
                }
            }

            let _ = fs::remove_dir_all(repo);
        }
    }

    #[test]
    fn build_chunks_emits_expected_markdown_chunks() {
        struct ExpectedMarkdown<'a> {
            start_line: u32,
            end_line: u32,
            text: &'a str,
            section_path: &'a [&'a str],
        }

        struct Case<'a> {
            name: &'a str,
            content: &'a str,
            expected: Vec<ExpectedMarkdown<'a>>,
        }

        let cases = [
            Case {
                name: "paragraph_list_and_fence_blocks",
                content: "# Title\n\n## Setup\nFirst paragraph.\n\n- one\n- two\n\n```rust\nfn attach() {}\n```\n",
                expected: vec![
                    ExpectedMarkdown {
                        start_line: 4,
                        end_line: 4,
                        text: "First paragraph.\n",
                        section_path: &["Title", "Setup"],
                    },
                    ExpectedMarkdown {
                        start_line: 6,
                        end_line: 7,
                        text: "- one\n- two\n",
                        section_path: &["Title", "Setup"],
                    },
                    ExpectedMarkdown {
                        start_line: 9,
                        end_line: 11,
                        text: "```rust\nfn attach() {}\n```\n",
                        section_path: &["Title", "Setup"],
                    },
                ],
            },
            Case {
                name: "headings_split_paragraphs_without_heading_chunks",
                content: "# One\nalpha\n\n## Two\nbeta\n",
                expected: vec![
                    ExpectedMarkdown {
                        start_line: 2,
                        end_line: 2,
                        text: "alpha\n",
                        section_path: &["One"],
                    },
                    ExpectedMarkdown {
                        start_line: 5,
                        end_line: 5,
                        text: "beta\n",
                        section_path: &["One", "Two"],
                    },
                ],
            },
        ];

        for case in cases {
            let repo = temp_repo(case.name);
            let path = repo.join("docs/design.md");
            write_file(&path, case.content);

            let source = SourceFile::new(&repo, path, false);
            let chunks = build_chunks(&source, case.content);
            let markdown: Vec<_> = chunks
                .iter()
                .filter(|chunk| chunk.kind == ChunkKind::MarkdownBlock)
                .collect();

            assert_eq!(markdown.len(), case.expected.len(), "case: {}", case.name);
            assert_eq!(markdown.len(), chunks.len(), "case: {}", case.name);

            for (chunk, expected) in markdown.iter().zip(case.expected.iter()) {
                let expected_section_path: Vec<_> = expected
                    .section_path
                    .iter()
                    .map(|part| (*part).to_string())
                    .collect();

                assert_eq!(chunk.start_line, expected.start_line, "case: {}", case.name);
                assert_eq!(chunk.end_line, expected.end_line, "case: {}", case.name);
                assert_eq!(chunk.text, expected.text, "case: {}", case.name);
                assert_eq!(chunk.section_path, expected_section_path, "case: {}", case.name);
                assert_eq!(chunk.language, None, "case: {}", case.name);
                assert_eq!(chunk.symbol_name, None, "case: {}", case.name);
            }

            for pair in markdown.windows(2) {
                assert!(
                    pair[0].end_line < pair[1].start_line,
                    "case: {} markdown chunks overlap: {:?} {:?}",
                    case.name,
                    pair[0],
                    pair[1]
                );
            }

            let _ = fs::remove_dir_all(repo);
        }
    }
}
