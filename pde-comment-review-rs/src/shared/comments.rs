//! Finds each comment in a file's syntax tree. This is the only file
//! that calls `language_rules/`.

use std::fmt;
use std::path::Path;

use tree_sitter::{Node, Parser};

use crate::language_rules::{LanguageRules, find_rules_for};

/// One comment and where it is in its file. Lines count from 1.
#[derive(Debug)]
pub struct Comment {
    pub line: usize,
    pub end_line: usize,
    pub style: CommentStyle,
    /// The comment's lines as written, leaving out any code that shares
    /// them. It ends in a newline unless code follows it.
    pub text: String,
}

/// How a comment is marked. Lines join into one comment only when
/// their style matches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommentStyle {
    /// `//` in Go and Rust.
    Line,
    /// `///` before a Rust function or type.
    ItemDoc,
    /// `//!` at the top of a Rust module.
    ModuleDoc,
    /// `#` in Python.
    Hash,
    /// A Python `"""` docstring.
    Docstring,
    /// A comment after code on its line, holding none of the code.
    AfterCode,
    /// `/* */`, `/** */` or `/*! */`, kept as whole lines unless code
    /// follows it.
    Block,
    /// A Rust `#[doc]` attribute.
    DocAttribute,
}

impl CommentStyle {
    /// Returns false for the styles the user can only keep or cut.
    pub fn can_reword(self) -> bool {
        match self {
            Self::Line | Self::ItemDoc | Self::ModuleDoc | Self::Hash | Self::Docstring => true,
            Self::AfterCode | Self::Block | Self::DocAttribute => false,
        }
    }

    /// Returns true for the line styles whose neighboring lines join
    /// into one comment.
    fn joins_neighboring_lines(self) -> bool {
        matches!(
            self,
            Self::Line | Self::ItemDoc | Self::ModuleDoc | Self::Hash
        )
    }
}

/// Why a file's comments could not be read.
#[derive(Debug, PartialEq, Eq)]
pub enum FindCommentsError {
    /// No language rules match the file's extension.
    UnsupportedLanguage,
    /// The file does not parse, so its comments could be misread.
    SyntaxError,
}

impl fmt::Display for FindCommentsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedLanguage => write!(f, "no language rules read this file"),
            Self::SyntaxError => write!(f, "the file does not parse"),
        }
    }
}

impl std::error::Error for FindCommentsError {}

/// Finds every comment in a file's source, in file order, leaving out
/// tool lines.
///
/// # Errors
///
/// Fails when no language rules read the file or the source does not
/// parse.
pub fn find_comments(file: &Path, source: &str) -> Result<Vec<Comment>, FindCommentsError> {
    let rules = find_rules_for(file).ok_or(FindCommentsError::UnsupportedLanguage)?;
    let mut parser = Parser::new();
    parser
        .set_language(&rules.grammar())
        .expect("each grammar crate supports this tree-sitter version");
    let tree = parser
        .parse(source, None)
        .expect("parsing fails only after a timeout or cancel, and neither is set");
    if tree.root_node().has_error() {
        return Err(FindCommentsError::SyntaxError);
    }
    let mut comments = Vec::new();
    collect_comments(rules, tree.root_node(), source, &mut comments);
    Ok(comments)
}

/// Adds the comments in `node` and its children to `comments`.
fn collect_comments(
    rules: &dyn LanguageRules,
    node: Node,
    source: &str,
    comments: &mut Vec<Comment>,
) {
    match rules.classify_node(node, source) {
        Some(style) => add_comment(node, style, source, comments),
        None => {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                collect_comments(rules, child, source, comments);
            }
        }
    }
}

/// Adds a comment, joining it to the comment above when both are
/// neighboring lines of one style.
fn add_comment(node: Node, style: CommentStyle, source: &str, comments: &mut Vec<Comment>) {
    let line_start = node.start_byte() - node.start_position().column;
    let after_code = !source[line_start..node.start_byte()].trim().is_empty();
    // A comment after code starts at its marker, so the code stays out.
    let text_start = if after_code {
        node.start_byte()
    } else {
        line_start
    };
    let style = if after_code {
        CommentStyle::AfterCode
    } else {
        style
    };

    let line_end = find_line_end(source, node.end_byte());
    // Code after a block comment on its line stays out of its text.
    let code_after = !source[node.end_byte()..line_end].trim().is_empty();
    let text_end = if code_after {
        node.end_byte()
    } else {
        line_end
    };

    let mut text = source[text_start..text_end].to_string();
    if !code_after && !text.ends_with('\n') {
        text.push('\n');
    }
    let line = node.start_position().row + 1;
    let end_line = line + text.lines().count() - 1;

    if let Some(above) = comments.last_mut()
        && style.joins_neighboring_lines()
        && above.style == style
        && above.end_line + 1 == line
    {
        above.end_line = end_line;
        above.text.push_str(&text);
        return;
    }
    comments.push(Comment {
        line,
        end_line,
        style,
        text,
    });
}

