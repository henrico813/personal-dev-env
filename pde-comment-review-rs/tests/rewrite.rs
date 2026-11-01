mod common;

use std::fs;
use std::path::Path;

use common::TempFolder;
use pde_comment_review::shared::comments::{CommentEdit, RewrittenFile, find_comments, rewrite};
use pde_comment_review::shared::{Error, ErrorCode};

/// Saves `source` as `file` in a new folder, beside a `rustfmt.toml` holding
/// `rustfmt_settings`, and rewrites it. Each edit names a comment by words in
/// its text and gives that comment's new text.
fn rewrite_in_new_folder(
    file: &str,
    source: &str,
    rustfmt_settings: &str,
    edits: &[(&str, Option<&str>)],
) -> Result<Vec<RewrittenFile>, Error> {
    let folder = TempFolder::new();
    fs::write(folder.path().join(file), source).unwrap();
    fs::write(folder.path().join("rustfmt.toml"), rustfmt_settings).unwrap();
    let mut comments = find_comments(Path::new(file), source).unwrap();
    let edits: Vec<CommentEdit> = edits
        .iter()
        .map(|(words, new_text)| {
            let index = comments
                .iter()
                .position(|comment| comment.text.contains(words))
                .unwrap_or_else(|| panic!("no comment holds {words}"));
            CommentEdit {
                comment: comments.swap_remove(index),
                new_text: new_text.map(str::to_owned),
            }
        })
        .collect();
    rewrite(folder.path(), &edits)
}

/// Rewrites `source` with no formatter settings and returns its new contents.
fn rewrite_to_text(file: &str, source: &str, edits: &[(&str, Option<&str>)]) -> String {
    let rewritten = rewrite_in_new_folder(file, source, "", edits).unwrap();
    assert_eq!(rewritten.len(), 1, "{file}");
    assert_eq!(rewritten[0].path, Path::new(file));
    rewritten[0].new_contents.clone()
}

/// `rewrite` must put each new comment text in its comment's place and cut a comment without the code on its line.
/// Otherwise an edit could land in the wrong place, or a cut could take the code beside the comment.
#[test]
fn rewrite_changes_only_the_edited_comments() {
    let cases = [
        (
            "save.go",
            "package main\n\n// Saves it.\nfunc save() int {\n\ttries := 3 // retries\n\treturn tries\n}\n",
            vec![
                ("Saves it", Some("// Saves the answer.\n")),
                ("retries", None),
            ],
            "package main\n\n// Saves the answer.\nfunc save() int {\n\ttries := 3\n\treturn tries\n}\n",
        ),
        (
            "save.py",
            "def save():\n    # Saves it.\n    tries = 3  # retries\n    return tries\n",
            vec![
                ("Saves it", Some("    # Saves the answer.\n")),
                ("retries", None),
            ],
            "def save():\n    # Saves the answer.\n    tries = 3\n    return tries\n",
        ),
        (
            "line.py",
            "def save():\n    \"\"\"Saves it.\"\"\"  # note\n    return 1\n",
            vec![("Saves it", None), ("note", None)],
            "def save():\n    return 1\n",
        ),
        (
            "save.rs",
            "/// Saves it.\nfn save() -> u8 {\n    let tries = 3; // retries\n    tries\n}\n",
            vec![
                ("Saves it", Some("/// Saves the answer.\n")),
                ("retries", None),
            ],
            "/// Saves the answer.\nfn save() -> u8 {\n    let tries = 3;\n    tries\n}\n",
        ),
        (
            "only.rs",
            "// Only a comment.\n",
            vec![("Only", None)],
            "\n",
        ),
    ];

    for (file, source, edits, expected) in cases {
        let new_contents = rewrite_to_text(file, source, &edits);

        assert_eq!(new_contents, expected, "{file}");
    }
}

