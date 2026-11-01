//! Finds Rust's line comments, block comments and `#[doc]`. Rust's
//! tools read attributes, so no Rust comment is a tool line.

use std::iter::successors;
use std::path::Path;

use tree_sitter::{Language, Node};

use super::{
    LanguageRules, format_if_old_is_formatted, is_inside, line_comments, nodes_starting_at,
};
use crate::shared::Error;
use crate::shared::comments::CommentStyle;

/// Words that start a comment line a tool reads, in any case.
const TOOL_MARKS: [&str; 1] = ["safety:"];

pub struct RustRules;

impl LanguageRules for RustRules {
    fn grammar(&self) -> Language {
        tree_sitter_rust::LANGUAGE.into()
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
        let item = find_item(code_below?)?;
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
