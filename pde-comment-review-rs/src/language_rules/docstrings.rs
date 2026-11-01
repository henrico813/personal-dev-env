//! This module turns a Python docstring into plain words and back, as
//! today's comment checker does. Section entries, such as one argument under
//! `Args:`, stay one line each, and examples keep their lines. Read the
//! `to_plain_words(text)` and `lay_out_plain_words(words, old_text)` functions
//! to understand the module.

use super::line_comments::{LINE_WIDTH, wrap_words};

/// Lists the headings whose lines are examples, kept as written.
const EXAMPLE_HEADINGS: [&str; 2] = ["Example:", "Examples:"];

/// Returns the docstring's words without quotes, or the text as it is when
/// it is not one whole docstring.
pub(super) fn to_plain_words(text: &str) -> String {
    match split_docstring(text) {
        Some((quotes, body)) => write_plain_parts(&read_docstring_parts(body, quotes.indent)),
        None => text.to_owned(),
    }
}

/// Returns `words` as a docstring with the indentation, prefix and quotes of
/// `old_text`, or an empty text when `words` has none.
pub(super) fn lay_out_plain_words(words: &str, old_text: &str) -> String {
    let quotes = match split_docstring(old_text) {
        Some((quotes, _)) => quotes,
        // A docstring in parentheses becomes a plain one.
        None => {
            let first_line = old_text.lines().next().unwrap_or("");
            let rest = first_line.trim_start_matches([' ', '\t']);
            Quotes {
                indent: &first_line[..first_line.len() - rest.len()],
                prefix: "",
                quotes: "\"\"\"",
            }
        }
    };
    write_docstring(&read_plain_parts(words), &quotes)
}

/// Holds the parts of a docstring around its words.
struct Quotes<'text> {
    indent: &'text str,
    prefix: &'text str,
    quotes: &'text str,
}

/// Holds one line of a docstring's plain words.
#[derive(Clone, Copy, PartialEq)]
enum PartKind {
    Paragraph,
    /// Holds a section heading, such as `Args:`.
    Heading,
    /// Holds one line of a section, such as an argument.
    Entry,
    /// Holds a line kept as written, such as an example.
    Kept,
    Blank,
}

struct Part {
    kind: PartKind,
    text: String,
}

/// Splits a whole docstring into its quotes and the words between them.
fn split_docstring(text: &str) -> Option<(Quotes<'_>, &str)> {
    let rest = text.trim_start_matches([' ', '\t']);
    let indent = &text[..text.len() - rest.len()];
    let prefix_len = usize::from(rest.starts_with(['r', 'R', 'u', 'U']));
    let (prefix, rest) = rest.split_at(prefix_len);
    let quotes = ["\"\"\"", "'''", "\"", "'"]
        .into_iter()
        .find(|quotes| rest.starts_with(quotes))?;
    let rest = rest.strip_suffix('\n').unwrap_or(rest);
    let body = rest
        .trim_end_matches([' ', '\t'])
        .strip_prefix(quotes)?
        .strip_suffix(quotes)?;
    let quotes = Quotes {
        indent,
        prefix,
        quotes,
    };
    Some((quotes, body))
}

