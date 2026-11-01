//! This module picks a language's comment rules from a file's path. Only
//! `shared/comments.rs` calls it. Read the `LanguageRules` trait and the
//! `find_rules_for(file)` function to understand the module.

mod docstrings;
mod go;
mod line_comments;
mod python;
mod rust;

use std::io::Write;
use std::iter::successors;
use std::path::Path;
use std::process::{Command, Stdio};

use tree_sitter::{Language, Node};

use crate::shared::comments::CommentStyle;
use crate::shared::{Error, ErrorCode};

/// Tells which syntax tree nodes are comments in one language.
pub trait LanguageRules {
    /// Returns the tree-sitter grammar.
    fn grammar(&self) -> Language;
    /// Returns a comment node's style, or `None` for other nodes and
    /// tool lines.
    fn classify_node(&self, node: Node, source: &str) -> Option<CommentStyle>;
    /// Returns the name of the code a comment describes, or `None` when
    /// it describes no one definition. `code_below` is the first node on
    /// the next line, or `None` when that line is blank.
    fn name_comment(&self, comment: Node, code_below: Option<Node>, source: &str)
    -> Option<String>;
    /// Returns the comment's words without its markers or quotes. Text that
    /// is not all marked lines, or one whole docstring, comes back as it is.
    fn to_plain_words(&self, text: &str) -> String;
    /// Returns `words` laid out like `old_text`, with its markers and
    /// indentation, or an empty text when `words` has none.
    fn lay_out_plain_words(&self, words: &str, old_text: &str) -> String;
    /// Returns the new source as the language's tools leave it, or refuses
    /// it. `folder` holds the file, so a formatter finds its settings.
    fn finish_rewrite(&self, folder: &Path, old: &str, new: &str) -> Result<String, Error>;
    /// Returns true for comment text a build or test tool reads, in any case
    /// or spacing, so a cut or reword would change what the tool does.
    fn is_read_by_tools(&self, file: &Path, text: &str) -> bool;
    /// Returns true for a comment node that a cut could turn into different
    /// code, such as one inside a macro call.
    fn is_keep_only(&self, comment: Node, source: &str) -> bool;
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

/// Returns true when `node` sits inside a node of one of `kinds`.
fn is_inside(node: Node, kinds: &[&str]) -> bool {
    successors(node.parent(), Node::parent).any(|outer| kinds.contains(&outer.kind()))
}

/// Returns `node` and each parent that starts where it does, innermost
/// first.
fn nodes_starting_at(node: Node) -> impl Iterator<Item = Node> {
    let start = node.start_byte();
    successors(Some(node), move |child| {
        child.parent().filter(|parent| parent.start_byte() == start)
    })
}

/// Formats `new` only when the formatter leaves `old` as it is, so a review
/// never reformats code the user wrote.
fn format_if_old_is_formatted(
    command: &[&str],
    folder: &Path,
    old: &str,
    new: &str,
) -> Result<String, Error> {
    match run_with_input(command, folder, old) {
        Ok(formatted) if formatted == old => {}
        // A missing formatter, or one that would change the old file, leaves the edits unformatted.
        _ => return Ok(new.to_owned()),
    }
    run_with_input(command, folder, new).map_err(|reason| Error {
        code: ErrorCode::WouldNotWrite,
        message: format!("{} cannot format the new text. {reason}", command[0]),
    })
}

/// Returns what `command` prints for `input`, or why it failed.
fn run_with_input(command: &[&str], folder: &Path, input: &str) -> Result<String, String> {
    let mut child = Command::new(command[0])
        .args(&command[1..])
        .current_dir(folder)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("Cannot run {}. {error}", command[0]))?;
    // Each tool reads all its input before it prints, so this write cannot block.
    let mut stdin = child.stdin.take().expect("stdin is piped");
    let written = stdin.write_all(input.as_bytes());
    drop(stdin);
    let output = child
        .wait_with_output()
        .map_err(|error| format!("Cannot run {}. {error}", command[0]))?;
    if !output.status.success() {
        // The last line says why, as in a Python traceback.
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(stderr.trim().lines().last().unwrap_or_default().to_owned());
    }
    written.map_err(|error| format!("Cannot send {} the file. {error}", command[0]))?;
    String::from_utf8(output.stdout).map_err(|error| error.to_string())
}
