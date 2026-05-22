use super::{build_fixed_windows, make_chunk, slice_lines, Chunk, ChunkKind, CONFIG_WINDOW_LINES};
use crate::source::SourceFile;
use std::path::Path;

#[derive(Clone, Copy)]
pub(super) enum ConfigFormat {
    Toml,
    Json,
    Yaml,
    Ini,
    Env,
}

enum ConfigParseResult {
    Parsed(Vec<Chunk>),
    Fallback,
}

pub(super) fn config_format(path: &Path) -> Option<ConfigFormat> {
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

pub(super) fn build_config_chunks(source: &SourceFile, text: &str, line_starts: &[usize], format: ConfigFormat) -> Vec<Chunk> {
    match parse_config_chunks(source, text, line_starts, format) {
        ConfigParseResult::Parsed(mut chunks) => {
            if chunks.is_empty() {
                return fallback_config_windows(source, text, line_starts);
            }
            chunks.sort_by_key(|chunk| (chunk.start_line, chunk.end_line));
            chunks
        }
        ConfigParseResult::Fallback => fallback_config_windows(source, text, line_starts),
    }
}

fn fallback_config_windows(source: &SourceFile, text: &str, line_starts: &[usize]) -> Vec<Chunk> {
    let total_lines = line_starts.len() as u32;
    let full_file = [(1, total_lines)];
    build_fixed_windows(source, text, line_starts, &full_file, ChunkKind::GenericFallbackWindow, None, CONFIG_WINDOW_LINES)
}

fn parse_config_chunks(source: &SourceFile, text: &str, line_starts: &[usize], format: ConfigFormat) -> ConfigParseResult {
    match format {
        ConfigFormat::Toml => parse_toml_or_ini_chunks(source, text, line_starts, true),
        ConfigFormat::Json => parse_json_chunks(source, text, line_starts),
        ConfigFormat::Yaml => parse_yaml_chunks(source, text, line_starts),
        ConfigFormat::Ini => parse_toml_or_ini_chunks(source, text, line_starts, false),
        ConfigFormat::Env => parse_env_chunks(source, text, line_starts),
    }
}

fn parse_toml_or_ini_chunks(source: &SourceFile, text: &str, line_starts: &[usize], is_toml: bool) -> ConfigParseResult {
    let lines: Vec<&str> = text.lines().collect();
    let mut chunks = Vec::new();
    let mut section_path: Vec<String> = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        let start_line = index as u32 + 1;
        let trimmed = line.trim();
        if trimmed.is_empty() || is_config_comment(trimmed, !is_toml) { continue; }
        if trimmed.starts_with("[[") || trimmed.starts_with(']') { return ConfigParseResult::Fallback; }
        if let Some(section) = parse_bracket_section(trimmed) {
            section_path = section.clone();
            chunks.push(make_chunk(source, ChunkKind::ConfigStanza, start_line, start_line, slice_lines(text, line_starts, start_line, start_line), None, None, Vec::new(), Some(&join_key_path(&section))));
            continue;
        }
        let Some((key, value)) = trimmed.split_once('=') else { return ConfigParseResult::Fallback; };
        let key = key.trim();
        let value = value.trim();
        let Some(key_path) = split_key_path(key) else { return ConfigParseResult::Fallback; };
        if key_path.is_empty() || value.is_empty() || value.starts_with('[') || value.starts_with('{') { return ConfigParseResult::Fallback; }
        let mut full_path = section_path.clone();
        full_path.extend(key_path);
        chunks.push(make_chunk(source, ChunkKind::ConfigStanza, start_line, start_line, slice_lines(text, line_starts, start_line, start_line), None, None, Vec::new(), Some(&join_key_path(&full_path))));
    }
    ConfigParseResult::Parsed(chunks)
}

fn parse_yaml_chunks(source: &SourceFile, text: &str, line_starts: &[usize]) -> ConfigParseResult {
    let lines: Vec<&str> = text.lines().collect();
    let mut chunks = Vec::new();
    let mut path_stack: Vec<(usize, String)> = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        let start_line = index as u32 + 1;
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') { continue; }
        if trimmed.starts_with('-') || trimmed.starts_with('[') || trimmed.starts_with('{') { return ConfigParseResult::Fallback; }
        let indent = line.chars().take_while(|ch| ch.is_whitespace()).count();
        while path_stack.last().is_some_and(|(stack_indent, _)| *stack_indent >= indent) { path_stack.pop(); }
        let Some((raw_key, raw_value)) = trimmed.split_once(':') else { return ConfigParseResult::Fallback; };
        let Some(key) = normalize_key(raw_key) else { return ConfigParseResult::Fallback; };
        let value = raw_value.trim();
        let mut full_path = current_key_path(&path_stack);
        full_path.push(key.clone());
        chunks.push(make_chunk(source, ChunkKind::ConfigStanza, start_line, start_line, slice_lines(text, line_starts, start_line, start_line), None, None, Vec::new(), Some(&join_key_path(&full_path))));
        if value.is_empty() { path_stack.push((indent, key)); } else if value.starts_with('[') || value.starts_with('{') || value == "|" || value == ">" { return ConfigParseResult::Fallback; }
    }
    ConfigParseResult::Parsed(chunks)
}