/// Returns the parts of a docstring's words.
fn read_docstring_parts(body: &str, indent: &str) -> Vec<Part> {
    let mut parts: Vec<Part> = Vec::new();
    // Lines deeper than this follow an example heading or a line ending in `::`.
    let mut example_indent: Option<usize> = None;
    let mut entry_indent: Option<usize> = None;
    let mut in_section = false;
    // A doctest's lines and its expected output stay whole up to a blank line.
    let mut in_doctest = false;
    for (number, line) in body.split('\n').enumerate() {
        // The first line starts after the quotes, so it has no indent.
        let line = if number == 0 {
            line
        } else {
            line.strip_prefix(indent).unwrap_or(line)
        };
        let stripped = line.trim();
        let depth = line.len() - line.trim_start_matches([' ', '\t']).len();
        if stripped.is_empty() {
            in_section = false;
            in_doctest = false;
            entry_indent = None;
            parts.push(make_part(PartKind::Blank, ""));
            continue;
        }
        if example_indent.is_some_and(|example| depth <= example) {
            example_indent = None;
        }
        in_doctest |= stripped.starts_with(">>>");
        if example_indent.is_some() || in_doctest || is_kept_line(line) {
            parts.push(make_part(PartKind::Kept, line.trim_end()));
            continue;
        }
        if depth == 0 && is_section_heading(stripped) {
            parts.push(make_part(PartKind::Heading, stripped));
            in_section = !EXAMPLE_HEADINGS.contains(&stripped);
            example_indent = if in_section { None } else { Some(0) };
            continue;
        }
        let words = stripped.split_whitespace().collect::<Vec<_>>().join(" ");
        let last_kind = parts.last().map(|part| part.kind);
        if in_section && entry_indent.is_none_or(|entry| depth <= entry) {
            entry_indent = Some(depth);
            parts.push(make_part(PartKind::Entry, &words));
        } else if matches!(last_kind, Some(PartKind::Entry | PartKind::Paragraph)) {
            let last = parts.last_mut().expect("the last part exists");
            last.text = format!("{} {words}", last.text);
        } else if in_section {
            parts.push(make_part(PartKind::Entry, &words));
        } else {
            parts.push(make_part(PartKind::Paragraph, &words));
        }
        if stripped.ends_with("::") {
            example_indent = Some(depth);
        }
    }
    trim_blank_parts(parts)
}

fn make_part(kind: PartKind, text: &str) -> Part {
    Part {
        kind,
        text: text.to_owned(),
    }
}

/// Returns true for a line whose line breaks must stay. Joining the line
/// after a backslash would leave a backslash before a space.
fn is_kept_line(line: &str) -> bool {
    let stripped = line.trim();
    stripped.starts_with(">>>") || stripped.starts_with("...") || stripped.ends_with('\\')
}

/// Returns true for a section heading such as `Args:` or `Other Parameters:`.
fn is_section_heading(line: &str) -> bool {
    let Some(heading) = line.strip_suffix(':') else {
        return false;
    };
    let mut words = heading.split(' ');
    let first = words.next().unwrap_or("");
    let second = words.next();
    words.next().is_none()
        && is_heading_word(first, true)
        && second.is_none_or(|word| is_heading_word(word, false))
}

/// Returns true for a letter and lowercase letters, with a capital first
/// when `capital` is true.
fn is_heading_word(word: &str, capital: bool) -> bool {
    let mut letters = word.chars();
    let Some(first) = letters.next() else {
        return false;
    };
    let rest = letters.as_str();
    (first.is_ascii_uppercase() || (!capital && first.is_ascii_lowercase()))
        && !rest.is_empty()
        && rest.chars().all(|letter| letter.is_ascii_lowercase())
}

/// Returns `parts` with no blank part first, last or twice in a row.
fn trim_blank_parts(parts: Vec<Part>) -> Vec<Part> {
    let mut trimmed: Vec<Part> = Vec::new();
    for part in parts {
        let after_words = trimmed
            .last()
            .is_some_and(|last| last.kind != PartKind::Blank);
        if part.kind != PartKind::Blank || after_words {
            trimmed.push(part);
        }
    }
    if trimmed
        .last()
        .is_some_and(|last| last.kind == PartKind::Blank)
    {
        trimmed.pop();
    }
    trimmed
}

/// Returns the plain words of a docstring's parts, one line each.
fn write_plain_parts(parts: &[Part]) -> String {
    if parts.is_empty() {
        return "\n".to_owned();
    }
    parts
        .iter()
        .map(|part| format!("{}\n", part.text))
        .collect()
}

