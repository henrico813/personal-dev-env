//! Finds Python's `#` comments and docstrings. Each keeps the text that
//! today's comment checker gives it, except around a tool line, so older
//! approvals still match.

use tree_sitter::{Language, Node};

use super::LanguageRules;
use crate::shared::comments::CommentStyle;

/// Words that start a Python tool comment, just after its `#`.
const TOOL_PREFIXES: [&str; 9] = [
    "type:", "pragma:", "fmt:", "ruff:", "pylint:", "mypy:", "pyright:", "isort:", "coding:",
];

pub struct PythonRules;

impl LanguageRules for PythonRules {
    fn grammar(&self) -> Language {
        tree_sitter_python::LANGUAGE.into()
    }

    fn classify_node(&self, node: Node, source: &str) -> Option<CommentStyle> {
        match node.kind() {
            "comment" if is_tool_line(&source[node.byte_range()]) => None,
            "comment" => Some(CommentStyle::Hash),
            "string" | "parenthesized_expression" if is_docstring(node) => {
                Some(CommentStyle::Docstring)
            }
            _ => None,
        }
    }
}

/// Returns true for a comment Python's tools read, such as `# noqa`,
/// as today's comment checker does.
fn is_tool_line(comment: &str) -> bool {
    let Some(rest) = comment.strip_prefix('#') else {
        return false;
    };
    let rest = rest.trim_start();
    if rest.starts_with('!') || rest.starts_with("-*-") {
        return true;
    }
    if let Some(after) = rest.strip_prefix("noqa") {
        // `noqa` counts only as a whole word.
        return !after.starts_with(|c: char| c.is_alphanumeric() || c == '_');
    }
    TOOL_PREFIXES.iter().any(|prefix| rest.starts_with(prefix))
}

/// Returns true when a string, maybe in parentheses, is the first
/// statement of a module, class or function.
fn is_docstring(expression: Node) -> bool {
    if !is_grouped_string(expression) {
        return false;
    }
    let Some(statement) = expression.parent() else {
        return false;
    };
    let Some(body) = statement.parent() else {
        return false;
    };
    let can_hold_docstring = match body.kind() {
        "module" => true,
        "block" => body.parent().is_some_and(|owner| {
            matches!(owner.kind(), "function_definition" | "class_definition")
        }),
        _ => false,
    };
    if statement.kind() != "expression_statement" || !can_hold_docstring {
        return false;
    }
    // A comment above the docstring is a named child too.
    let mut cursor = body.walk();
    let mut children = body.named_children(&mut cursor);
    children.find(|child| child.kind() != "comment") == Some(statement)
}

/// Returns true for a string literal, alone or inside grouping
/// parentheses such as `("""Saves it.""")`.
fn is_grouped_string(node: Node) -> bool {
    match node.kind() {
        "string" => true,
        "parenthesized_expression" => node.named_child(0).is_some_and(is_grouped_string),
        _ => false,
    }
}
