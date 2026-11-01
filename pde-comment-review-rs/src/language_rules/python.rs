//! Finds Python's `#` comments and docstrings. Each keeps the text that
//! today's comment checker gives it, except around a tool line, so older
//! approvals still match.

use std::collections::BTreeSet;
use std::iter::successors;
use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;
use tree_sitter::{Language, Node};

use super::{
    LanguageRules, docstrings, is_inside, line_comments, nodes_starting_at, run_with_input,
};
use crate::shared::comments::{CommentStyle, Definition, DocstringFacts, NameCheck};
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

/// Lists the decorators that mark a property, whose name needs no verb.
const PROPERTY_DECORATORS: [&str; 5] =
    ["cached_property", "deleter", "getter", "property", "setter"];

/// Matches a name Python, unittest, http.server or pytest requires, and `main`.
static FRAMEWORK_NAME: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^(?:__\w+__|main|setUp\w*|tearDown\w*|asyncSetUp|asyncTearDown|do_[A-Z]+|pytest_\w+)$",
    )
    .expect("the pattern is valid")
});

static GENERATED_MARKER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?m)^# (?:Code generated .* DO NOT EDIT\.$|@generated\b)")
        .expect("the pattern is valid")
});

/// Matches a tool comment such as `# noqa` after prose in the same comment.
static TRAILING_TOOL_COMMENT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\s*#\s*(?:noqa\b|type:|pragma:).*").expect("the pattern is valid")
});

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
            return self.name_docstring_owner(body.parent()?, source);
        }
        self.name_code_below(code_below?, source)
    }

    fn name_code_below(&self, code: Node, source: &str) -> Option<String> {
        let statement = find_named_statement(code)?;
        let mut outer = successors(statement.parent(), Node::parent);
        // A comment in a function body describes no one item.
        if outer.any(|node| node.kind() == "function_definition") {
            return None;
        }
        Some(format!("# {}", find_dotted_name(statement, source)?))
    }

    fn name_docstring_owner(&self, definition: Node, source: &str) -> Option<String> {
        if !matches!(
            definition.kind(),
            "function_definition" | "class_definition"
        ) {
            return None;
        }
        find_dotted_name(definition, source)
    }

    fn to_plain_words(&self, text: &str) -> String {
        if text.trim_start().starts_with('#') {
            line_comments::to_plain_words(text, "#")
        } else {
            docstrings::to_plain_words(text)
        }
    }

    fn to_paragraphs(&self, text: &str) -> Vec<String> {
        if !text.trim_start().starts_with('#') {
            return docstrings::to_paragraphs(text);
        }
        let lines: Vec<String> = text
            .lines()
            .map(|line| match line.split_once('#') {
                Some((indent, words)) => {
                    format!("{indent}#{}", TRAILING_TOOL_COMMENT.replace(words, ""))
                }
                None => line.to_owned(),
            })
            .collect();
        line_comments::to_paragraphs(&lines.join("\n"), "#")
    }

    /// Returns a `#` comment's lines joined as today's checker joins them for
    /// its length and colon rules, examples included, or `None` for a docstring.
    fn join_unfiltered_lines(&self, text: &str) -> Option<String> {
        if !text.trim_start().starts_with('#') {
            return None;
        }
        let words: Vec<String> = text
            .lines()
            .map(|line| {
                let words = line.trim_start().strip_prefix('#').unwrap_or(line);
                TRAILING_TOOL_COMMENT.replace(words, "").trim().to_owned()
            })
            .collect();
        Some(words.join(" "))
    }

    /// Returns each function and class, with the lines that belong to it alone.
    fn list_definitions(&self, _file: &Path, root: Node, source: &str) -> Vec<Definition> {
        let mut definitions = Vec::new();
        let mut nodes = vec![root];
        while let Some(node) = nodes.pop() {
            if matches!(node.kind(), "function_definition" | "class_definition")
                && let Some(definition) = read_definition(node, source)
            {
                definitions.push(definition);
            }
            let mut cursor = node.walk();
            let children: Vec<Node> = node.named_children(&mut cursor).collect();
            nodes.extend(children.into_iter().rev());
        }
        definitions
    }

    fn is_generated(&self, source: &str) -> bool {
        GENERATED_MARKER.is_match(source)
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
    fn finish_rewrite(&self, folder: &Path, old: &str, new: &str) -> Result<String, Error> {
        if read_file_header(old) != read_file_header(new) {
            return Err(Error {
                code: ErrorCode::WouldNotWrite,
                message: "The new text changes the encoding or `#!` line Python reads.".to_owned(),
            });
        }
        let mut old_warnings = list_compile_warnings(folder, old).map_err(|reason| Error {
            code: ErrorCode::FileUnreadable,
            message: format!("Python cannot compile the file. {reason}"),
        })?;
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

/// Returns a function or class with its name rules and docstring facts.
fn read_definition(node: Node, source: &str) -> Option<Definition> {
    let name = find_dotted_name(node, source)?;
    let bare_name = read_name(node, source)?;
    let decorated = node
        .parent()
        .filter(|parent| parent.kind() == "decorated_definition");
    let decorators = decorated.map_or_else(Vec::new, |outer| list_decorator_names(outer, source));
    let is_function = node.kind() == "function_definition";
    let is_test = is_function && bare_name.starts_with("test");
    let name_check =
        if !is_function || FRAMEWORK_NAME.is_match(bare_name) || decorators.contains(&"override") {
            NameCheck::Skip
        } else if is_test
            || decorators
                .iter()
                .any(|decorator| *decorator == "fixture" || PROPERTY_DECORATORS.contains(decorator))
        {
            NameCheck::WordCount
        } else {
            NameCheck::WordCountAndVerb
        };
    let is_dunder = bare_name.starts_with("__") && bare_name.ends_with("__");
    Some(Definition {
        line: node.start_position().row + 1,
        name_check,
        docstring: Some(DocstringFacts {
            text: read_docstring_text(node, source),
            own_lines: list_own_lines(decorated.unwrap_or(node), node),
            is_required: !is_dunder || bare_name == "__init__",
            is_test,
        }),
        name,
    })
}

/// Returns the last part of each decorator, such as `fixture` for
/// `@pytest.fixture(scope="module")`.
fn list_decorator_names<'source>(decorated: Node, source: &'source str) -> Vec<&'source str> {
    let mut cursor = decorated.walk();
    decorated
        .named_children(&mut cursor)
        .filter(|child| child.kind() == "decorator")
        .filter_map(|decorator| {
            let mut expression = decorator.named_child(0)?;
            if expression.kind() == "call" {
                expression = expression.child_by_field_name("function")?;
            }
            let name = match expression.kind() {
                "attribute" => expression.child_by_field_name("attribute")?,
                "identifier" => expression,
                _ => return None,
            };
            Some(&source[name.byte_range()])
        })
        .collect()
}

