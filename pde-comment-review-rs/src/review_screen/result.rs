//! This module holds the Review Result Screen, which says whether the User
//! Feedback went into the code, or why not and what to do next. The screen
//! shows its own words for each error code, never the message written for
//! agents, except why a write failed. Read the `write_and_explain(root)` and
//! `ResultScreen::build_screen()` functions to understand the module.

use std::path::{Path, PathBuf};

use super::draw::{Row, Style, fit_label};
use super::fit::{clip, display_width, escape_control_characters, fold};
use super::keys::Key;
use super::screen::{AfterKey, HEADER_ROWS, KEY_HELP_ROWS};
use crate::shared::git;
use crate::shared::review_session::{self, ReviewOrigin, ReviewStatus};
use crate::shared::{Error, ErrorCode};
use crate::steps::write_user_feedback::write_user_feedback;

/// Shows once every answer is in the code.
const DONE: &str = "Done. Your answers are in the code. Press d to see the changes.";
/// Shows once the last User Feedback hands the Review Session to the agent.
const NOTES_WAIT: &str =
    "Your User Notes went to the agent, so nothing is written yet. Tell the agent to answer them.";
/// Shows when `open NAME` finds a dismissed review.
const DISMISSED: &str =
    "This review was dismissed. Run pde-comment-review restore NAME to answer it.";
/// Shows after step 7 refuses with `text_changed`.
const TEXT_CHANGED: &str = "A comment changed after the agent sent it, so nothing was written. Those Feedback Requests went back to the agent. Tell the agent to send the list again.";
/// Shows after step 7 refuses with `write_failed`, and the list waits again.
const WRITE_FAILED: &str =
    "The last write failed, and every file was put back. Open the review again to try again.";
/// Shows when step 7 wrote every file but could not save `done`.
const WRITTEN_NOT_MARKED_DONE: &str = "Every answer was written, but the review could not be marked done. Do not put back the original files. Check the disk and the folder's permissions.";

/// Says whether the answers are in the code.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum ResultStatus {
    Done,
    NotWritten,
}

/// Holds what the Review Result Screen says.
#[derive(Debug, PartialEq)]
pub(super) struct ReviewResult {
    pub(super) status: ResultStatus,
    pub(super) message: String,
}

impl ReviewResult {
    /// Returns a result that says nothing was written, and why.
    pub(super) fn not_written(message: impl Into<String>) -> Self {
        Self {
            status: ResultStatus::NotWritten,
            message: message.into(),
        }
    }

    /// Returns what to say about a Review Session that no longer waits for
    /// User Feedback, or `None` while it waits.
    pub(super) fn from_status(root: &Path, status: ReviewStatus) -> Option<Self> {
        match status {
            ReviewStatus::WaitingForUserFeedback => None,
            ReviewStatus::Done => Some(Self {
                status: ResultStatus::Done,
                message: DONE.to_owned(),
            }),
            ReviewStatus::UserNotesWaiting => Some(Self::not_written(NOTES_WAIT)),
            ReviewStatus::Dismissed => Some(Self::not_written(DISMISSED)),
            ReviewStatus::WritingUserFeedback => {
                Some(Self::not_written(explain_unfinished_write(root)))
            }
        }
    }

    /// Returns what to say about an error, in the user's words, with what to
    /// do next.
    pub(super) fn from_error(root: &Path, error: &Error) -> Self {
        let message = match error.code {
            ErrorCode::TextChanged => TEXT_CHANGED,
            ErrorCode::WriteFailed => WRITE_FAILED,
            ErrorCode::WriteNotFinished => {
                return Self::not_written(explain_unfinished_write(root));
            }
            ErrorCode::NothingToWrite | ErrorCode::LateUserFeedback => {
                "This review stopped taking answers, perhaps on another screen, so nothing was saved. Open the review again to see where it stands."
            }
            ErrorCode::RequestChanged => {
                "This Feedback Request changed after the screen showed it, so nothing was saved. Read it again, then answer."
            }
            ErrorCode::SavedFileUnreadable => {
                "A saved review file can't be read, so it was moved aside. Ask the agent to send the list again."
            }
            ErrorCode::UnknownSavedField => {
                "A newer version saved this review. Run pde-installer, then open the review again."
            }
            ErrorCode::SaveFailed => {
                "The review could not be saved. Check the disk and the folder's permissions, then open the review again."
            }
            ErrorCode::NotAGitCheckout => {
                "This folder is not in a Git worktree. Open the review from its worktree."
            }
            ErrorCode::FileUnreadable | ErrorCode::WouldNotWrite => {
                "A file can't take these answers now, so nothing was written. Tell the agent, then open the review again."
            }
            // The other codes refuse agent calls, which the screen never makes.
            _ => "Something went wrong, so nothing was written. Ask the agent to check the review.",
        };
        Self::not_written(message)
    }
}

