//! Finds Python's `#` comments and docstrings. Each keeps the text that
//! today's comment checker gives it, except around a tool line, so older
//! approvals still match.

use std::iter::successors;

use tree_sitter::{Language, Node};

use super::{LanguageRules, nodes_starting_at};
use crate::shared::comments::CommentStyle;

/// Words that start a Python tool comment, just after its `#`.
const TOOL_PREFIXES: [&str; 13] = [
    "type:",
    "pragma:",
    "fmt:",
    "ruff:",
    "pylint:",
    "mypy:",
    "pyright:",
    "isort:",
    "coding:",
    "flake8:",
    "ty:",
    "yapf:",
    "pyre-ignore",
];

/// Words that start a Python tool comment only when whole, so `# noqa`
/// counts and `# noqab` does not.
const TOOL_WORDS: [&str; 2] = ["noqa", "nosec"];

pub struct PythonRules;

impl LanguageRules for PythonRules {
    fn grammar(&self) -> Language {
        tree_sitter_python::LANGUAGE.into()
    }

    fn classify_node(&self, node: Node, source: &str) -> Option<CommentStyle> {
        match node.kind() {
            "comment" if is_tool_line(&source[node.byte_range()]) => None,
            "comment" if is_encoding_line(node, source) => None,
            "comment" => Some(CommentStyle::Hash),
            "string" | "parenthesized_expression" if is_docstring(node) => {
                Some(CommentStyle::Docstring)
            }
            _ => None,
        }
    }

    fn name_comment(
        &self,
        comment: Node,
        code_below: Option<Node>,
        source: &str,
    ) -> Option<String> {
        // Today's comment checker counts a docstring's parentheses as code
        // on its lines, so it names no such docstring.
        if comment.kind() == "parenthesized_expression" {
            return None;
        }
        if comment.kind() == "string" {
            // A docstring sits in a statement in the module or in a body.
            let body = comment.parent()?.parent()?;
            if body.kind() == "module" {
                return Some("<module>".to_string());
            }
            return find_dotted_name(body.parent()?, source);
        }
        let statement = find_named_statement(code_below?)?;
        let mut outer = successors(statement.parent(), Node::parent);
        // A comment in a function body describes no one item.
        if outer.any(|node| node.kind() == "function_definition") {
            return None;
        }
        Some(format!("# {}", find_dotted_name(statement, source)?))
    }
}

/// Returns the definition or one-name assignment that starts where
/// `code` does.
fn find_named_statement(code: Node) -> Option<Node> {
    nodes_starting_at(code).find_map(|node| match node.kind() {
        "decorated_definition" => node.child_by_field_name("definition"),
        "function_definition" | "class_definition" => Some(node),
        "assignment" if is_one_name_assignment(node) => Some(node),
        _ => None,
    })
}

/// Returns true for an assignment to one plain name, as today's comment
/// checker names only those.
fn is_one_name_assignment(assignment: Node) -> bool {
    let left = assignment.child_by_field_name("left");
    let right = assignment.child_by_field_name("right");
    left.is_some_and(|left| left.kind() == "identifier")
        && right.is_none_or(|right| right.kind() != "assignment")
}

/// Returns the statement's name after the names of the classes and
/// functions around it, joined by dots.
fn find_dotted_name(statement: Node, source: &str) -> Option<String> {
    let mut names = vec![read_name(statement, source)?];
    for outer in successors(statement.parent(), Node::parent) {
        if matches!(outer.kind(), "function_definition" | "class_definition") {
            names.push(read_name(outer, source)?);
        }
    }
    names.reverse();
    Some(names.join("."))
}

/// Returns the name a definition or assignment gives.
fn read_name<'source>(statement: Node, source: &'source str) -> Option<&'source str> {
    let field = if statement.kind() == "assignment" {
        "left"
    } else {
        "name"
    };
    Some(&source[statement.child_by_field_name(field)?.byte_range()])
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
    let starts_with_word = |word: &str| {
        rest.strip_prefix(word)
            .is_some_and(|after| !after.starts_with(|c: char| c.is_alphanumeric() || c == '_'))
    };
    TOOL_WORDS.iter().any(|word| starts_with_word(word))
        || TOOL_PREFIXES.iter().any(|prefix| rest.starts_with(prefix))
}

/// Returns true for an encoding line such as `# vim: fileencoding=utf-8`,
/// which Python reads only on the first two lines.
fn is_encoding_line(comment: Node, source: &str) -> bool {
    let text = &source[comment.byte_range()];
    comment.start_position().row < 2 && (text.contains("coding:") || text.contains("coding="))
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