/// Returns the text between a definition's docstring quotes, or `None` when
/// it has no docstring.
fn read_docstring_text(definition: Node, source: &str) -> Option<String> {
    let body = definition.child_by_field_name("body")?;
    let mut cursor = body.walk();
    let first = body
        .named_children(&mut cursor)
        .find(|child| child.kind() != "comment")?;
    let mut expression = first.named_child(0)?;
    if !is_docstring(expression, source) {
        return None;
    }
    while expression.kind() == "parenthesized_expression" {
        expression = expression.named_child(0)?;
    }
    docstrings::read_string_body(&source[expression.byte_range()]).map(str::to_owned)
}

/// Returns the lines from a definition's first decorator to its end, leaving
/// out the definitions inside it, so a change there counts for them alone.
fn list_own_lines(decorated: Node, definition: Node) -> BTreeSet<usize> {
    let mut lines = list_lines(decorated);
    let mut cursor = definition.walk();
    let mut nodes: Vec<Node> = definition.named_children(&mut cursor).collect();
    while let Some(node) = nodes.pop() {
        if matches!(
            node.kind(),
            "function_definition" | "class_definition" | "decorated_definition"
        ) {
            for line in list_lines(node) {
                lines.remove(&line);
            }
            continue;
        }
        let mut cursor = node.walk();
        nodes.extend(node.named_children(&mut cursor));
    }
    lines
}

/// Returns the lines a node spans, counting from 1.
fn list_lines(node: Node) -> BTreeSet<usize> {
    (node.start_position().row + 1..=node.end_position().row + 1).collect()
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
