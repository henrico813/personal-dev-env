use crate::source::SourceFile;
#[cfg(test)]
use std::path::PathBuf;
#[cfg(test)]
use std::time::{SystemTime, UNIX_EPOCH};
#[cfg(test)]
use std::{fs, io::Write};

mod code;
mod configs;
mod markdown;

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

pub(super) const CODE_WINDOW_LINES: u32 = 20;
pub(super) const CONFIG_WINDOW_LINES: u32 = 40;

pub(crate) fn build_chunks(source: &SourceFile, text: &str) -> Vec<Chunk> {
    if text.is_empty() {
        return Vec::new();
    }

    let line_starts = line_starts(text);
    if markdown::is_markdown(source.path()) {
        return markdown::build_markdown_chunks(source, text, &line_starts);
    }

    if let Some(config_format) = configs::config_format(source.path()) {
        return configs::build_config_chunks(source, text, &line_starts, config_format);
    }

    code::build_code_chunks(source, text, &line_starts)
}

pub(super) fn line_starts(text: &str) -> Vec<usize> {
    let mut line_starts = vec![0];
    for (offset, ch) in text.char_indices() {
        if ch == '\n' && offset + 1 < text.len() {
            line_starts.push(offset + 1);
        }
    }
    line_starts
}

pub(super) fn slice_lines(text: &str, line_starts: &[usize], start_line: u32, end_line: u32) -> String {
    let start = line_starts[start_line as usize - 1];
    let end = if (end_line as usize) < line_starts.len() {
        line_starts[end_line as usize]
    } else {
        text.len()
    };
    text[start..end].to_string()
}

pub(super) fn make_chunk(
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

pub(super) fn build_fixed_windows(
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
pub(crate) fn temp_repo(name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("surveil-chunks-{name}-{stamp}"));
    fs::create_dir_all(&path).expect("create temp repo");
    path
}

#[cfg(test)]
pub(crate) fn write_file(path: &PathBuf, content: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent dirs");
    }
    let mut file = fs::File::create(path).expect("create file");
    file.write_all(content.as_bytes()).expect("write file");
}
