use super::{make_chunk, slice_lines, Chunk, ChunkKind};
use crate::source::SourceFile;
use std::path::Path;

pub(super) fn is_markdown(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|ext| ext.to_str()),
        Some("md" | "markdown" | "mdx")
    )
}

pub(super) fn build_markdown_chunks(
    source: &SourceFile,
    text: &str,
    line_starts: &[usize],
) -> Vec<Chunk> {
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
                if next.is_empty()
                    || markdown_heading(next).is_some()
                    || fence_marker(next).is_some()
                {
                    break;
                }
                if is_list_line(next)
                    || lines[index].starts_with(' ')
                    || lines[index].starts_with('\t')
                {
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

#[cfg(test)]
mod tests {
    use super::super::{build_chunks, temp_repo, write_file, ChunkKind};
    use crate::source::SourceFile;
    use std::fs;

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
            Case { name: "paragraph_list_and_fence_blocks", content: "# Title\n\n## Setup\nFirst paragraph.\n\n- one\n- two\n\n```rust\nfn attach() {}\n```\n", expected: vec![ExpectedMarkdown { start_line: 4, end_line: 4, text: "First paragraph.\n", section_path: &["Title", "Setup"] }, ExpectedMarkdown { start_line: 6, end_line: 7, text: "- one\n- two\n", section_path: &["Title", "Setup"] }, ExpectedMarkdown { start_line: 9, end_line: 11, text: "```rust\nfn attach() {}\n```\n", section_path: &["Title", "Setup"] }] },
            Case { name: "headings_split_paragraphs_without_heading_chunks", content: "# One\nalpha\n\n## Two\nbeta\n", expected: vec![ExpectedMarkdown { start_line: 2, end_line: 2, text: "alpha\n", section_path: &["One"] }, ExpectedMarkdown { start_line: 5, end_line: 5, text: "beta\n", section_path: &["One", "Two"] }] },
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
                assert_eq!(
                    chunk.section_path, expected_section_path,
                    "case: {}",
                    case.name
                );
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