fn parse_json_chunks(source: &SourceFile, text: &str, line_starts: &[usize]) -> ConfigParseResult {
    let lines: Vec<&str> = text.lines().collect();
    let mut chunks = Vec::new();
    let mut path_stack: Vec<String> = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        let start_line = index as u32 + 1;
        let trimmed = line.trim();
        if trimmed.is_empty() { continue; }
        if trimmed == "}" || trimmed == "}," { path_stack.pop(); continue; }
        if trimmed == "{" || trimmed == "{," { continue; }
        if trimmed.contains('[') || trimmed.contains(']') { return ConfigParseResult::Fallback; }
        let Some((key, value)) = parse_json_property(trimmed) else { return ConfigParseResult::Fallback; };
        let value = value.trim_end_matches(',').trim();
        let mut full_path = path_stack.clone();
        full_path.push(key.clone());
        chunks.push(make_chunk(source, ChunkKind::ConfigStanza, start_line, start_line, slice_lines(text, line_starts, start_line, start_line), None, None, Vec::new(), Some(&join_key_path(&full_path))));
        if value == "{" { path_stack.push(key); } else if value.starts_with('{') || value.starts_with('[') { return ConfigParseResult::Fallback; }
    }
    ConfigParseResult::Parsed(chunks)
}

fn parse_env_chunks(source: &SourceFile, text: &str, line_starts: &[usize]) -> ConfigParseResult {
    let lines: Vec<&str> = text.lines().collect();
    let mut chunks = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        let start_line = index as u32 + 1;
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') { continue; }
        let stripped = trimmed.strip_prefix("export ").unwrap_or(trimmed);
        let Some((key, _value)) = stripped.split_once('=') else { return ConfigParseResult::Fallback; };
        let key = key.trim();
        if key.is_empty() || !key.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '_') { return ConfigParseResult::Fallback; }
        chunks.push(make_chunk(source, ChunkKind::ConfigStanza, start_line, start_line, slice_lines(text, line_starts, start_line, start_line), None, None, Vec::new(), Some(key)));
    }
    ConfigParseResult::Parsed(chunks)
}

