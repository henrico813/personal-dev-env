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