/// Runs step 7 and returns what the Review Result Screen says about it.
pub(super) fn write_and_explain(root: &Path) -> ReviewResult {
    match write_user_feedback(root) {
        Ok(_) => ReviewResult {
            status: ResultStatus::Done,
            message: DONE.to_owned(),
        },
        Err(error) => explain_write_error(root, &error),
    }
}

/// Returns what to say after step 7 refused with `error`. A refusal after a
/// failed write gives its reason on the rows below the screen's own words.
fn explain_write_error(root: &Path, error: &Error) -> ReviewResult {
    if matches!(
        error.code,
        ErrorCode::TextChanged | ErrorCode::WriteFailed | ErrorCode::WriteNotFinished
    ) {
        let mut result = ReviewResult::from_error(root, error);
        // A lasting cause, such as a file left in the way, fails every retry the same way.
        if error.code == ErrorCode::WriteFailed {
            result.message = format!("{}\n{}", result.message, error.message);
        }
        return result;
    }
    // A save or waiting-list error can come after the write, so the saved status says what happened.
    let status = match review_session::load(root) {
        Ok(Some(review_session)) => Some(review_session.status),
        _ => None,
    };
    // Step 7 removes the original files after a put-back or a finished write, so finding them here means each file holds its answer.
    let originals_kept = find_backup_folder(root).is_some_and(|folder| folder.exists());
    if error.code == ErrorCode::SaveFailed
        && status.is_none_or(|status| status == ReviewStatus::WritingUserFeedback)
        && originals_kept
    {
        return ReviewResult {
            status: ResultStatus::Done,
            message: WRITTEN_NOT_MARKED_DONE.to_owned(),
        };
    }
    status
        .and_then(|status| ReviewResult::from_status(root, status))
        .unwrap_or_else(|| ReviewResult::from_error(root, error))
}

/// Returns what to say when a write did not finish, naming the folder that
/// holds the original files while it is there.
fn explain_unfinished_write(root: &Path) -> String {
    let start = "The last write did not finish, so some files may have changed. Check git status.";
    match find_backup_folder(root).filter(|folder| folder.exists()) {
        Some(folder) => format!(
            "{start} The original files are in {}. Keep each file that already holds your answers.",
            folder.display()
        ),
        None => start.to_owned(),
    }
}

/// Returns the folder where step 7 keeps the original files while it writes.
fn find_backup_folder(root: &Path) -> Option<PathBuf> {
    let folder = git::find_worktree_git_folder(root).ok()?;
    Some(folder.join("pde/write-backup"))
}

/// Shows a Review Result, with `d` for the diff of the review's files.
pub(super) struct ResultScreen {
    origin: Option<ReviewOrigin>,
    /// Counts the Review Session's Feedback Requests, or `None` without one.
    request_count: Option<usize>,
    pub(super) result: ReviewResult,
    /// Holds the worktree's top folder and the review's files, which `d`
    /// shows the changes in.
    pub(super) diff: Option<(PathBuf, Vec<PathBuf>)>,
    scroll: usize,
    pub(super) cols: usize,
    pub(super) rows: usize,
    pub(super) notice: String,
}

impl ResultScreen {
    /// Returns a Review Result Screen. Without an origin it names no review.
    pub(super) fn new(
        origin: Option<ReviewOrigin>,
        request_count: Option<usize>,
        result: ReviewResult,
        diff: Option<(PathBuf, Vec<PathBuf>)>,
        (cols, rows): (usize, usize),
    ) -> Self {
        Self {
            origin,
            request_count,
            result,
            diff,
            scroll: 0,
            cols,
            rows,
            notice: String::new(),
        }
    }

    /// Acts on one key. `d` shows the diff, and `q` or Ctrl-C closes.
    pub(super) fn press_key(&mut self, key: Key) -> AfterKey {
        self.notice.clear();
        match key {
            Key::Interrupt | Key::Char('q') => return AfterKey::Quit,
            Key::Char('j') | Key::ScrollDown => {
                let max = self.build_body().len().saturating_sub(self.body_room());
                self.scroll = (self.scroll + 1).min(max);
            }
            Key::Char('k') | Key::ScrollUp => self.scroll = self.scroll.saturating_sub(1),
            Key::Char('d') if self.diff.is_some() => return AfterKey::Diff,
            _ => {}
        }
        AfterKey::Stay
    }

    /// Returns how many body rows fit between the header and the key help.
    fn body_room(&self) -> usize {
        self.rows.saturating_sub(HEADER_ROWS + KEY_HELP_ROWS).max(1)
    }

    /// Returns the message folded to fit after a one-column mark. Each line
    /// of the message starts a new row.
    fn build_body(&self) -> Vec<String> {
        let width = self.cols.saturating_sub(1).max(10);
        self.result
            .message
            .lines()
            .flat_map(|line| fold(&escape_control_characters(line), width))
            .map(|row| format!(" {row}"))
            .collect()
    }

