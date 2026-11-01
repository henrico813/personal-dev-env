//! Finds Python's `#` comments and docstrings. Each keeps the text that
//! today's comment checker gives it, except around a tool line, so older
//! approvals still match.

use std::iter::successors;
use std::path::Path;

use tree_sitter::{Language, Node};

use super::{
    LanguageRules, docstrings, is_inside, line_comments, nodes_starting_at, run_with_input,
};
use crate::shared::comments::CommentStyle;
use crate::shared::{Error, ErrorCode};

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

/// Compiles the source read from stdin and prints its warnings as JSON.
const COMPILE_SCRIPT: &str = "\
import json, sys, warnings
with warnings.catch_warnings(record=True) as caught:
    warnings.simplefilter('always')
    compile(sys.stdin.buffer.read(), '<file>', 'exec', dont_inherit=True)
print(json.dumps([str(warning.message) for warning in caught]))
";

/// Words that start a Python tool comment only when whole, so `# noqa`
/// counts and `# noqab` does not.
const TOOL_WORDS: [&str; 2] = ["noqa", "nosec"];

/// Words that mark a `#` comment a linter, type checker or formatter reads,
/// wherever they sit in it, in any case.
const TOOL_MARKS: [&str; 11] = [
    "noqa", "type:", "pragma", "pyre-", "pylint:", "mypy:", "nosec", "fmt:", "isort:", "pyright:",
    "pyrefly:",
];

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
            "string" | "parenthesized_expression" if is_docstring(node, source) => {
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

    fn to_plain_words(&self, text: &str) -> String {
        if text.trim_start().starts_with('#') {
            line_comments::to_plain_words(text, "#")
        } else {
            docstrings::to_plain_words(text)
        }
    }

    fn lay_out_plain_words(&self, words: &str, old_text: &str) -> String {
        if old_text.trim_start().starts_with('#') {
            line_comments::lay_out_plain_words(words, old_text, "#")
        } else {
            docstrings::lay_out_plain_words(words, old_text)
        }
    }

    /// Refuses new text that Python cannot compile or that adds a warning,
    /// such as an invalid escape. Warnings already in the file stay allowed.
    ///
    /// When this machine's Python cannot compile the old file, such as one
    /// that needs a newer Python, only a new backslash is refused, since a
    /// backslash could start an escape that no Python compiles.
    fn finish_rewrite(&self, folder: &Path, old: &str, new: &str) -> Result<String, Error> {
        if read_file_header(old) != read_file_header(new) {
            return Err(Error {
                code: ErrorCode::WouldNotWrite,
                message: "The new text changes the encoding or `#!` line Python reads.".to_owned(),
            });
        }
        let Ok(mut old_warnings) = list_compile_warnings(folder, old) else {
            if new.matches('\\').count() > old.matches('\\').count() {
                return Err(Error {
                    code: ErrorCode::WouldNotWrite,
                    message:
                        "This Python cannot compile the file, so it cannot check a new backslash."
                            .to_owned(),
                });
            }
            return Ok(new.to_owned());
        };
        let new_warnings = list_compile_warnings(folder, new).map_err(|reason| Error {
            code: ErrorCode::WouldNotWrite,
            message: format!("Python cannot compile the new text. {reason}"),
        })?;
        for warning in new_warnings {
            let Some(index) = old_warnings.iter().position(|old| *old == warning) else {
                return Err(Error {
                    code: ErrorCode::WouldNotWrite,
                    message: format!("The new text adds the Python warning \"{warning}\"."),
                });
            };
            old_warnings.swap_remove(index);
        }
        Ok(new.to_owned())
    }

    /// Returns true for a `#` comment that holds a tool's mark, such as
    /// `noqa` or `pragma`.
    fn is_read_by_tools(&self, _file: &Path, text: &str) -> bool {
        let text = text.to_lowercase();
        text.trim_start().starts_with('#') && TOOL_MARKS.iter().any(|mark| text.contains(mark))
    }

    /// Returns true for a comment inside an f-string's braces, which is part
    /// of the string's text when the braces end in `=`.
    fn is_keep_only(&self, comment: Node, _source: &str) -> bool {
        is_inside(comment, &["string"])
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

/// Returns whether line 1 is a `#!` line and the encoding Python reads from
/// lines 1 and 2, as PEP 263 says.
fn read_file_header(source: &str) -> (bool, Option<String>) {
    let mut encoding = None;
    for line in source.split('\n').take(2) {
        encoding = find_encoding_name(line);
        let code = line.trim_start_matches([' ', '\t', '\x0c']);
        // Python reads line 2 only when line 1 is blank or a comment.
        let blank_or_comment = code.is_empty() || code.starts_with(['#', '\r']);
        if encoding.is_some() || !blank_or_comment {
            break;
        }
    }
    (source.starts_with("#!"), encoding)
}

/// Returns the lowercased name in an encoding line, such as `latin-1` in
/// `# -*- coding: latin-1 -*-`.
fn find_encoding_name(line: &str) -> Option<String> {
    if !line
        .trim_start_matches([' ', '\t', '\x0c'])
        .starts_with('#')
    {
        return None;
    }
    line.match_indices("coding").find_map(|(index, word)| {
        let rest = line[index + word.len()..].strip_prefix([':', '='])?;
        let rest = rest.trim_start_matches([' ', '\t']);
        let is_name_char = |c: char| c.is_ascii_alphanumeric() || "-_.".contains(c);
        let end = rest.find(|c: char| !is_name_char(c)).unwrap_or(rest.len());
        (end > 0).then(|| rest[..end].to_lowercase())
    })
}

/// Returns true when a string, maybe in parentheses, is the first
/// statement of a module, class or function.
fn is_docstring(expression: Node, source: &str) -> bool {
    if !is_grouped_string(expression, source) {
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
fn is_grouped_string(node: Node, source: &str) -> bool {
    match node.kind() {
        "string" => {
            let text = &source[node.byte_range()];
            let prefix = &text[..text.find(['"', '\'']).unwrap_or(0)];
            // Python's docstring rule skips f-strings, t-strings and bytes, which are code.
            !prefix.contains(['f', 'F', 't', 'T', 'b', 'B'])
        }
        "parenthesized_expression" => node
            .named_child(0)
            .is_some_and(|inner| is_grouped_string(inner, source)),
        _ => false,
    }
}

/// Returns the warnings Python gives when it compiles `source`, or why it
/// cannot compile it.
fn list_compile_warnings(folder: &Path, source: &str) -> Result<Vec<String>, String> {
    let printed = run_with_input(&["python3", "-c", COMPILE_SCRIPT], folder, source)?;
    serde_json::from_str(&printed).map_err(|error| error.to_string())
}