fn parse_bracket_section(trimmed: &str) -> Option<Vec<String>> {
    let content = trimmed.strip_prefix('[')?.strip_suffix(']')?.trim();
    if content.is_empty() || content.contains('[') || content.contains(']') { return None; }
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

fn is_config_comment(trimmed: &str, allow_semicolon: bool) -> bool { trimmed.starts_with('#') || (allow_semicolon && trimmed.starts_with(';')) }
fn split_key_path(raw: &str) -> Option<Vec<String>> { let parts: Vec<String> = raw.split('.').map(str::trim).filter(|part| !part.is_empty()).map(str::to_string).collect(); if parts.is_empty() { None } else { Some(parts) } }
fn normalize_key(raw: &str) -> Option<String> { let key = raw.trim(); if key.is_empty() { return None; } if key.len() >= 2 { let bytes = key.as_bytes(); if (bytes[0] == b'"' && bytes[key.len() - 1] == b'"') || (bytes[0] == b'\'' && bytes[key.len() - 1] == b'\'') { return Some(key[1..key.len() - 1].to_string()); } } Some(key.to_string()) }
fn join_key_path(parts: &[String]) -> String { parts.join(".") }
fn current_key_path(path_stack: &[(usize, String)]) -> Vec<String> { path_stack.iter().map(|(_, key)| key.clone()).collect() }

#[cfg(test)]
mod tests {
    use super::super::{build_chunks, temp_repo, write_file, ChunkKind};
    use crate::source::SourceFile;
    use std::fs;

    #[test]
    fn build_chunks_emits_expected_config_chunks() {
        struct ExpectedConfig<'a> { kind: ChunkKind, start_line: u32, end_line: u32, key_path: Option<&'a str> }
        struct Case<'a> { name: &'a str, file_name: &'a str, content: &'a str, expected: Vec<ExpectedConfig<'a>> }
        let cases = [
            Case { name: "toml_nested_paths", file_name: "config.toml", content: "[server]\nhost = \"localhost\"\nport = 8080\n", expected: vec![ExpectedConfig { kind: ChunkKind::ConfigStanza, start_line: 1, end_line: 1, key_path: Some("server") }, ExpectedConfig { kind: ChunkKind::ConfigStanza, start_line: 2, end_line: 2, key_path: Some("server.host") }, ExpectedConfig { kind: ChunkKind::ConfigStanza, start_line: 3, end_line: 3, key_path: Some("server.port") }] },
            Case { name: "yaml_nested_paths", file_name: "config.yaml", content: "server:\n  host: localhost\n  database:\n    port: 5432\n", expected: vec![ExpectedConfig { kind: ChunkKind::ConfigStanza, start_line: 1, end_line: 1, key_path: Some("server") }, ExpectedConfig { kind: ChunkKind::ConfigStanza, start_line: 2, end_line: 2, key_path: Some("server.host") }, ExpectedConfig { kind: ChunkKind::ConfigStanza, start_line: 3, end_line: 3, key_path: Some("server.database") }, ExpectedConfig { kind: ChunkKind::ConfigStanza, start_line: 4, end_line: 4, key_path: Some("server.database.port") }] },
            Case { name: "json_nested_paths", file_name: "config.json", content: "{\n  \"server\": {\n    \"host\": \"localhost\",\n    \"port\": 8080\n  }\n}\n", expected: vec![ExpectedConfig { kind: ChunkKind::ConfigStanza, start_line: 2, end_line: 2, key_path: Some("server") }, ExpectedConfig { kind: ChunkKind::ConfigStanza, start_line: 3, end_line: 3, key_path: Some("server.host") }, ExpectedConfig { kind: ChunkKind::ConfigStanza, start_line: 4, end_line: 4, key_path: Some("server.port") }] },
            Case { name: "env_keys", file_name: ".env.local", content: "API_URL=https://example.test\nSECRET_KEY=abc123\n", expected: vec![ExpectedConfig { kind: ChunkKind::ConfigStanza, start_line: 1, end_line: 1, key_path: Some("API_URL") }, ExpectedConfig { kind: ChunkKind::ConfigStanza, start_line: 2, end_line: 2, key_path: Some("SECRET_KEY") }] },
            Case { name: "env_extension_keys", file_name: "config/settings.env", content: "APP_HOST=localhost\nAPP_PORT=8080\n", expected: vec![ExpectedConfig { kind: ChunkKind::ConfigStanza, start_line: 1, end_line: 1, key_path: Some("APP_HOST") }, ExpectedConfig { kind: ChunkKind::ConfigStanza, start_line: 2, end_line: 2, key_path: Some("APP_PORT") }] },
            Case { name: "generic_fallback_full_file", file_name: "config.json", content: "[\n 1,\n 2\n]\n", expected: vec![ExpectedConfig { kind: ChunkKind::GenericFallbackWindow, start_line: 1, end_line: 4, key_path: None }] },
            Case { name: "generic_fallback_multi_window", file_name: "settings.yaml", content: concat!("bad1\n","bad2\n","bad3\n","bad4\n","bad5\n","bad6\n","bad7\n","bad8\n","bad9\n","bad10\n","bad11\n","bad12\n","bad13\n","bad14\n","bad15\n","bad16\n","bad17\n","bad18\n","bad19\n","bad20\n","bad21\n","bad22\n","bad23\n","bad24\n","bad25\n","bad26\n","bad27\n","bad28\n","bad29\n","bad30\n","bad31\n","bad32\n","bad33\n","bad34\n","bad35\n","bad36\n","bad37\n","bad38\n","bad39\n","bad40\n","bad41\n"), expected: vec![ExpectedConfig { kind: ChunkKind::GenericFallbackWindow, start_line: 1, end_line: 40, key_path: None }, ExpectedConfig { kind: ChunkKind::GenericFallbackWindow, start_line: 41, end_line: 41, key_path: None }] },
            Case { name: "comment_only_env_falls_back", file_name: ".env", content: "# comment\n# still comment\n", expected: vec![ExpectedConfig { kind: ChunkKind::GenericFallbackWindow, start_line: 1, end_line: 2, key_path: None }] },
            Case { name: "comment_only_yaml_falls_back", file_name: "config/settings.yaml", content: "# comment\n# still comment\n", expected: vec![ExpectedConfig { kind: ChunkKind::GenericFallbackWindow, start_line: 1, end_line: 2, key_path: None }] },
            Case { name: "whitespace_only_json_falls_back", file_name: "config/settings.json", content: "   \n\t\n", expected: vec![ExpectedConfig { kind: ChunkKind::GenericFallbackWindow, start_line: 1, end_line: 2, key_path: None }] },
            Case { name: "empty_json_object_falls_back", file_name: "config/settings.json", content: "{\n}\n", expected: vec![ExpectedConfig { kind: ChunkKind::GenericFallbackWindow, start_line: 1, end_line: 2, key_path: None }] },
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
}