/// `rewrite` must refuse an edit that changes the code, changes what a tool reads, or writes text the user did not choose.
/// Otherwise User Feedback on one comment could change how the program builds or runs.
#[test]
fn rewrite_refuses_edits_it_cannot_write_safely() {
    let cases = [
        (
            "reword adds code",
            "save.go",
            "package main\n\n// Saves it.\nfunc save() {}\n",
            "",
            ("Saves it", Some("// Saves it.\nfunc extra() {}\n")),
        ),
        (
            "reword becomes a tool line",
            "save.py",
            "# Saves it.\nanswer = 1\n",
            "",
            ("Saves it", Some("# type: ignore\n")),
        ),
        (
            "reword of a keep or cut comment",
            "save.go",
            "package main\n\nvar tries = 3 // retries\n",
            "",
            ("retries", Some("// tries\n")),
        ),
        (
            "cut leaves an empty body",
            "save.py",
            "def save():\n    \"\"\"Saves it.\"\"\"\n",
            "",
            ("Saves it", None),
        ),
        (
            "reword adds a Python warning",
            "save.py",
            "def save():\n    \"\"\"Saves it.\"\"\"\n",
            "",
            ("Saves it", Some("    \"\"\"Matches \\d.\"\"\"\n")),
        ),
        (
            "reword changes the comment's kind",
            "save.rs",
            "// Saves it.\nfn save() {}\n",
            "",
            ("Saves it", Some("/// Saves it.\n")),
        ),
        (
            "new backslash in a file Python cannot compile",
            "save.py",
            "def save():\n    \"\"\"Saves it.\"\"\"\nreturn 1\n",
            "",
            (
                "Saves it",
                Some("    \"\"\"Saves \\N{NOT A NAME} it.\"\"\"\n"),
            ),
        ),
        (
            "reword gives a docstring an f prefix",
            "save.py",
            "def save():\n    \"\"\"Saves it.\"\"\"\n    return 1\n",
            "",
            ("Saves it", Some("    f\"\"\"Saves it.\"\"\"\n")),
        ),
        (
            "cut of a Go block comment",
            "save.go",
            "package main\n\nfunc save(ch chan int) {\n\treturn /*\n\t*/ <-ch\n}\n",
            "",
            ("/*", None),
        ),
        (
            "cut of a Rust block comment",
            "save.rs",
            "fn save() -> u8 {\n    let tries = 3; /* retries */\n    tries\n}\n",
            "",
            ("retries", None),
        ),
        (
            "cut of a doc line inside a macro call",
            "save.rs",
            "fn main() {\n    let x = stringify! {\n        /// doc\n        0\n    };\n    println!(\"{x}\");\n}\n",
            "",
            ("doc", None),
        ),
        (
            "cut of a comment inside an f-string",
            "save.py",
            "x = 1\ns = f\"\"\"{x # note\n=}\"\"\"\n",
            "",
            ("note", None),
        ),
        (
            "cut of a cgo preamble",
            "save.go",
            "package main\n\n// #include <stdio.h>\nimport \"C\"\n",
            "",
            ("#include", None),
        ),
        (
            "cut of a cgo preamble in an import list",
            "save.go",
            "package main\n\nimport (\n\t// #include <stdlib.h>\n\t\"C\"\n)\n",
            "",
            ("#include", None),
        ),
        (
            "cut of a build line",
            "save.go",
            "// +build linux\n\npackage main\n",
            "",
            ("+build", None),
        ),
        (
            "cut of a directive with no space",
            "save.go",
            "package main\n\nvar x = 1 //noinspection GoUnusedGlobalVariable\n",
            "",
            ("noinspection", None),
        ),
        (
            "cut of a nosec mark",
            "save.go",
            "package main\n\nimport (\n\t\"os\"\n\t\"os/exec\"\n)\n\nfunc run() {\n\t// #nosec G204 -- the user picks the command.\n\t_ = exec.Command(os.Args[1])\n}\n",
            "",
            ("nosec", None),
        ),
        (
            "reword becomes a nosec mark",
            "save.go",
            "package main\n\nimport (\n\t\"os\"\n\t\"os/exec\"\n)\n\nfunc run() {\n\t// Runs the command.\n\t_ = exec.Command(os.Args[1])\n}\n",
            "",
            ("Runs", Some("\t// #nosec G204 runs the command.\n")),
        ),
        (
            "cut of a SAFETY comment",
            "save.rs",
            "fn read(pointer: *const u8) -> u8 {\n    // SAFETY: the caller passes a valid pointer.\n    unsafe { *pointer }\n}\n",
            "",
            ("SAFETY", None),
        ),
        (
            "cut of a pyrefly mark",
            "save.py",
            "answer = 1  # pyrefly: ignore\n",
            "",
            ("pyrefly", None),
        ),
        (
            "reword becomes a build line",
            "save.go",
            "// Prose.\n\npackage main\n\nvar x  = 1\n",
            "",
            ("Prose", Some("// +build ignore\n")),
        ),
        (
            "cut of a capitalized noqa",
            "save.py",
            "answer = 1  # NOQA\n",
            "",
            ("NOQA", None),
        ),
        (
            "cut of a pragma with no colon",
            "save.py",
            "def save():  # pragma no cover\n    return 1\n",
            "",
            ("pragma", None),
        ),
        (
            "cut moves an encoding line",
            "save.py",
            "# Prose.\n\n# -*- coding: latin-1 -*-\ns = \"\u{e9}\"\nprint(len(s))\n",
            "",
            ("Prose", None),
        ),
        (
            "cut moves a #! line",
            "save.py",
            "# Prose.\n#!/usr/bin/env python3\nprint(1)\n",
            "",
            ("Prose", None),
        ),
        (
            "cut of an example's expected output",
            "save_test.go",
            "package main\n\nimport \"fmt\"\n\nfunc ExampleSave() {\n\tfmt.Println(1)\n\t// Output: 1\n}\n",
            "",
            ("Output", None),
        ),
        (
            "formatter changes the edited comment",
            "save.go",
            "package main\n\n// Saves it.\nfunc save() {}\n",
            "",
            ("Saves it", Some("//\ta\n")),
        ),
        (
            "reword changes the text direction",
            "save.go",
            "package main\n\n// Saves it.\nfunc save() {}\n",
            "",
            ("Saves it", Some("// Saves \u{202E}it.\n")),
        ),
        (
            "formatter changes code",
            "save.rs",
            "struct Point {\n    x: u8,\n}\n\nfn make(x: u8) -> Point {\n    Point {\n        x: // c\n            x,\n    }\n}\n",
            "use_field_init_shorthand = true\n",
            ("// c", None),
        ),
    ];

    for (case, file, source, rustfmt_settings, edit) in cases {
        let error = rewrite_in_new_folder(file, source, rustfmt_settings, &[edit]).unwrap_err();

        assert_eq!(
            error.code,
            ErrorCode::WouldNotWrite,
            "{case}: {}",
            error.message
        );
    }
}