/// Returns the parts of plain words the user or agent edited.
fn read_plain_parts(words: &str) -> Vec<Part> {
    let mut parts: Vec<Part> = Vec::new();
    let mut in_section = false;
    let mut in_doctest = false;
    for line in words.split('\n') {
        let stripped = line.trim();
        let joined = stripped.split_whitespace().collect::<Vec<_>>().join(" ");
        in_doctest = !stripped.is_empty() && (in_doctest || stripped.starts_with(">>>"));
        if stripped.is_empty() {
            in_section = false;
            parts.push(make_part(PartKind::Blank, ""));
        } else if line.starts_with('\t')
            || line.starts_with("  ")
            || in_doctest
            || is_kept_line(line)
        {
            parts.push(make_part(PartKind::Kept, line.trim_end()));
        } else if is_section_heading(stripped) {
            parts.push(make_part(PartKind::Heading, stripped));
            in_section = !EXAMPLE_HEADINGS.contains(&stripped);
        } else if in_section {
            parts.push(make_part(PartKind::Entry, &joined));
        } else if let Some(last) = parts.last_mut()
            && last.kind == PartKind::Paragraph
        {
            last.text = format!("{} {joined}", last.text);
        } else {
            parts.push(make_part(PartKind::Paragraph, &joined));
        }
    }
    trim_blank_parts(parts)
}

/// Returns `parts` as a docstring. Paragraphs wrap, and section entries are
/// indented four spaces, with their later lines four more. One short
/// paragraph stays on one line.
fn write_docstring(parts: &[Part], quotes: &Quotes) -> String {
    let Some(first) = parts.first() else {
        return String::new();
    };
    let Quotes { indent, prefix, .. } = quotes;
    let quote = &quotes.quotes[..1];
    let triple = quote.repeat(3);
    let width = LINE_WIDTH.saturating_sub(indent.replace('\t', "    ").chars().count());
    let summary = &first.text;
    // A last quote or backslash would join the closing quotes.
    if parts.len() == 1
        && first.kind == PartKind::Paragraph
        && !summary.ends_with(quote)
        && !summary.ends_with('\\')
    {
        let one_line_quotes = if summary.contains(quote) {
            &triple
        } else {
            quotes.quotes
        };
        let line = format!("{prefix}{one_line_quotes}{summary}{one_line_quotes}");
        if line.chars().count() <= width && !summary.contains(&triple) {
            return format!("{indent}{line}\n");
        }
    }
    let mut lines = vec![format!("{indent}{prefix}{triple}")];
    for (number, part) in parts.iter().enumerate() {
        match part.kind {
            PartKind::Paragraph => {
                let opening = if number == 0 {
                    prefix.len() + triple.len()
                } else {
                    0
                };
                let mut wrapped =
                    wrap_first_and_rest(&part.text, width.saturating_sub(opening), width)
                        .into_iter();
                if number == 0 {
                    lines[0].push_str(&wrapped.next().unwrap_or_default());
                }
                lines.extend(wrapped.map(|line| format!("{indent}{line}")));
            }
            PartKind::Entry => {
                let wrapped = wrap_first_and_rest(
                    &part.text,
                    width.saturating_sub(4),
                    width.saturating_sub(8),
                );
                for (index, line) in wrapped.iter().enumerate() {
                    let depth = if index == 0 { "    " } else { "        " };
                    lines.push(format!("{indent}{depth}{line}"));
                }
            }
            PartKind::Heading | PartKind::Kept => lines.push(format!("{indent}{}", part.text)),
            PartKind::Blank => lines.push(String::new()),
        }
    }
    lines.push(format!("{indent}{triple}"));
    lines.iter().map(|line| format!("{line}\n")).collect()
}

/// Wraps `words` with the first line at most `first` wide and the rest at
/// most `rest`. A backslash before a space, and a named escape such as
/// `\N{EM DASH}`, stay on one line, since Python reads them differently
/// when split.
fn wrap_first_and_rest(words: &str, first: usize, rest: usize) -> Vec<String> {
    // Plain words never hold a NUL, so one stands for a space that must not break.
    let mut protected = words.replace("\\ ", "\\\0");
    let mut search_from = 0;
    while let Some(start) = protected[search_from..].find("\\N{") {
        let start = search_from + start;
        let Some(length) = protected[start..].find('}') else {
            break;
        };
        let end = start + length;
        let escape = protected[start..end].replace(' ', "\0");
        protected.replace_range(start..end, &escape);
        search_from = end;
    }
    let mut lines = wrap_words(&protected, first);
    if lines.len() > 1 {
        let later = lines.split_off(1).join(" ");
        lines.extend(wrap_words(&later, rest));
    }
    lines.iter().map(|line| line.replace('\0', " ")).collect()
}
