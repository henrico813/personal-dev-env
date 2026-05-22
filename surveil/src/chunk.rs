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
const CONFIG_WINDOW_LINES: u32 = 16;

pub(crate) fn build_chunks(source: &SourceFile, text: &str) -> Vec<Chunk> {
    if text.is_empty() {
        return Vec::new();
    }

    let line_starts = line_starts(text);
    if is_markdown(source.path()) {
        return build_markdown_chunks(source, text, &line_starts);
    }

    if let Some(config_format) = config_format(source.path()) {
        return build_config_chunks(source, text, &line_starts, config_format);
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

enum ConfigFormat {
    Toml,
    Json,
    Yaml,
    Ini,
    Env,
}

fn config_format(path: &Path) -> Option<ConfigFormat> {
    let file_name = path.file_name()?.to_str()?;
    if file_name == ".env" || file_name.starts_with(".env.") {
        return Some(ConfigFormat::Env);
    }

    match path.extension().and_then(|ext| ext.to_str()) {
        Some("toml") => Some(ConfigFormat::Toml),
        Some("json") => Some(ConfigFormat::Json),
        Some("yaml" | "yml") => Some(ConfigFormat::Yaml),
        Some("ini" | "cfg" | "conf") => Some(ConfigFormat::Ini),
        _ => None,
    }
}

fn build_config_chunks(
    source: &SourceFile,
    text: &str,
    line_starts: &[usize],
    format: ConfigFormat,
) -> Vec<Chunk> {
    let total_lines = line_starts.len() as u32;
    let full_file = [(1, total_lines)];
    let mut chunks = match format {
        ConfigFormat::Toml => build_toml_or_ini_chunks(source, text, line_starts, true),
        ConfigFormat::Json => build_json_chunks(source, text, line_starts),
        ConfigFormat::Yaml => build_yaml_chunks(source, text, line_starts),
        ConfigFormat::Ini => build_toml_or_ini_chunks(source, text, line_starts, false),
        ConfigFormat::Env => build_env_chunks(source, text, line_starts),
    }
    .unwrap_or_default();

    let window_lines = CONFIG_WINDOW_LINES;

    if chunks.is_empty() {
        return build_fixed_windows(
            source,
            text,
            line_starts,
            &full_file,
            ChunkKind::GenericFallbackWindow,
            None,
            window_lines,
        );
    }

    chunks.sort_by_key(|chunk| (chunk.start_line, chunk.end_line));

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
        ChunkKind::GenericFallbackWindow,
        None,
        window_lines,
    ));
    chunks.sort_by_key(|chunk| (chunk.start_line, chunk.end_line));
    chunks
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

fn build_toml_or_ini_chunks(
    source: &SourceFile,
    text: &str,
    line_starts: &[usize],
    is_toml: bool,
) -> Option<Vec<Chunk>> {
    let lines: Vec<&str> = text.lines().collect();
    let mut chunks = Vec::new();
    let mut section_path: Vec<String> = Vec::new();

    for (index, line) in lines.iter().enumerate() {
        let start_line = index as u32 + 1;
        let trimmed = line.trim();
        if trimmed.is_empty() || is_config_comment(trimmed, !is_toml) {
            continue;
        }

        if trimmed.starts_with("[[") || trimmed.starts_with(']') {
            return None;
        }

        if let Some(section) = parse_bracket_section(trimmed) {
            section_path = section.clone();
            chunks.push(make_chunk(
                source,
                ChunkKind::ConfigStanza,
                start_line,
                start_line,
                slice_lines(text, line_starts, start_line, start_line),
                None,
                None,
                Vec::new(),
                Some(&join_key_path(&section)),
            ));
            continue;
        }

        let Some((key, value)) = trimmed.split_once('=') else {
            return None;
        };

        let key = key.trim();
        let value = value.trim();
        let key_path = split_key_path(key)?;
        if key_path.is_empty() || value.is_empty() || value.starts_with('[') || value.starts_with('{') {
            return None;
        }

        let mut full_path = section_path.clone();
        full_path.extend(key_path);
        chunks.push(make_chunk(
            source,
            ChunkKind::ConfigStanza,
            start_line,
            start_line,
            slice_lines(text, line_starts, start_line, start_line),
            None,
            None,
            Vec::new(),
            Some(&join_key_path(&full_path)),
        ));
    }

    Some(chunks)
}

fn build_yaml_chunks(source: &SourceFile, text: &str, line_starts: &[usize]) -> Option<Vec<Chunk>> {
    let lines: Vec<&str> = text.lines().collect();
    let mut chunks = Vec::new();
    let mut path_stack: Vec<(usize, String)> = Vec::new();

    for (index, line) in lines.iter().enumerate() {
        let start_line = index as u32 + 1;
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if trimmed.starts_with('-') || trimmed.starts_with('[') || trimmed.starts_with('{') {
            return None;
        }

        let indent = line.chars().take_while(|ch| ch.is_whitespace()).count();
        while path_stack.last().is_some_and(|(stack_indent, _)| *stack_indent >= indent) {
            path_stack.pop();
        }

        let Some((raw_key, raw_value)) = trimmed.split_once(':') else {
            return None;
        };
        let key = normalize_key(raw_key)?;
        let value = raw_value.trim();
        let mut full_path = current_key_path(&path_stack);
        full_path.push(key.clone());

        chunks.push(make_chunk(
            source,
            ChunkKind::ConfigStanza,
            start_line,
            start_line,
            slice_lines(text, line_starts, start_line, start_line),
            None,
            None,
            Vec::new(),
            Some(&join_key_path(&full_path)),
        ));

        if value.is_empty() {
            path_stack.push((indent, key));
        } else if value.starts_with('[') || value.starts_with('{') || value == "|" || value == ">" {
            return None;
        }
    }

    Some(chunks)
}

fn build_json_chunks(source: &SourceFile, text: &str, line_starts: &[usize]) -> Option<Vec<Chunk>> {
    let lines: Vec<&str> = text.lines().collect();
    let mut chunks = Vec::new();
    let mut path_stack: Vec<String> = Vec::new();

    for (index, line) in lines.iter().enumerate() {
        let start_line = index as u32 + 1;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if trimmed == "}" || trimmed == "}," {
            path_stack.pop();
            continue;
        }

        if trimmed == "{" || trimmed == "{," {
            continue;
        }

        if trimmed.contains('[') || trimmed.contains(']') {
            return None;
        }

        let Some((key, value)) = parse_json_property(trimmed) else {
            return None;
        };
        let value = value.trim_end_matches(',').trim();
        let mut full_path = path_stack.clone();
        full_path.push(key.clone());

        chunks.push(make_chunk(
            source,
            ChunkKind::ConfigStanza,
            start_line,
            start_line,
            slice_lines(text, line_starts, start_line, start_line),
            None,
            None,
            Vec::new(),
            Some(&join_key_path(&full_path)),
        ));

        if value == "{" {
            path_stack.push(key);
        } else if value.starts_with('{') || value.starts_with('[') {
            return None;
        }
    }

    Some(chunks)
}

fn build_env_chunks(source: &SourceFile, text: &str, line_starts: &[usize]) -> Option<Vec<Chunk>> {
    let lines: Vec<&str> = text.lines().collect();
    let mut chunks = Vec::new();

    for (index, line) in lines.iter().enumerate() {
        let start_line = index as u32 + 1;
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        let stripped = trimmed.strip_prefix("export ").unwrap_or(trimmed);
        let Some((key, _value)) = stripped.split_once('=') else {
            return None;
        };
        let key = key.trim();
        if key.is_empty()
            || !key
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
        {
            return None;
        }

        chunks.push(make_chunk(
            source,
            ChunkKind::ConfigStanza,
            start_line,
            start_line,
            slice_lines(text, line_starts, start_line, start_line),
            None,
            None,
            Vec::new(),
            Some(key),
        ));
    }

    Some(chunks)
}

fn parse_bracket_section(trimmed: &str) -> Option<Vec<String>> {
    let content = trimmed.strip_prefix('[')?.strip_suffix(']')?.trim();
    if content.is_empty() || content.contains('[') || content.contains(']') {
        return None;
    }

    split_key_path(content)
}

fn parse_json_property(trimmed: &str) -> Option<(String, &str)> {
    let key_start = trimmed.find('"')?;
    let remainder = &trimmed[key_start + 1..];
    let key_end = remainder.find('"')?;
    let key = remainder[..key_end].to_string();
    let after_key = remainder[key_end + 1..].trim_start();
    let value = after_key.strip_prefix(':')?.trim_start();
    Some((key, value))
}

fn is_config_comment(trimmed: &str, allow_semicolon: bool) -> bool {
    trimmed.starts_with('#') || (allow_semicolon && trimmed.starts_with(';'))
}

fn split_key_path(raw: &str) -> Option<Vec<String>> {
    let parts: Vec<String> = raw
        .split('.')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(str::to_string)
        .collect();
    if parts.is_empty() {
        None
    } else {
        Some(parts)
    }
}

fn normalize_key(raw: &str) -> Option<String> {
    let key = raw.trim();
    if key.is_empty() {
        return None;
    }
    if key.len() >= 2 {
        let bytes = key.as_bytes();
        if (bytes[0] == b'"' && bytes[key.len() - 1] == b'"')
            || (bytes[0] == b'\'' && bytes[key.len() - 1] == b'\'')
        {
            return Some(key[1..key.len() - 1].to_string());
        }
    }
    Some(key.to_string())
}

fn join_key_path(parts: &[String]) -> String {
    parts.join(".")
}

fn current_key_path(path_stack: &[(usize, String)]) -> Vec<String> {
    path_stack.iter().map(|(_, key)| key.clone()).collect()
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

fn build_fixed_windows(
    source: &SourceFile,
    text: &str,
    line_starts: &[usize],
    ranges: &[(u32, u32)],
    kind: ChunkKind,
    language: Option<&str>,
    window_lines: u32,
) -> Vec<Chunk> {
    let mut chunks = Vec::new();
    for &(range_start, range_end) in ranges {
        let mut start_line = range_start;
        while start_line <= range_end {
            let end_line = (start_line + window_lines - 1).min(range_end);
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
    fn build_chunks_emits_expected_config_chunks() {
        struct ExpectedConfig<'a> {
            kind: ChunkKind,
            start_line: u32,
            end_line: u32,
            key_path: Option<&'a str>,
        }

        struct Case<'a> {
            name: &'a str,
            file_name: &'a str,
            content: &'a str,
            expected: Vec<ExpectedConfig<'a>>,
        }

        let cases = [
            Case {
                name: "toml_nested_paths",
                file_name: "config.toml",
                content: "[server]\nhost = \"localhost\"\nport = 8080\n",
                expected: vec![
                    ExpectedConfig {
                        kind: ChunkKind::ConfigStanza,
                        start_line: 1,
                        end_line: 1,
                        key_path: Some("server"),
                    },
                    ExpectedConfig {
                        kind: ChunkKind::ConfigStanza,
                        start_line: 2,
                        end_line: 2,
                        key_path: Some("server.host"),
                    },
                    ExpectedConfig {
                        kind: ChunkKind::ConfigStanza,
                        start_line: 3,
                        end_line: 3,
                        key_path: Some("server.port"),
                    },
                ],
            },
            Case {
                name: "yaml_nested_paths",
                file_name: "config.yaml",
                content: "server:\n  host: localhost\n  database:\n    port: 5432\n",
                expected: vec![
                    ExpectedConfig {
                        kind: ChunkKind::ConfigStanza,
                        start_line: 1,
                        end_line: 1,
                        key_path: Some("server"),
                    },
                    ExpectedConfig {
                        kind: ChunkKind::ConfigStanza,
                        start_line: 2,
                        end_line: 2,
                        key_path: Some("server.host"),
                    },
                    ExpectedConfig {
                        kind: ChunkKind::ConfigStanza,
                        start_line: 3,
                        end_line: 3,
                        key_path: Some("server.database"),
                    },
                    ExpectedConfig {
                        kind: ChunkKind::ConfigStanza,
                        start_line: 4,
                        end_line: 4,
                        key_path: Some("server.database.port"),
                    },
                ],
            },
            Case {
                name: "json_nested_paths",
                file_name: "config.json",
                content: "{\n  \"server\": {\n    \"host\": \"localhost\",\n    \"port\": 8080\n  }\n}\n",
                expected: vec![
                    ExpectedConfig {
                        kind: ChunkKind::ConfigStanza,
                        start_line: 2,
                        end_line: 2,
                        key_path: Some("server"),
                    },
                    ExpectedConfig {
                        kind: ChunkKind::ConfigStanza,
                        start_line: 3,
                        end_line: 3,
                        key_path: Some("server.host"),
                    },
                    ExpectedConfig {
                        kind: ChunkKind::ConfigStanza,
                        start_line: 4,
                        end_line: 4,
                        key_path: Some("server.port"),
                    },
                ],
            },
            Case {
                name: "env_keys",
                file_name: ".env.local",
                content: "API_URL=https://example.test\nSECRET_KEY=abc123\n",
                expected: vec![
                    ExpectedConfig {
                        kind: ChunkKind::ConfigStanza,
                        start_line: 1,
                        end_line: 1,
                        key_path: Some("API_URL"),
                    },
                    ExpectedConfig {
                        kind: ChunkKind::ConfigStanza,
                        start_line: 2,
                        end_line: 2,
                        key_path: Some("SECRET_KEY"),
                    },
                ],
            },
            Case {
                name: "generic_fallback_full_file",
                file_name: "config.json",
                content: "[\n 1,\n 2\n]\n",
                expected: vec![ExpectedConfig {
                    kind: ChunkKind::GenericFallbackWindow,
                    start_line: 1,
                    end_line: 4,
                    key_path: None,
                }],
            },
            Case {
                name: "generic_fallback_multi_window",
                file_name: "settings.yaml",
                content: concat!(
                    "bad1\n",
                    "bad2\n",
                    "bad3\n",
                    "bad4\n",
                    "bad5\n",
                    "bad6\n",
                    "bad7\n",
                    "bad8\n",
                    "bad9\n",
                    "bad10\n",
                    "bad11\n",
                    "bad12\n",
                    "bad13\n",
                    "bad14\n",
                    "bad15\n",
                    "bad16\n",
                    "bad17\n",
                    "bad18\n",
                ),
                expected: vec![
                    ExpectedConfig {
                        kind: ChunkKind::GenericFallbackWindow,
                        start_line: 1,
                        end_line: 16,
                        key_path: None,
                    },
                    ExpectedConfig {
                        kind: ChunkKind::GenericFallbackWindow,
                        start_line: 17,
                        end_line: 18,
                        key_path: None,
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

            assert_eq!(chunks.len(), case.expected.len(), "case: {}", case.name);

            for (chunk, expected) in chunks.iter().zip(case.expected.iter()) {
                assert_eq!(chunk.kind, expected.kind, "case: {}", case.name);
                assert_eq!(chunk.start_line, expected.start_line, "case: {}", case.name);
                assert_eq!(chunk.end_line, expected.end_line, "case: {}", case.name);
                assert_eq!(chunk.key_path.as_deref(), expected.key_path, "case: {}", case.name);
                assert_eq!(chunk.language, None, "case: {}", case.name);
                assert_eq!(chunk.symbol_name, None, "case: {}", case.name);
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
