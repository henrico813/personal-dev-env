//! Finds Go's `//` and `/* */` comments, though today's comment checker
//! skips `/* */`. Other comments keep that checker's text, except around
//! a tool line, so older approvals still match.

use tree_sitter::{Language, Node};

use super::LanguageRules;
use crate::shared::comments::CommentStyle;

pub struct GoRules;

impl LanguageRules for GoRules {
    fn grammar(&self) -> Language {
        tree_sitter_go::LANGUAGE.into()
    }

    fn classify_node(&self, node: Node, source: &str) -> Option<CommentStyle> {
        if node.kind() != "comment" {
            return None;
        }
        let text = &source[node.byte_range()];
        if text.starts_with("/*") {
            Some(CommentStyle::Block)
        } else if is_tool_line(text) {
            None
        } else {
            Some(CommentStyle::Line)
        }
    }

    fn name_comment(
        &self,
        _comment: Node,
        code_below: Option<Node>,
        source: &str,
    ) -> Option<String> {
        let code = code_below?;
        let line = read_line_from(source, code.start_byte());
        if code.start_position().column == 0 {
            return name_top_level_line(code, line, source);
        }
        let word = find_declared_word(line)?;
        let declaration = find_top_level(code);
        let declaration_line = read_line_from(source, declaration.start_byte());
        match declaration.kind() {
            "function_declaration" | "method_declaration" => None,
            _ if find_block_keyword(declaration_line).is_some() => Some(word.to_string()),
            // A struct field or interface method takes its type's name.
            _ => {
                let outer = find_declared_word(declaration_line).unwrap_or("");
                Some(format!("{outer}.{word}"))
            }
        }
    }
}

/// Names the code on a line that starts at column 0.
fn name_top_level_line(code: Node, line: &str, source: &str) -> Option<String> {
    if code.kind() == "method_declaration" {
        return name_method(code, source);
    }
    if let Some(keyword) = find_block_keyword(line) {
        // A block takes the name of its first entry, after `const` or `var`.
        let line_end = code.start_byte() + line.len();
        let first_entry = source[line_end..]
            .lines()
            .map(str::trim)
            .find(|entry| !entry.is_empty() && !entry.starts_with("//"))?;
        return Some(format!("{keyword} {}", find_declared_word(first_entry)?));
    }
    find_declared_word(line).map(String::from)
}

/// Returns a method's receiver type and name, joined by a dot.
fn name_method(method: Node, source: &str) -> Option<String> {
    let receiver = method.child_by_field_name("receiver")?.named_child(0)?;
    let receiver_type = receiver.child_by_field_name("type")?;
    let type_text = source[receiver_type.byte_range()].trim_start_matches('*');
    let type_name = find_leading_word(type_text.trim_start())?;
    let name = method.child_by_field_name("name")?;
    Some(format!("{type_name}.{}", &source[name.byte_range()]))
}

/// Returns the top-level declaration that holds `node`.
fn find_top_level(node: Node) -> Node {
    let mut top = node;
    while let Some(parent) = top.parent()
        && parent.parent().is_some()
    {
        top = parent;
    }
    top
}

/// Returns the rest of the line from `start`, without its newline.
fn read_line_from(source: &str, start: usize) -> &str {
    source[start..].lines().next().unwrap_or("")
}

/// Returns `const` or `var` when the line opens a block of them.
fn find_block_keyword(line: &str) -> Option<&'static str> {
    ["const", "var"].into_iter().find(|keyword| {
        line.strip_prefix(keyword)
            .is_some_and(|rest| rest.trim() == "(")
    })
}

/// Returns the name a line declares, read as today's comment checker
/// reads it, so older approvals still match.
fn find_declared_word(line: &str) -> Option<&str> {
    for keyword in ["func", "type", "const", "var", "package"] {
        if let Some(rest) = line.strip_prefix(keyword)
            && rest.starts_with(char::is_whitespace)
            && let Some(word) = find_leading_word(rest.trim_start())
        {
            return Some(word);
        }
    }
    find_leading_word(line)
}

/// Returns the letters, digits and underscores that start `text`.
fn find_leading_word(text: &str) -> Option<&str> {
    let end = text
        .find(|c: char| !c.is_alphanumeric() && c != '_')
        .unwrap_or(text.len());
    if end == 0 { None } else { Some(&text[..end]) }
}

/// Returns true for a line Go's tools read, such as `//go:generate`,
/// as today's comment checker does.
fn is_tool_line(comment: &str) -> bool {
    let Some(rest) = comment.strip_prefix("//") else {
        return false;
    };
    if ["line ", "extern ", "export "]
        .iter()
        .any(|word| rest.starts_with(word))
    {
        return true;
    }
    // A tool name, a colon and a word, with no space after `//`.
    let Some((tool, after)) = rest.split_once(':') else {
        return false;
    };
    let is_word_char = |c: char| c.is_ascii_lowercase() || c.is_ascii_digit();
    !tool.is_empty() && tool.chars().all(is_word_char) && after.starts_with(is_word_char)
}
