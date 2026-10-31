//! Picks a language's comment rules from a file's path. Only
//! `shared/comments.rs` calls this.

mod go;
mod python;
mod rust;

use std::path::Path;

use tree_sitter::{Language, Node};

use crate::shared::comments::CommentStyle;

/// Tells which syntax tree nodes are comments in one language.
pub trait LanguageRules {
    /// Returns the tree-sitter grammar.
    fn grammar(&self) -> Language;
    /// Returns a comment node's style, or `None` for other nodes and
    /// tool lines.
    fn classify_node(&self, node: Node, source: &str) -> Option<CommentStyle>;
}

/// Returns the rules for the file's language, if supported.
pub fn find_rules_for(file: &Path) -> Option<&'static dyn LanguageRules> {
    match file.extension()?.to_str()? {
        "go" => Some(&go::GoRules),
        "py" => Some(&python::PythonRules),
        "rs" => Some(&rust::RustRules),
        _ => None,
    }
}
