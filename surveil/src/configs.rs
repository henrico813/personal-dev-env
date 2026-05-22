use super::{build_fixed_windows, make_chunk, slice_lines, Chunk, ChunkKind, CONFIG_WINDOW_LINES};
use crate::source::SourceFile;

pub(super) fn config_format(path: &std::path::Path) -> Option<ConfigFormat> {
    let file_name = path.file_name()?.to_str()?;
    if file_name == ".env" || file_name.starts_with(".env.") {
        return Some(ConfigFormat::Env);
    }

    match path.extension().and_then(|ext| ext.to_str()) {
        Some("toml") => Some(ConfigFormat::Toml),
        Some("json") => Some(ConfigFormat::Json),
        Some("yaml" | "yml") => Some(ConfigFormat::Yaml),
        Some("ini" | "cfg" | "conf") => Some(ConfigFormat::Ini),
        Some("env") => Some(ConfigFormat::Env),
        _ => None,
    }
}

pub(super) fn build_config_chunks(
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

    if chunks.is_empty() {
        return build_fixed_windows(
            source,
            text,
            line_starts,
            &full_file,
            ChunkKind::GenericFallbackWindow,
            None,
            CONFIG_WINDOW_LINES,
        );
    }

    chunks.sort_by_key(|chunk| (chunk.start_line, chunk.end_line));
    chunks
}

#[derive(Clone, Copy)]
pub(super) enum ConfigFormat {
    Toml,
    Json,
    Yaml,
    Ini,
    Env,
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