/// `rewrite` must run the formatter only on a file the formatter would leave as it is.
/// Otherwise User Feedback on one comment could reformat code the user wrote.
#[test]
fn rewrite_formats_only_already_formatted_files() {
    let formatted = "package main\n\nfunc save() {\n\ta := 1          // one\n\tbbbbbbbbbb := 2 // two\n\t_, _ = a, bbbbbbbbbb\n}\n";
    let unformatted = "package main\n\nfunc save() {\n\ta := 1          // one\n\tbbbbbbbbbb := 2 // two\n\t_,_ = a,bbbbbbbbbb\n}\n";

    let from_formatted = rewrite_to_text("save.go", formatted, &[("two", None)]);
    let from_unformatted = rewrite_to_text("save.go", unformatted, &[("two", None)]);

    assert_eq!(
        from_formatted,
        "package main\n\nfunc save() {\n\ta := 1 // one\n\tbbbbbbbbbb := 2\n\t_, _ = a, bbbbbbbbbb\n}\n"
    );
    assert_eq!(
        from_unformatted,
        "package main\n\nfunc save() {\n\ta := 1          // one\n\tbbbbbbbbbb := 2\n\t_,_ = a,bbbbbbbbbb\n}\n"
    );
}

/// `rewrite` must allow a Python warning the file already had before the edit.
/// Otherwise every edit to a comment in that file would be refused.
#[test]
fn rewrite_allows_warnings_file_already_had() {
    let warned = "DIGIT = \"\\d\"\n# Saves it.\nanswer = 1\n";

    let from_warned = rewrite_to_text("save.py", warned, &[("Saves it", None)]);

    assert_eq!(from_warned, "DIGIT = \"\\d\"\nanswer = 1\n");
}

/// `rewrite` must still edit a Python file that this machine's Python cannot compile, such as one that needs a newer Python.
/// Otherwise the user could not change any comment in that file.
#[test]
fn rewrite_edits_files_python_cannot_compile() {
    let not_compiled = "# Saves it.\nanswer = 1\nreturn answer\n";

    let new_contents = rewrite_to_text("save.py", not_compiled, &[("Saves it", None)]);

    assert_eq!(new_contents, "answer = 1\nreturn answer\n");
}

/// `rewrite` must refuse an edit whose comment no longer matches the file.
/// Otherwise it could cut or write over text the user never saw.
#[test]
fn rewrite_refuses_comment_changed_since_read() {
    let folder = TempFolder::new();
    let read_source = "package main\n\n// Saves it.\nfunc save() {}\n";
    let comment = find_comments(Path::new("save.go"), read_source)
        .unwrap()
        .remove(0);
    // Another edit changes the comment after the review read it.
    let changed_source = "package main\n\n// Saves it twice.\nfunc save() {}\n";
    fs::write(folder.path().join("save.go"), changed_source).unwrap();
    let edit = CommentEdit {
        comment,
        new_text: None,
    };

    let error = rewrite(folder.path(), &[edit]).unwrap_err();

    assert_eq!(error.code, ErrorCode::TextChanged, "{}", error.message);
}