    /// Returns one row per terminal row, none wider than the terminal.
    pub(super) fn build_screen(&self) -> Vec<Row> {
        let label = match &self.origin {
            Some(origin) => fit_label(origin, self.cols),
            None => clip("comment review", self.cols).to_owned(),
        };
        let status = match self.result.status {
            ResultStatus::Done => "done",
            ResultStatus::NotWritten => "not written",
        };
        let title = match self.request_count {
            Some(count) => format!("all {count} answered"),
            None => String::new(),
        };
        let width = self.cols.saturating_sub(display_width(status) + 1);
        let title = clip(&title, width);
        let padding = " ".repeat(width - display_width(title));
        let mut screen = vec![
            Row::new(Style::Plain, label),
            Row(vec![
                (Style::Title, format!("{title}{padding}")),
                (Style::Plain, " ".to_owned()),
                (Style::Status, status.to_owned()),
            ]),
        ];
        let body = self.build_body();
        let shown = body.into_iter().skip(self.scroll).take(self.body_room());
        screen.extend(shown.map(|row| Row::new(Style::Plain, row)));
        while screen.len() < self.rows.saturating_sub(KEY_HELP_ROWS) {
            screen.push(Row::new(Style::Plain, String::new()));
        }
        if !self.notice.is_empty() {
            // A long Review Notice wraps upward over the body, never over the header.
            let notice = fold(&escape_control_characters(&self.notice), self.cols);
            let room_below_header = screen.len().saturating_sub(HEADER_ROWS);
            let notice = &notice[notice.len().saturating_sub(room_below_header)..];
            let first = screen.len() - notice.len();
            for (row, text) in screen[first..].iter_mut().zip(notice) {
                *row = Row::new(Style::Note, text.clone());
            }
        }
        let hint = |keys: &str| Row::new(Style::Faded, clip(keys, self.cols).to_owned());
        let first_hint = if self.diff.is_some() {
            "d diff q quit"
        } else {
            "q quit"
        };
        screen.push(hint(first_hint));
        screen.push(hint("j k scroll"));
        screen
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use serde_json::json;

    use super::*;
    use crate::review_screen::test_repo::MadeUpRepo;

    /// `explain_write_error` must say every answer was written after a failed
    /// save only while the original files are kept, and must not name their
    /// folder once it is gone. Otherwise the user puts the originals back over
    /// written answers, trusts answers never written, or looks for a folder
    /// that is gone.
    #[test]
    fn explain_write_error_tells_written_from_unwritten() {
        let cases = [
            // Step 7 wrote every file, then could not save `done`.
            (
                "originals kept",
                true,
                ResultStatus::Done,
                WRITTEN_NOT_MARKED_DONE,
            ),
            // Step 7 put every file back, then could not save that the review waits again.
            (
                "originals put back",
                false,
                ResultStatus::NotWritten,
                "The last write did not finish, so some files may have changed. Check git status.",
            ),
        ];
        for (case, originals_kept, status, message) in cases {
            let repo = MadeUpRepo::new("feat/screen", &[]);
            repo.save_review("writing_user_feedback", vec![], json!({}));
            if originals_kept {
                fs::create_dir(repo.path().join(".git/pde/write-backup")).unwrap();
            }
            let error = Error {
                code: ErrorCode::SaveFailed,
                message: "The Review Session could not be saved.".to_owned(),
            };

            let result = explain_write_error(repo.path(), &error);

            assert_eq!(
                result,
                ReviewResult {
                    status,
                    message: message.to_owned(),
                },
                "{case}"
            );
        }
    }

    /// `ResultScreen::build_screen` must give 16 rows of at most 40 columns
    /// on the phone popup, even for the long message about an unfinished
    /// write. Otherwise the terminal wraps a row and the end of the message,
    /// which says what to do, falls off the phone screen.
    #[test]
    fn build_screen_fits_phone_popup() {
        let origin = ReviewOrigin {
            repo: "personal-dev-env".to_owned(),
            branch: "feat/rust-review-screen-pieces-for-phones".to_owned(),
            agent_session: "ses_123".to_owned(),
        };
        let message = "The last write did not finish, so some files may have changed. Check git status. The original files are in /home/user/Projects/personal-dev-env/.git/worktrees/feat-rust-review/pde/write-backup. Keep each file that already holds your answers.";
        let result = ReviewResult::not_written(message);
        let diff = Some((PathBuf::from("/repo"), vec![PathBuf::from("store.go")]));

        let screen = ResultScreen::new(Some(origin), Some(12), result, diff, (40, 16));
        let rows = screen.build_screen();

        assert_eq!(rows.len(), 16);
        for row in &rows {
            let text: String = row.0.iter().map(|(_, text)| text.as_str()).collect();
            assert!(display_width(&text) <= 40, "{text:?}");
        }
    }
}
