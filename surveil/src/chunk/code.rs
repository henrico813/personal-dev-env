use super::{build_fixed_windows, make_chunk, slice_lines, Chunk, ChunkKind, CODE_WINDOW_LINES};
use crate::source::SourceFile;
use tree_sitter::{Node, Parser};

pub(super) fn build_code_chunks(
    source: &SourceFile,
    text: &str,
    line_starts: &[usize],
) -> Vec<Chunk> {
    let total_lines = line_starts.len() as u32;
    let full_file = [(1, total_lines)];

    let Some((language_name, language)) =
        (match source.path().extension().and_then(|ext| ext.to_str()) {
            Some("rs") => Some(("rust", tree_sitter_rust::LANGUAGE.into())),
            Some("go") => Some(("go", tree_sitter_go::LANGUAGE.into())),
            Some("py") => Some(("python", tree_sitter_python::LANGUAGE.into())),
            Some("ts") => Some((
                "typescript",
                tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            )),
            Some("tsx") => Some(("tsx", tree_sitter_typescript::LANGUAGE_TSX.into())),
            _ => None,
        })
    else {
        return build_fixed_windows(
            source,
            text,
            line_starts,
            &full_file,
            ChunkKind::CodeFallbackWindow,
            None,
            CODE_WINDOW_LINES,
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
            CODE_WINDOW_LINES,
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
        CODE_WINDOW_LINES,
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

        let Some(name_node) = node
            .child_by_field_name("name")
            .or_else(|| node.named_child(0))
        else {
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

#[cfg(test)]
mod tests {
    use super::super::{build_chunks, temp_repo, write_file, ChunkKind};
    use crate::source::SourceFile;
    use std::fs;

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
                assert_eq!(
                    chunk.language.as_deref(),
                    expected.language,
                    "case: {}",
                    case.name
                );
                assert_eq!(
                    chunk.symbol_name.as_deref(),
                    expected.symbol_name,
                    "case: {}",
                    case.name
                );
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
                assert_eq!(
                    chunk.language.as_deref(),
                    expected.language,
                    "case: {}",
                    case.name
                );
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
}
