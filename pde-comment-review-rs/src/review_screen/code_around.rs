//! Finds a Feedback Request's comment in its file, with the code above and
//! below it that the screen shows faded.

use crate::shared::comments::{Comment, CommentStyle, find_comments};
use crate::shared::review_session::FeedbackRequest;

/// How many lines of code the screen shows above and below a comment.
const CODE_LINES_AROUND: usize = 100;

/// Holds the request's comment as its file has it now, and the code around it.
pub(super) struct CodeAround {
    pub(super) comment: Comment,
    pub(super) above: Vec<String>,
    pub(super) below: Vec<String>,
}

/// Finds the comment by its name and text, or by its text alone when it has
/// no name. The match nearest its saved line wins, so lines the agent added
/// above it since do not matter.
pub(super) fn find_code_around(
    request: &FeedbackRequest,
    source: &str,
) -> Result<CodeAround, String> {
    let comments = find_comments(&request.file, source)
        .map_err(|error| format!("{}: {error}", request.file.display()))?;
    let comment = comments
        .into_iter()
        .filter(|comment| comment.text == request.text)
        .filter(|comment| request.name.is_none() || comment.name == request.name)
        .min_by_key(|comment| comment.line.abs_diff(request.line))
        .ok_or_else(|| format!("the comment is no longer in {}", request.file.display()))?;
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
    Ok(CodeAround {
        above: above.iter().map(|line| line.to_string()).collect(),
        below: below.iter().map(|line| line.to_string()).collect(),
        comment,
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    /// Without it, the screen could show, and check answers against, another
    /// copy of the comment, or drop the line just above or below it.
    #[test]
    fn finds_comment_by_name_else_nearest_text() {
        let source = "package main\n\n// Saves it.\nfunc first() {\n\t// Saves it.\n\tsave()\n}\n\n// Saves it.\nfunc second() {\n\tadded()\n\t// Saves it.\n\tsave()\n}\n";
        let cases = [
            (
                "named",
                json!("first"),
                "// Saves it.\n",
                9,
                3,
                "",
                "func first() {",
            ),
            (
                "unnamed",
                json!(null),
                "\t// Saves it.\n",
                11,
                12,
                "\tadded()",
                "\tsave()",
            ),
        ];

        for (case, name, text, line, found_line, line_above, line_below) in cases {
            let request: FeedbackRequest = serde_json::from_value(json!({
                "id": 1,
                "file": "save.go",
                "name": name,
                "reason": "new",
                "text": text,
                "can_reword": true,
                "line": line,
                "suggestion": null,
            }))
            .expect("the request matches its saved shape");

            let code = find_code_around(&request, source).expect("the comment is in the source");

            assert_eq!(code.comment.line, found_line, "{case}");
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
