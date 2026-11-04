//! This module runs nvim, less and rg in place, in the screen's own pane, for
//! the `e`, `v`, `s` and `d` keys. The caller hands them the terminal first and
//! takes it back after. Read the `ReviewScreen::edit_comment()`,
//! `ReviewScreen::view_file()`, `search_repository(root, text)` and
//! `show_diff(root, files, cols)` functions to understand the module.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{self, Command, Stdio};
use std::thread;

use super::screen::{AfterKey, ReviewScreen};
use crate::shared::comments::to_plain_words;
use crate::shared::git;
use crate::shared::review_session::SuggestionKind;
use crate::shared::user_feedback::{UserAnswer, UserFeedback};

/// Ends the words on the Comment Edit Page. The code below it is there to
/// read, and edits to it are ignored.
const EDIT_MARKER: &str = "# ---- edits below are ignored ----";

/// How many lines of the code below the comment the Comment Edit Page shows.
const EDIT_PAGE_CODE_LINES: usize = 30;

/// Says what the user left on the Comment Edit Page.
#[derive(Debug, PartialEq)]
pub(super) enum EditedPage {
    /// Holds the user's new words.
    Words(String),
    /// Says every word was deleted, which cuts the comment.
    NoWords,
    /// Says the words are as they were, so nothing is saved.
    Unchanged,
    /// Says the marker line is gone, so the words cannot be told from the code.
    MarkerDeleted,
}

impl ReviewScreen<'_> {
    /// Opens the shown request's words on the Comment Edit Page in nvim, and
    /// gives the request the user's new words as an answer once nvim closes.
    /// Words left as they were save nothing, and no words cut the comment.
    pub(super) fn edit_comment(&mut self) -> AfterKey {
        let code = match &self.code {
            Ok(code) => code,
            Err(not_shown) => {
                self.notice = not_shown.message().to_owned();
                return AfterKey::Stay;
            }
        };
        let start_words = self.find_start_words();
        let below_end = code.below.len().min(EDIT_PAGE_CODE_LINES);
        let page = build_edit_page(
            &start_words,
            code.definition_line.as_deref(),
            &code.below[..below_end],
        );
        let edited = match edit_in_nvim(self.root, &page) {
            Ok(saved) => read_edit_page(&saved, &start_words),
            Err(notice) => {
                self.notice = notice.to_owned();
                return AfterKey::Stay;
            }
        };
        match edited {
            EditedPage::Words(words) => self.answer(UserAnswer::Edited { text: words }),
            EditedPage::NoWords => self.answer(UserAnswer::Cut),
            EditedPage::Unchanged => AfterKey::Stay,
            EditedPage::MarkerDeleted => {
                self.notice = "The line starting with # ---- was deleted, so nothing was saved. Press e to edit again.".to_owned();
                AfterKey::Stay
            }
        }
    }

    /// Returns the words the Comment Edit Page starts with. The user's own
    /// words come first, then the Agent Suggestion's, then the comment's.
    fn find_start_words(&self) -> String {
        let request = self.shown_request();
        if let Some(UserFeedback::Answer(UserAnswer::Edited { text })) =
            self.review_session.feedback.get(&request.id)
        {
            return text.clone();
        }
        match request
            .suggestion
            .as_ref()
            .map(|suggestion| &suggestion.kind)
        {
            Some(SuggestionKind::Reword { text }) => text.clone(),
            _ => to_plain_words(&request.file, &request.text),
        }
    }

    /// Opens the shown request's file read-only in nvim at its comment.
    pub(super) fn view_file(&mut self) {
        let Ok(code) = &self.code else {
            return;
        };
        let opened = Command::new("nvim")
            .arg("-R")
            .arg(format!("+{}", code.comment.line))
            .arg(&self.shown_request().file)
            .current_dir(self.root)
            .status();
        if opened.is_err() {
            self.notice = "nvim can't start. Install it, then press v again.".to_owned();
        }
    }
}

/// Returns the Comment Edit Page, with the words, the marker, then a
/// docstring's Function Definition Line and the code below the comment.
pub(super) fn build_edit_page(
    words: &str,
    definition_line: Option<&str>,
    code_below: &[String],
) -> String {
    let mut page = format!("{}\n{EDIT_MARKER}\n", words.trim_end());
    for line in definition_line
        .into_iter()
        .chain(code_below.iter().map(String::as_str))
    {
        page.push_str(line);
        page.push('\n');
    }
    page
}

/// Returns what the user left on the page, compared with the words it
/// started with.
pub(super) fn read_edit_page(saved: &str, start_words: &str) -> EditedPage {
    // Without the marker, the code below it would become part of the comment.
    let Some((words, _)) = saved.split_once(EDIT_MARKER) else {
        return EditedPage::MarkerDeleted;
    };
    let words = words.trim();
    if words == start_words.trim() {
        EditedPage::Unchanged
    } else if words.is_empty() {
        EditedPage::NoWords
    } else {
        EditedPage::Words(words.to_owned())
    }
}

