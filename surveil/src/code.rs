use super::{build_fixed_windows, make_chunk, slice_lines, Chunk, ChunkKind, CODE_WINDOW_LINES};
use crate::source::SourceFile;
use tree_sitter::{Node, Parser};

pub(super) fn build_code_chunks(source: &SourceFile, text: &str, line_starts: &[usize]) -> Vec<Chunk> {
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
