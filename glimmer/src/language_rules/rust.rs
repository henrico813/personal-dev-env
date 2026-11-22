//! This module finds Rust's line comments, block comments and `#[doc]`, names
//! the code each describes, and lists the code examples rustdoc reads as Rust
//! code. Read the `RustRules::classify_node(node, source)`,
//! `RustRules::name_code_below(code, source)` and `RustRules::list_doc_tests(text)`
//! functions to understand the module.

use std::iter::successors;
use std::path::Path;

use tree_sitter::{Language, Node};

use super::{
    LanguageRules, format_if_old_is_formatted, is_inside, line_comments, nodes_starting_at,
};
use crate::shared::Error;
use crate::shared::comments::{CommentStyle, Definition, NameCheck};
use crate::shared::style_rules::Rule;

/// Words that start a comment line a tool reads, in any case.
const TOOL_MARKS: [&str; 1] = ["safety:"];

/// Lists the words in a code fence's info string that make rustdoc read the
/// block as Rust. `edition2021` and `ignore-wasm32` match by their start.
const RUST_FENCE_WORDS: [&str; 7] = [
    "rust",
    "ignore",
    "no_run",
    "should_panic",
    "compile_fail",
    "test_harness",
    "standalone_crate",
];

pub struct RustRules;

impl LanguageRules for RustRules {
    fn grammar(&self) -> Language {
        tree_sitter_rust::LANGUAGE.into()
    }

    fn name(&self) -> &'static str {
        "rust"
    }

    fn classify_node(&self, node: Node, source: &str) -> Option<CommentStyle> {
        match node.kind() {
            "line_comment" => Some(find_line_style(node)),
            "block_comment" => Some(CommentStyle::Block),
            "attribute_item" | "inner_attribute_item" if is_doc_attribute(node, source) => {
                Some(CommentStyle::DocAttribute)
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
        if is_module_doc(comment) {
            return Some("<module>".to_string());
        }
        self.name_code_below(code_below?, source)
    }

    fn name_code_below(&self, code: Node, source: &str) -> Option<String> {
        let item = find_item(code)?;
        let mut names = vec![name_item(item, source)?];
        for outer in successors(item.parent(), Node::parent) {
            match outer.kind() {
                // A comment in a function body describes no one item.
                "block" => return None,
                "impl_item" => names.push(read_impl_type(outer, source)?.to_string()),
                "mod_item" | "trait_item" | "struct_item" | "enum_item" | "union_item" => {
                    names.push(name_item(outer, source)?);
                }
                _ => {}
            }
        }
        names.reverse();
        Some(names.join("::"))
    }

    fn to_plain_words(&self, text: &str) -> String {
        line_comments::to_plain_words(text, find_line_marker(text))
    }

    fn to_paragraphs(&self, text: &str) -> Vec<String> {
        line_comments::to_paragraphs(text, find_line_marker(text))
    }

    /// Returns each function and method whose name this code chose, so not
    /// one in an `extern` block.
    fn list_definitions(&self, _file: &Path, root: Node, source: &str) -> Vec<Definition> {
        let mut definitions = Vec::new();
        let mut nodes = vec![root];
        while let Some(node) = nodes.pop() {
            let is_function = node.kind() == "function_item"
                || (node.kind() == "function_signature_item"
                    && node
                        .parent()
                        .and_then(|list| list.parent())
                        .map(|item| item.kind())
                        == Some("trait_item"));
            if is_function && let Some(name) = name_function(node, source) {
                definitions.push(Definition {
                    line: node.start_position().row + 1,
                    name_check: choose_name_check(node, source),
                    name,
                    docstring: None,
                });
            }
            let mut cursor = node.walk();
            let children: Vec<Node> = node.named_children(&mut cursor).collect();
            nodes.extend(children.into_iter().rev());
        }
        definitions
    }

    fn style_rules(&self) -> &'static [Rule] {
        &[
            Rule::Filler,
            Rule::AvoidedWord,
            Rule::RestatedName,
            Rule::StartsWithName,
            Rule::SevenWords,
            Rule::VerbFirst,
        ]
    }

    fn lay_out_plain_words(&self, words: &str, old_text: &str) -> String {
        line_comments::lay_out_plain_words(words, old_text, find_line_marker(old_text))
    }

    fn finish_rewrite(&self, folder: &Path, old: &str, new: &str) -> Result<String, Error> {
        // Without an edition, rustfmt cannot parse 2024 syntax such as let chains.
        format_if_old_is_formatted(&["rustfmt", "--edition", "2024"], folder, old, new)
    }

    /// Returns true for a `/* */` comment and for any comment inside a macro
    /// call or definition, where a macro can read a `///` line as code.
    fn is_keep_only(&self, comment: Node, _source: &str) -> bool {
        comment.kind() == "block_comment"
            || is_inside(comment, &["token_tree", "token_tree_pattern"])
    }

    /// Returns true when a line starts with a mark a tool reads, such as
    /// `SAFETY:` above an unsafe block.
    fn is_read_by_tools(&self, _file: &Path, text: &str) -> bool {
        text.lines().any(|line| {
            let words = line
                .trim_start()
                .trim_start_matches(['/', '!'])
                .trim_start();
            TOOL_MARKS
                .iter()
                .any(|mark| words.to_lowercase().starts_with(mark))
        })
    }

    /// Returns each block of a `///` or `//!` comment that rustdoc reads as
    /// Rust code, its fence lines included, with each line's end trimmed.
    fn list_doc_tests(&self, text: &str) -> Vec<String> {
        let marker = find_line_marker(text);
        // `find_line_marker` gives `//` for every other comment, block comments too. rustdoc reads
        // only `///` and `//!` here, since a block doc comment takes no reword.
        if marker == "//" || text.trim_start().starts_with("////") {
            return Vec::new();
        }
        let bodies: Option<Vec<String>> = text
            .lines()
            .map(|line| {
                let body = line.trim_start().strip_prefix(marker)?;
                Some(body.replace('\t', "    ").trim_end().to_owned())
            })
            .collect();
        bodies.map_or_else(Vec::new, |bodies| find_doc_test_blocks(&bodies))
    }
}