/// Writes the page beside `review.json`, opens it in nvim, and returns the
/// saved page, or the Review Notice that says why nothing was saved.
fn edit_in_nvim(root: &Path, page: &str) -> Result<String, &'static str> {
    const CANNOT_SAVE: &str =
        "The edit page can't be saved, so nothing was saved. Check the disk, then press e again.";
    let folder = git::find_worktree_git_folder(root).map_err(|_| CANNOT_SAVE)?;
    let path = folder
        .join("pde")
        .join(format!("edit-page-{}.txt", process::id()));
    fs::create_dir_all(folder.join("pde")).map_err(|_| CANNOT_SAVE)?;
    fs::write(&path, page).map_err(|_| CANNOT_SAVE)?;
    let edited = run_nvim_on_page(&path);
    let _ = fs::remove_file(&path);
    edited
}

/// Runs nvim on the page and reads what it saved.
fn run_nvim_on_page(path: &Path) -> Result<String, &'static str> {
    let status = Command::new("nvim")
        .args(["-c", "setlocal wrap linebreak nolist spell"])
        .arg(path)
        .status()
        .map_err(|_| "nvim can't start, so nothing was saved. Install it, then press e again.")?;
    if !status.success() {
        return Err("nvim quit with an error, so nothing was saved. Press e to edit again.");
    }
    fs::read_to_string(path)
        .map_err(|_| "The edit page can't be read, so nothing was saved. Press e to edit again.")
}

/// Shows where `text` appears in the repository, taken literally, through
/// `rg` in less. Returns the Review Notice when the search cannot run.
pub(super) fn search_repository(root: &Path, text: &str) -> Result<(), &'static str> {
    const FAILED: &str = "The search failed. Check that rg is installed, then press s again.";
    let found = Command::new("rg")
        .args(["--color=always", "-n", "-S", "-F", "-C2", "--", text, "."])
        .current_dir(root)
        .output()
        .map_err(|_| FAILED)?;
    // rg exits 1 with nothing on stderr when nothing matches.
    let shown = match found.status.code() {
        Some(0) => found.stdout,
        Some(1) if found.stderr.is_empty() => format!("No match for {text}.\n").into_bytes(),
        _ => return Err(FAILED),
    };
    show_in_less(&shown)
}

/// Shows the uncommitted changes in the review's files in less, through
/// delta when it is installed.
pub(super) fn show_diff(root: &Path, files: &[PathBuf], cols: usize) -> Result<(), &'static str> {
    let diff = git::read_diff_of_files(root, files)
        .map_err(|_| "The changes can't be read. Run git diff in the worktree.")?;
    if diff.is_empty() {
        return show_in_less(b"No changes. Every comment stayed as it was.\n");
    }
    let colored = Command::new("delta")
        .args(["--paging=never", &format!("--width={cols}")])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()
        .and_then(|mut delta| {
            let mut input = delta.stdin.take()?;
            let diff = &diff;
            // Writing on its own thread lets delta print while it reads, so a long diff cannot fill both pipes.
            let output = thread::scope(|scope| {
                scope.spawn(move || {
                    let _ = input.write_all(diff);
                });
                delta.wait_with_output()
            })
            .ok()?;
            output.status.success().then_some(output.stdout)
        });
    show_in_less(colored.as_deref().unwrap_or(&diff))
}

/// Pages `text` in `less -R`, which keeps its colors.
fn show_in_less(text: &[u8]) -> Result<(), &'static str> {
    const FAILED: &str = "less can't start. Install it, then try again.";
    let mut less = Command::new("less")
        .arg("-R")
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|_| FAILED)?;
    if let Some(mut input) = less.stdin.take() {
        // less may quit before reading everything, which is not a failure.
        let _ = input.write_all(text);
    }
    less.wait().map_err(|_| FAILED)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `read_edit_page` must take only the words above the marker, and save
    /// nothing when the marker is gone or the words did not change. Otherwise
    /// code below the marker ends up in the comment, or an untouched page
    /// replaces the agent's words.
    #[test]
    fn read_edit_page_tells_what_user_left() {
        let code = "func Save() {}\n";
        let cases = [
            (
                "new words",
                format!("New words.\n{EDIT_MARKER}\n{code}"),
                EditedPage::Words("New words.".to_owned()),
            ),
            (
                "no words",
                format!("\n{EDIT_MARKER}\n{code}"),
                EditedPage::NoWords,
            ),
            (
                "unchanged",
                format!("Old words.\n{EDIT_MARKER}\n{code}"),
                EditedPage::Unchanged,
            ),
            (
                "marker deleted",
                format!("New words.\n{code}"),
                EditedPage::MarkerDeleted,
            ),
        ];

        for (case, saved, expected) in cases {
            assert_eq!(read_edit_page(&saved, "Old words."), expected, "{case}");
        }
    }

    /// `build_edit_page` must show a docstring's Function Definition Line
    /// just below the marker, above the code. Otherwise the user rewrites a
    /// docstring without seeing what it describes.
    #[test]
    fn build_edit_page_shows_definition_line() {
        let code_below = ["    return note.strip()".to_owned()];

        let page = build_edit_page("Strips the note.", Some("def save(note):"), &code_below);

        assert_eq!(
            page,
            format!("Strips the note.\n{EDIT_MARKER}\ndef save(note):\n    return note.strip()\n")
        );
    }
}
