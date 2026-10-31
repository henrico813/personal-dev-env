//! Finds Rust's line comments, block comments and `#[doc]`. Rust's
//! tools read attributes, so no Rust comment is a tool line.

use std::iter::successors;

use tree_sitter::{Language, Node};

use super::{LanguageRules, nodes_starting_at};
use crate::shared::comments::CommentStyle;

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
