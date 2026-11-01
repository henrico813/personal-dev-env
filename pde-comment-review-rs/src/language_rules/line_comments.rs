//! Turns `//` and `#` comment lines into plain words and back, as today's
//! comment checker does.

use std::mem;

/// Limits how far a rewritten comment's lines reach, counting indentation.
pub(super) const LINE_WIDTH: usize = 79;

/// Lists the marks that open and close a code fence.
const FENCE_MARKS: [&str; 2] = ["```", "~~~"];

/// Holds comment lines that plain words keep together.
enum Block {
    /// Holds sentences joined into one line of words.
    Words(String),
    /// Holds lines that start with a tab or two spaces, kept as written.
    Example(Vec<String>),
    /// Holds a code fence and the lines inside it, kept as written.
    Fence(Vec<String>),
}

/// Returns the comment's words without `marker`, or the text as it is when a
/// line does not start with `marker`.
pub(super) fn to_plain_words(text: &str, marker: &str) -> String {
    let mut bodies = Vec::new();
    for line in text.lines() {
        let Some(body) = line.trim_start().strip_prefix(marker) else {
            return text.to_owned();
        };
        bodies.push(body);
    }
    let blocks: Vec<String> = group_blocks(&bodies)
        .into_iter()
        .map(|block| match block {
            Block::Words(words) => words,
            Block::Example(lines) | Block::Fence(lines) => lines.join("\n"),
        })
        .collect();
    blocks.join("\n\n") + "\n"
}

/// Returns the comment's paragraphs of sentences, each on one line, or none
/// when a line does not start with `marker`.
pub(super) fn to_paragraphs(text: &str, marker: &str) -> Vec<String> {
    let mut bodies = Vec::new();
    for line in text.lines() {
        let Some(body) = line.trim_start().strip_prefix(marker) else {
            return Vec::new();
        };
        bodies.push(body);
    }
    group_blocks(&bodies)
        .into_iter()
        .filter_map(|block| match block {
            Block::Words(words) => Some(words),
            Block::Example(_) | Block::Fence(_) => None,
        })
        .collect()
}

/// Returns `words` as comment lines with the indentation of `old_text`, or
/// an empty text when `words` has none.
pub(super) fn lay_out_plain_words(words: &str, old_text: &str, marker: &str) -> String {
    let first_line = old_text.lines().next().unwrap_or("");
    let indent = &first_line[..first_line.len() - first_line.trim_start_matches([' ', '\t']).len()];
    let indent_width = indent.replace('\t', "    ").chars().count();
    let width = LINE_WIDTH.saturating_sub(indent_width + marker.len() + 1);
    let mut lines = Vec::new();
    for block in group_blocks(&words.lines().collect::<Vec<_>>()) {
        if !lines.is_empty() {
            lines.push(format!("{indent}{marker}"));
        }
        match block {
            Block::Words(words) => {
                for line in wrap_words(&words, width) {
                    lines.push(format!("{indent}{marker} {line}"));
                }
            }
            Block::Example(kept) => {
                for line in kept {
                    lines.push(format!("{indent}{marker}{line}"));
                }
            }
            Block::Fence(kept) => {
                for line in kept {
                    let space = if line.is_empty() { "" } else { " " };
                    lines.push(format!("{indent}{marker}{space}{line}"));
                }
            }
        }
    }
    lines.iter().map(|line| format!("{line}\n")).collect()
}

/// Splits comment lines, with their markers removed, into blocks. A blank
/// line, or a switch between sentences and kept lines, starts a new block.
fn group_blocks(bodies: &[&str]) -> Vec<Block> {
    let mut blocks = Vec::new();
    let mut words: Vec<&str> = Vec::new();
    let mut examples: Vec<String> = Vec::new();
    // A fence closes only with the mark that opened it.
    let mut fence: Option<(&str, Vec<String>)> = None;
    for body in bodies {
        let fence_mark = FENCE_MARKS
            .into_iter()
            .find(|mark| body.trim_start().starts_with(mark));
        if let Some((open_mark, lines)) = &mut fence {
            lines.push(strip_one_space(body));
            if fence_mark == Some(*open_mark) {
                blocks.push(Block::Fence(mem::take(lines)));
                fence = None;
            }
            continue;
        }
        let is_fence_mark = fence_mark.is_some();
        let is_blank = body.trim().is_empty();
        let is_example =
            !is_blank && !is_fence_mark && (body.starts_with('\t') || body.starts_with("  "));
        if !words.is_empty() && (is_blank || is_example || is_fence_mark) {
            blocks.push(Block::Words(words.join(" ")));
            words.clear();
        }
        if !examples.is_empty() && !is_example {
            blocks.push(Block::Example(mem::take(&mut examples)));
        }
        if let Some(mark) = fence_mark {
            fence = Some((mark, vec![strip_one_space(body)]));
        } else if is_example {
            examples.push(body.trim_end().to_owned());
        } else if !is_blank {
            words.extend(body.split_whitespace());
        }
    }
    if !words.is_empty() {
        blocks.push(Block::Words(words.join(" ")));
    }
    if !examples.is_empty() {
        blocks.push(Block::Example(examples));
    }
    if let Some((_, lines)) = fence {
        blocks.push(Block::Fence(lines));
    }
    blocks
}

/// Returns a fence line without the space after its marker.
fn strip_one_space(body: &str) -> String {
    body.strip_prefix(' ').unwrap_or(body).trim_end().to_owned()
}

/// Joins words with single spaces into lines at most `width` wide. A word
/// longer than `width` gets a line of its own.
pub(super) fn wrap_words(words: &str, width: usize) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for word in words.split_whitespace() {
        match lines.last_mut() {
            Some(line) if line.chars().count() + 1 + word.chars().count() <= width => {
                line.push(' ');
                line.push_str(word);
            }
            _ => lines.push(word.to_owned()),
        }
    }
    lines
}
