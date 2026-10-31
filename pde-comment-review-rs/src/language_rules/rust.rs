//! Finds Rust's line comments, block comments and `#[doc]`. Rust's
//! tools read attributes, so no Rust comment is a tool line.

use tree_sitter::{Language, Node};

use super::LanguageRules;
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
