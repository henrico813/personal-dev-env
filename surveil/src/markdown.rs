use super::{make_chunk, slice_lines, Chunk, ChunkKind};
use crate::source::SourceFile;

pub(super) fn is_markdown(path: &std::path::Path) -> bool {
    matches!(
        path.extension().and_then(|ext| ext.to_str()),
        Some("md" | "markdown" | "mdx")
    )
}

pub(super) fn build_markdown_chunks(source: &SourceFile, text: &str, line_starts: &[usize]) -> Vec<Chunk> {
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