/// Returns the end of the line holding `node_end`, just past its
/// newline.
fn find_line_end(source: &str, node_end: usize) -> usize {
    // Rust's `///` and `//!` nodes already end with their newline.
    if source[..node_end].ends_with('\n') {
        return node_end;
    }
    match source[node_end..].find('\n') {
        Some(offset) => node_end + offset + 1,
        None => source.len(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use CommentStyle::{AfterCode, Block, DocAttribute, Docstring, Hash, ItemDoc, Line, ModuleDoc};

    /// One comment a test expects `find_comments` to return.
    struct Expected {
        line: usize,
        end_line: usize,
        style: CommentStyle,
        text: &'static str,
    }

    /// Checks each comment's lines, style and exact text.
    fn assert_finds(file: &str, source: &str, expected: &[Expected]) {
        let comments = find_comments(Path::new(file), source).expect("the sample parses");
        let found: Vec<_> = comments
            .iter()
            .map(|c| (c.line, c.end_line, c.style, c.text.as_str()))
            .collect();
        let wanted: Vec<_> = expected
            .iter()
            .map(|e| (e.line, e.end_line, e.style, e.text))
            .collect();
        assert_eq!(found, wanted, "{file}");
    }

    /// A comment missed or cut short here never reaches the owner.
    #[test]
    fn finds_every_comment_in_each_language() {
        assert_finds(
            "review_store.go",
            include_str!("../../tests/samples/review_store.go"),
            &[
                Expected {
                    line: 3,
                    end_line: 3,
                    style: Line,
                    text: "// Saves the owner's answer under the review's lock.\n",
                },
                Expected {
                    line: 7,
                    end_line: 7,
                    style: Line,
                    text: "\t// The status is saved, never worked out from files.\n",
                },
            ],
        );
        assert_finds(
            "approvals.py",
            include_str!("../../tests/samples/approvals.py"),
            &[
                Expected {
                    line: 1,
                    end_line: 1,
                    style: Hash,
                    text: "# Approvals live in the clone's shared Git folder.\n",
                },
                Expected {
                    line: 5,
                    end_line: 5,
                    style: Docstring,
                    text: "    \"\"\"Return every approval entry, oldest first.\"\"\"\n",
                },
            ],
        );
        assert_finds(
            "review.rs",
            include_str!("../../tests/samples/review.rs"),
            &[
                Expected {
                    line: 1,
                    end_line: 1,
                    style: ModuleDoc,
                    text: "//! Saves each review in one file, under a lock.\n",
                },
                Expected {
                    line: 3,
                    end_line: 3,
                    style: ItemDoc,
                    text: "/// Saves the owner's answer under the review's lock.\n",
                },
                Expected {
                    line: 5,
                    end_line: 5,
                    style: Line,
                    text: "    // Entries are only added, never changed.\n",
                },
            ],
        );
    }

    /// Approvals match a comment's exact text, so a wrong join loses
    /// them.
    #[test]
    fn groups_comment_lines_by_style() {
        assert_finds(
            "review_store.go",
            "package review\n\n// Saves the answer.\n// Waits for the lock.\nfunc SaveAnswer() {}\n",
            &[Expected {
                line: 3,
                end_line: 4,
                style: Line,
                text: "// Saves the answer.\n// Waits for the lock.\n",
            }],
        );
        assert_finds(
            "approvals.py",
            "# Approvals live in Git.\n# Each one has a name.\nAPPROVALS_PATH = \"\"\n",
            &[Expected {
                line: 1,
                end_line: 2,
                style: Hash,
                text: "# Approvals live in Git.\n# Each one has a name.\n",
            }],
        );
        assert_finds(
            "review.rs",
            "//! Saves each review in one file.\n//! Holds one lock per review.\n",
            &[Expected {
                line: 1,
                end_line: 2,
                style: ModuleDoc,
                text: "//! Saves each review in one file.\n//! Holds one lock per review.\n",
            }],
        );
        assert_finds(
            "comment_styles.rs",
            include_str!("../../tests/samples/comment_styles.rs"),
            &[
                Expected {
                    line: 1,
                    end_line: 2,
                    style: ItemDoc,
                    text: concat!(
                        "/// Saves one User Feedback under the lock.\n",
                        "/// Waits while another screen holds the lock.\n",
                    ),
                },
                Expected {
                    line: 3,
                    end_line: 3,
                    style: Line,
                    text: "// Only the review screen calls this.\n",
                },
                Expected {
                    line: 5,
                    end_line: 5,
                    style: AfterCode,
                    text: "// The lock waits at most three times.\n",
                },
            ],
        );
    }

    /// A missed keep-or-cut comment slips past review. One holding code
    /// would lose that code when cut.
    #[test]
    fn finds_keep_or_cut_comments_exactly() {
        assert_finds(
            "keep_or_cut.go",
            include_str!("../../tests/samples/keep_or_cut.go"),
            &[
                Expected {
                    line: 3,
                    end_line: 3,
                    style: Block,
                    text: "/* Saves the owner's answer under the review's lock. */\n",
                },
                Expected {
                    line: 7,
                    end_line: 7,
                    style: AfterCode,
                    text: "// The lock waits at most three times.\n",
                },
                Expected {
                    line: 8,
                    end_line: 8,
                    style: AfterCode,
                    text: "// Each wait is one second.\n",
                },
            ],
        );
        assert_finds(
            "keep_or_cut.rs",
            include_str!("../../tests/samples/keep_or_cut.rs"),
            &[
                Expected {
                    line: 1,
                    end_line: 1,
                    style: Block,
                    text: "/*! Saves each review in one file, under a lock. */\n",
                },
                Expected {
                    line: 2,
                    end_line: 3,
                    style: Block,
                    text: "/* Entries are only added,\n   never changed. */\n",
                },
                Expected {
                    line: 5,
                    end_line: 5,
                    style: AfterCode,
                    text: "/* The lock waits at most three times. */",
                },
                Expected {
                    line: 8,
                    end_line: 8,
                    style: Block,
                    text: "/** Finishes writing the review. */\n",
                },
                Expected {
                    line: 9,
                    end_line: 9,
                    style: DocAttribute,
                    text: "#[doc = \"Writes the answers into the code.\"]\n",
                },
            ],
        );
    }

    /// A docstring missed here never reaches review.
    #[test]
    fn finds_docstrings_below_comments_or_in_parentheses() {
        assert_finds(
            "docstrings.py",
            include_str!("../../tests/samples/docstrings.py"),
            &[
                Expected {
                    line: 1,
                    end_line: 1,
                    style: Hash,
                    text: "# Reads the approvals file for the review tool.\n",
                },
                Expected {
                    line: 2,
                    end_line: 2,
                    style: Docstring,
                    text: "\"\"\"Reads the approvals file.\"\"\"\n",
                },
                Expected {
                    line: 6,
                    end_line: 6,
                    style: Hash,
                    text: "    # Approvals are read once per step.\n",
                },
                Expected {
                    line: 7,
                    end_line: 10,
                    style: Docstring,
                    text: concat!(
                        "    \"\"\"Return every approval entry, oldest first.\n",
                        "\n",
                        "    A missing file has no entries.\n",
                        "    \"\"\"\n",
                    ),
                },
                Expected {
                    line: 15,
                    end_line: 15,
                    style: Docstring,
                    text: "    (\"\"\"Write every approval entry, newest last.\"\"\")\n",
                },
            ],
        );
        assert_finds(
            "approvals.py",
            "(\n    \"\"\"Reads the approvals file.\"\"\"\n)\n",
            &[Expected {
                line: 1,
                end_line: 3,
                style: Docstring,
                text: "(\n    \"\"\"Reads the approvals file.\"\"\"\n)\n",
            }],
        );
        assert_finds("approvals.py", "(load_approvals())\n", &[]);
    }

    /// One source and how many comments `find_comments` should find.
    struct ToolCase {
        file: &'static str,
        source: &'static str,
        expected_count: usize,
    }

    /// Cutting a tool line changes the build. Dropping a prose line skips
    /// its review.
    #[test]
    fn finds_prose_but_leaves_out_tool_lines() {
        let cases = [
            ToolCase {
                file: "review_store.go",
                source: "package review\n\n//go:generate stringer -type=Status\n",
                expected_count: 0,
            },
            ToolCase {
                file: "review_store.go",
                source: "package review\n\n//line review_store.go:10\n",
                expected_count: 0,
            },
            ToolCase {
                file: "review_store.go",
                source: "package review\n\n// Saves one answer: the newest.\n",
                expected_count: 1,
            },
            ToolCase {
                file: "approvals.py",
                source: "#!/usr/bin/env python3\n",
                expected_count: 0,
            },
            ToolCase {
                file: "approvals.py",
                source: "# pylint: disable=invalid-name\n",
                expected_count: 0,
            },
            ToolCase {
                file: "approvals.py",
                source: "import json  # noqa: F401\n",
                expected_count: 0,
            },
        ];

        for case in cases {
            let comments =
                find_comments(Path::new(case.file), case.source).expect("the source parses");
            assert_eq!(comments.len(), case.expected_count, "{:?}", case.source);
        }
    }

    /// A file the program could misread must stop the review, not lose
    /// comments.
    #[test]
    fn refuses_a_file_that_does_not_parse() {
        let source = "package review\n\nfunc SaveAnswer( {}\n";

        let result = find_comments(Path::new("broken.go"), source);

        assert_eq!(result.err(), Some(FindCommentsError::SyntaxError));
    }
}