/// Returns a function's name after the types, traits and modules around it,
/// as `name_code_below` names a comment above it, such as `Store::save`.
fn name_function(function: Node, source: &str) -> Option<String> {
    let mut names = vec![name_item(function, source)?];
    for outer in successors(function.parent(), Node::parent) {
        match outer.kind() {
            "block" => break,
            "impl_item" => names.push(read_impl_type(outer, source)?.to_string()),
            "mod_item" | "trait_item" => names.push(name_item(outer, source)?),
            _ => {}
        }
    }
    names.reverse();
    Some(names.join("::"))
}

/// Returns which name rules a function must meet. A method in
/// `impl Trait for Type` and `main` are skipped, since the trait or Rust
/// chose the name. A test, a constructor and a getter need no verb.
fn choose_name_check(function: Node, source: &str) -> NameCheck {
    let name = function
        .child_by_field_name("name")
        .map_or("", |name| &source[name.byte_range()]);
    let is_in_trait_impl = function
        .parent()
        .and_then(|list| list.parent())
        .is_some_and(|item| {
            item.kind() == "impl_item" && item.child_by_field_name("trait").is_some()
        });
    if is_in_trait_impl || name == "main" {
        return NameCheck::Skip;
    }
    let is_constructor = name == "new" || name.starts_with("new_") || name.starts_with("with_");
    if is_constructor || is_test_function(function, source) || is_getter(function, source) {
        NameCheck::WordCount
    } else {
        NameCheck::WordCountAndVerb
    }
}

/// Returns true when an attribute above the function ends in `test`, such
/// as `#[test]` or `#[tokio::test]`.
fn is_test_function(function: Node, source: &str) -> bool {
    successors(function.prev_named_sibling(), Node::prev_named_sibling)
        .take_while(|node| node.kind() == "attribute_item" || node.kind().ends_with("comment"))
        .filter(|node| node.kind() == "attribute_item")
        .filter_map(|item| item.named_child(0)?.named_child(0))
        .any(|path| {
            let path = &source[path.byte_range()];
            path.rsplit("::")
                .next()
                .is_some_and(|last| last.ends_with("test"))
        })
}

/// Returns true for a method whose code shows it is a getter. It takes only
/// `&self`, and its body is one expression that reads a field of `self`,
/// such as `&self.title` or `self.name.clone()`.
fn is_getter(function: Node, source: &str) -> bool {
    let (Some(parameters), Some(body)) = (
        function.child_by_field_name("parameters"),
        function.child_by_field_name("body"),
    ) else {
        return false;
    };
    let takes_shared_self = matches!(
        list_code(parameters).as_slice(),
        [only] if only.kind() == "self_parameter" && {
            let text = &source[only.byte_range()];
            text.starts_with('&') && !text.contains("mut")
        }
    );
    takes_shared_self
        && matches!(list_code(body).as_slice(), [only] if reads_self_field(*only, source))
}

/// Returns a node's named children other than comments.
fn list_code(node: Node) -> Vec<Node> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .filter(|child| !child.kind().ends_with("comment"))
        .collect()
}

