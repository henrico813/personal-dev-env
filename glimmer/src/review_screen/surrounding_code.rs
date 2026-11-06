//! This module finds a Feedback Request's comment in its file, with the
//! Surrounding Code above and below it and a docstring's Function Definition
//! Line. Read the `read_surrounding_code(repo, request)` and
//! `find_surrounding_code(request, source)` functions to understand the
//! module.

use std::fs;
use std::io;
use std::path::Path;

use crate::shared::comments::{
    Comment, CommentStyle, FindCommentsError, find_comments, find_function_definition_line,
    is_only_statement_in_block,
};
use crate::shared::review_session::FeedbackRequest;

/// How many lines of code the screen shows above and below a comment.
const CODE_LINES_AROUND: usize = 100;

/// Holds the request's comment as its file has it, and its Surrounding Code.
pub(super) struct SurroundingCode {
    pub(super) comment: Comment,
    pub(super) above: Vec<String>,
    pub(super) below: Vec<String>,
    /// Holds the `def` or `class` line a docstring describes, or `None` for
    /// any other comment.
    pub(super) definition_line: Option<String>,
    /// Is true for a docstring that is the only statement in its block.
    pub(super) is_only_statement_in_block: bool,
}

/// Says why the screen cannot show a request's comment.
#[derive(Debug, PartialEq)]
pub(super) enum CodeNotShown {
    FileGone,
    FileUnreadable,
    SyntaxError,
    CommentGone,
}

impl CodeNotShown {
    /// Returns what the user reads, with what to do next.
    pub(super) fn message(&self) -> &'static str {
        match self {
            Self::FileGone => "The file is gone. Press m to ask the agent.",
            Self::FileUnreadable => "The file can't be read. Press m to ask the agent.",
            Self::SyntaxError => "The file has a syntax error. Press m to ask the agent.",
            Self::CommentGone => "The comment is no longer in its file. Press m to ask the agent.",
        }
    }
}

/// Reads the request's file under `repo` and finds its comment there.
pub(super) fn read_surrounding_code(
    repo: &Path,
    request: &FeedbackRequest,
) -> Result<SurroundingCode, CodeNotShown> {
    let source =
        fs::read_to_string(repo.join(&request.file)).map_err(|error| match error.kind() {
            io::ErrorKind::NotFound => CodeNotShown::FileGone,
            _ => CodeNotShown::FileUnreadable,
        })?;
    find_surrounding_code(request, &source)
}

/// Finds the comment by its name and text, or by its text alone when it has
/// no name. The match nearest its saved line wins, so lines the agent added
/// above it since do not matter.
pub(super) fn find_surrounding_code(
    request: &FeedbackRequest,
    source: &str,
) -> Result<SurroundingCode, CodeNotShown> {
    let comments = find_comments(&request.file, source).map_err(|error| match error {
        FindCommentsError::SyntaxError => CodeNotShown::SyntaxError,
        FindCommentsError::UnsupportedLanguage => CodeNotShown::FileUnreadable,
    })?;
    let comment = comments
        .into_iter()
        .filter(|comment| comment.text == request.text)
        .filter(|comment| request.name.is_none() || comment.name == request.name)
        .min_by_key(|comment| comment.line.abs_diff(request.line))
        .ok_or(CodeNotShown::CommentGone)?;
    let lines: Vec<&str> = source.lines().collect();
    // Lines count from 1, so the comment starts at index line - 1.
    let start = comment.line - 1;
    // A comment that shares its line with code shows that line below it.
    let shares_line = comment.style == CommentStyle::AfterCode || !comment.text.ends_with('\n');
    let end = if shares_line {
        start
    } else {
        comment.end_line.min(lines.len())
    };
    let above = &lines[start.saturating_sub(CODE_LINES_AROUND)..start];
    let below = &lines[end..(end + CODE_LINES_AROUND).min(lines.len())];
    Ok(SurroundingCode {
        above: above.iter().map(|line| line.to_string()).collect(),
        below: below.iter().map(|line| line.to_string()).collect(),
        definition_line: find_function_definition_line(&comment, source).map(str::to_owned),
        is_only_statement_in_block: is_only_statement_in_block(&comment, source),
        comment,
    })
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;

    /// Returns an unnamed request about `text` near `line` of `save.go`, or
    /// a named one when `name` is a string.
    fn make_request(name: Value, text: &str, line: usize) -> FeedbackRequest {
        serde_json::from_value(json!({
            "id": 1,
            "file": "save.go",
            "name": name,
            "reason": "new",
            "text": text,
            "can_reword": true,
            "line": line,
            "suggestion": null,
        }))
        .expect("the request matches its saved shape")
    }

    /// `find_surrounding_code` must pick the copy of a repeated comment that
    /// the request names, or the one nearest its line when it has no name.
    /// Otherwise the screen shows, and checks answers against, another copy.
    #[test]
    fn find_surrounding_code_picks_the_requested_copy() {
        let source = "package main\n\n// Saves it.\nfunc first() {\n\t// Saves it.\n\tsave()\n}\n\n// Saves it.\nfunc second() {\n\tadded()\n\t// Saves it.\n\tsave()\n}\n";
        let cases = [
            ("named", json!("first"), "// Saves it.\n", 9, 3),
            ("unnamed", json!(null), "\t// Saves it.\n", 11, 12),
        ];

        for (case, name, text, line, found_line) in cases {
            let request = make_request(name, text, line);

            let code =
                find_surrounding_code(&request, source).expect("the comment is in the source");

            assert_eq!(code.comment.line, found_line, "{case}");
        }
    }

    /// `find_surrounding_code` must keep the code lines just above and below
    /// the comment, and a line it shares with code below it. Otherwise an
    /// off-by-one drops a line of code or shows a comment line as code.
    #[test]
    fn find_surrounding_code_keeps_adjacent_lines() {
        let source = "package main\n\nfunc save() {\n\t// Saves it\n\t// twice.\n\tsave()\n\tx := 1 // Counts.\n\t_ = x\n}\n";
        let cases = [
            (
                "own lines",
                "\t// Saves it\n\t// twice.\n",
                4,
                "func save() {",
                "\tsave()",
            ),
            (
                "after code",
                "// Counts.\n",
                7,
                "\tsave()",
                "\tx := 1 // Counts.",
            ),
        ];

        for (case, text, line, line_above, line_below) in cases {
            let request = make_request(json!(null), text, line);

            let code =
                find_surrounding_code(&request, source).expect("the comment is in the source");

            assert_eq!(
                code.above.last().map(String::as_str),
                Some(line_above),
                "{case}"
            );
            assert_eq!(
                code.below.first().map(String::as_str),
                Some(line_below),
                "{case}"
            );
        }
    }
}