/// Returns true for `self.title`, a reference or `*` copy of it, or the
/// field followed by `.clone()`, `.copied()`, `.cloned()` or an `as_` call
/// with no arguments, such as `self.name.as_str()`.
fn reads_self_field(expression: Node, source: &str) -> bool {
    let reads_value = |node: Node| {
        node.child_by_field_name("value")
            .is_some_and(|value| value.kind() == "self" || reads_self_field(value, source))
    };
    match expression.kind() {
        "field_expression" => reads_value(expression),
        "reference_expression" => expression
            .child_by_field_name("value")
            .is_some_and(|value| reads_self_field(value, source)),
        "unary_expression" => {
            source[expression.byte_range()].starts_with('*')
                && expression
                    .named_child(0)
                    .is_some_and(|value| reads_self_field(value, source))
        }
        "call_expression" => {
            let (Some(method), Some(arguments)) = (
                expression.child_by_field_name("function"),
                expression.child_by_field_name("arguments"),
            ) else {
                return false;
            };
            let Some(method_name) = method.child_by_field_name("field") else {
                return false;
            };
            let method_name = &source[method_name.byte_range()];
            method.kind() == "field_expression"
                && arguments.named_child_count() == 0
                && (["clone", "copied", "cloned"].contains(&method_name)
                    || method_name.starts_with("as_"))
                && reads_value(method)
        }
        _ => false,
    }
}

/// Returns the code blocks in doc comment lines, read as rustdoc reads them
/// after it removes the lines' shared indentation.
fn find_doc_test_blocks(bodies: &[String]) -> Vec<String> {
    let shared_indent = bodies
        .iter()
        .filter(|body| !body.is_empty())
        .map(|body| body.len() - body.trim_start().len())
        .min()
        .unwrap_or(0);
    let mut blocks = Vec::new();
    let mut fence: Option<Fence> = None;
    let mut indented: Vec<&str> = Vec::new();
    let mut in_paragraph = false;
    for body in bodies {
        let line = body.get(shared_indent..).unwrap_or("");
        if let Some(open) = &mut fence {
            open.lines.push(line);
            if open.is_closed_by(line) {
                if open.is_rust {
                    blocks.push(open.lines.join("\n"));
                }
                fence = None;
            }
            continue;
        }
        let indent = line.len() - line.trim_start().len();
        // An indented line inside a paragraph only continues it.
        if indent >= 4 && !in_paragraph {
            indented.push(line);
            continue;
        }
        if line.is_empty() {
            if !indented.is_empty() {
                indented.push(line);
            }
            in_paragraph = false;
            continue;
        }
        if !indented.is_empty() {
            blocks.push(indented.join("\n").trim_end().to_owned());
            indented.clear();
        }
        fence = Fence::open(line);
        in_paragraph = fence.is_none() && !line.trim_start().starts_with('#');
    }
    // An unclosed fence runs to the end of the comment.
    if let Some(open) = fence
        && open.is_rust
    {
        blocks.push(open.lines.join("\n"));
    }
    if !indented.is_empty() {
        blocks.push(indented.join("\n").trim_end().to_owned());
    }
    blocks
}

/// Holds an open code fence, its mark, and the lines read so far.
struct Fence<'a> {
    mark: char,
    length: usize,
    is_rust: bool,
    lines: Vec<&'a str>,
}

impl<'a> Fence<'a> {
    /// Returns the fence `line` opens, or `None` when it opens none.
    fn open(line: &'a str) -> Option<Self> {
        let trimmed = line.trim_start();
        let mark = trimmed
            .chars()
            .next()
            .filter(|mark| ['`', '~'].contains(mark))?;
        let length = trimmed.len() - trimmed.trim_start_matches(mark).len();
        let info = &trimmed[length..];
        if line.len() - trimmed.len() >= 4 || length < 3 || (mark == '`' && info.contains('`')) {
            return None;
        }
        let words: Vec<&str> = info
            .split(|letter: char| letter == ',' || letter.is_whitespace())
            .filter(|word| !word.is_empty())
            .collect();
        // rustdoc reads a block as Rust unless every word names something else.
        let is_rust = words.is_empty() || words.iter().any(|word| is_rust_fence_word(word));
        Some(Self {
            mark,
            length,
            is_rust,
            lines: vec![line],
        })
    }

    /// Returns true when `line` closes this fence.
    fn is_closed_by(&self, line: &str) -> bool {
        let trimmed = line.trim_start();
        line.len() - trimmed.len() < 4
            && trimmed.len() >= self.length
            && trimmed.chars().all(|letter| letter == self.mark)
    }
}

/// Returns true for an info string word that makes rustdoc read a block as Rust.
fn is_rust_fence_word(word: &str) -> bool {
    RUST_FENCE_WORDS.contains(&word) || word.starts_with("edition") || word.starts_with("ignore-")
}

/// Returns true for `//!`, `/*! */` and `#![doc]`, which describe the
/// module they are in.
fn is_module_doc(comment: Node) -> bool {
    comment.kind() == "inner_attribute_item"
        || comment
            .named_child(0)
            .is_some_and(|marker| marker.kind() == "inner_doc_comment_marker")
}

/// Returns the item that starts where `code` does, past any attributes
/// above it.
fn find_item(code: Node) -> Option<Node> {
    let mut item = nodes_starting_at(code).last()?;
    while item.kind() == "attribute_item" {
        item = item.next_named_sibling()?;
    }
    Some(item)
}

/// Returns an item's name, or `impl` and its type for an `impl` block.
fn name_item(item: Node, source: &str) -> Option<String> {
    if item.kind() != "impl_item" {
        let name = item.child_by_field_name("name")?;
        return Some(source[name.byte_range()].to_string());
    }
    let type_name = read_impl_type(item, source)?;
    match item.child_by_field_name("trait") {
        Some(trait_name) => Some(format!(
            "impl {} for {type_name}",
            strip_generics(&source[trait_name.byte_range()])
        )),
        None => Some(format!("impl {type_name}")),
    }
}

/// Returns the type an `impl` block is for, without its generics.
fn read_impl_type<'source>(impl_item: Node, source: &'source str) -> Option<&'source str> {
    let type_node = impl_item.child_by_field_name("type")?;
    Some(strip_generics(&source[type_node.byte_range()]))
}

/// Returns a type's name without the `<...>` after it.
fn strip_generics(type_text: &str) -> &str {
    type_text
        .split_once('<')
        .map_or(type_text, |(name, _)| name)
}

/// Returns `ItemDoc` for `///`, `ModuleDoc` for `//!`, and `Line`
/// for `//`.
fn find_line_style(comment: Node) -> CommentStyle {
    // tree-sitter marks a doc comment with a child node after its `//`.
    match comment.named_child(0).map(|marker| marker.kind()) {
        Some("outer_doc_comment_marker") => CommentStyle::ItemDoc,
        Some("inner_doc_comment_marker") => CommentStyle::ModuleDoc,
        _ => CommentStyle::Line,
    }
}

/// Returns true for `#[doc = "..."]`, but not for a setting such as
/// `#[doc(hidden)]`.
fn is_doc_attribute(attribute_item: Node, source: &str) -> bool {
    let Some(attribute) = attribute_item.named_child(0) else {
        return false;
    };
    let Some(path) = attribute.named_child(0) else {
        return false;
    };
    &source[path.byte_range()] == "doc" && attribute.child_by_field_name("value").is_some()
}

/// Returns the marker that starts each line of a line comment.
fn find_line_marker(text: &str) -> &'static str {
    let text = text.trim_start();
    if text.starts_with("//!") {
        "//!"
    } else if text.starts_with("///") {
        "///"
    } else {
        "//"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod list_doc_tests {
        use super::*;

        /// `list_doc_tests` must list each block rustdoc reads as Rust, fenced or indented, and no other block.
        /// Otherwise a reword could change a doc test unchecked, or be refused over a block that is not code.
        #[test]
        fn lists_only_blocks_rustdoc_reads_as_rust() {
            let fence = |info: &str| {
                format!(
                    "/// Adds one.\n///\n/// ```{info}\n/// add(1);\n/// ```\n/// Returns two.\n"
                )
            };
            let block = |info: &str| format!("```{info}\nadd(1);\n```");
            let cases = [
                ("no-tag", fence(""), vec![block("")]),
                (
                    "rust-and-no-run",
                    fence("rust,no_run"),
                    vec![block("rust,no_run")],
                ),
                ("ignore", fence("ignore"), vec![block("ignore")]),
                ("edition", fence("edition2021"), vec![block("edition2021")]),
                ("text", fence("text"), vec![]),
                (
                    "indented",
                    "/// Adds one.\n///\n///     add(1);\n".to_owned(),
                    vec!["    add(1);".to_owned()],
                ),
                (
                    "paragraph-continuation",
                    "/// Adds\n///     one.\n".to_owned(),
                    vec![],
                ),
                ("line-comment", fence("").replace("///", "//"), vec![]),
            ];

            for (case, text, expected) in cases {
                assert_eq!(RustRules.list_doc_tests(&text), expected, "{case}");
            }
        }
    }
}
