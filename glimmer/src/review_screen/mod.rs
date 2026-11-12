//! This module shows a Review Session one Feedback Request at a time, sized
//! for a 40x16 phone popup, and saves each piece of User Feedback as soon as
//! its key is pressed. After the last one it runs step 7 and shows the Review
//! Result Screen. Read the `run_screen_command(kind)`,
//! `run_open_command(branch)` and `show_review_screen(dir)` functions to
//! understand the module.

mod draw;
mod fit;
mod herdr;
mod in_place;
mod keys;
mod picker;
mod result;
mod saving;
mod screen;
mod stale_reviews;
mod start;
mod surrounding_code;
#[cfg(test)]
mod test_repo;

use std::collections::HashSet;
use std::io::{self, Write};
use std::path::Path;
use std::time::{Duration, Instant};

use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::event;
use crossterm::terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::{execute, queue};

use crate::shared::git;
use crate::shared::review_session::{self, ReviewSession};
use draw::Row;
use picker::{AfterPick, ReviewSessionPicker, Waiting, WaitingReview};
use result::{ResultScreen, ReviewResult, write_and_explain};
use screen::{AfterKey, ReviewScreen};

pub(crate) use start::find_worktrees_by_branch;
pub use start::{run_open_command, run_screen_command};

/// Turns on mouse reports for taps and wheel turns only, sent as text such as
/// `\x1b[<65;20;5M`. A phone app sends a swipe as wheel turns.
const MOUSE_ON: &str = "\x1b[?1000h\x1b[?1006h";
const MOUSE_OFF: &str = "\x1b[?1006l\x1b[?1000l";

/// Says why a command could not show the screen, in words for the user.
#[derive(Debug)]
pub struct StartFailure(pub String);

/// Shows the Review Session of the worktree that holds `dir` in this
/// terminal, and saves each piece of User Feedback as it is given. Runs step 7
/// once every Feedback Request has an answer.
///
/// # Errors
///
/// Fails only when the terminal cannot be read or drawn on. The screen shows
/// every other failure itself, in the user's words.
pub fn show_review_screen(dir: &Path) -> io::Result<()> {
    let mut terminal = RawTerminal::start()?;
    show_review(&mut terminal, dir)
}

/// Shows the Review Session in `dir` on a terminal that is already raw.
fn show_review(terminal: &mut RawTerminal, dir: &Path) -> io::Result<()> {
    let size = read_size()?;
    let root = match git::find_root(dir) {
        Ok(root) => root,
        Err(error) => {
            let result = ReviewResult::from_error(dir, &error);
            return run_result_screen(terminal, ResultScreen::new(None, None, result, None, size));
        }
    };
    let mut review_session = match review_session::load(&root) {
        Ok(Some(review_session)) => review_session,
        Ok(None) => {
            let result = ReviewResult::not_written(
                "No review waits in this worktree. An agent sends one for you to answer.",
            );
            return run_result_screen(terminal, ResultScreen::new(None, None, result, None, size));
        }
        Err(error) => {
            let result = ReviewResult::from_error(&root, &error);
            return run_result_screen(terminal, ResultScreen::new(None, None, result, None, size));
        }
    };
    let result = match find_result_before_review(&root, &review_session) {
        Some(result) => Some(result),
        None => {
            let mut screen = ReviewScreen::new(&root, &mut review_session, size)
                .expect("a request waits for User Feedback");
            run_review_loop(terminal, &mut screen)?
        }
    };
    let Some(result) = result else {
        return Ok(());
    };
    let requests = review_session.newest_requests();
    let mut files: Vec<_> = requests
        .iter()
        .map(|request| request.file.clone())
        .collect();
    files.sort();
    files.dedup();
    let request_count = Some(requests.len());
    let origin = Some(review_session.origin.clone());
    let screen = ResultScreen::new(origin, request_count, result, Some((root, files)), size);
    run_result_screen(terminal, screen)
}

/// Returns what the Review Result Screen says about a Review Session with
/// nothing for the user to answer, or `None` while a request waits. Runs step 7
/// when every request has an answer, so opening the review again retries a write.
fn find_result_before_review(root: &Path, review_session: &ReviewSession) -> Option<ReviewResult> {
    if let Some(result) = ReviewResult::from_status(root, review_session.status) {
        return Some(result);
    }
    let waits_for_user = review_session
        .newest_requests()
        .iter()
        .any(|request| !review_session.feedback.contains_key(&request.id));
    if waits_for_user {
        return None;
    }
    Some(write_and_explain(root))
}

/// Reads keys until the user quits, or until the Review Session ends on a
/// Review Result, which it returns.
fn run_review_loop(
    terminal: &mut RawTerminal,
    screen: &mut ReviewScreen,
) -> io::Result<Option<ReviewResult>> {
    loop {
        let (cols, rows) = read_size()?;
        screen.set_size(cols, rows);
        draw(&screen.build_screen())?;
        if screen.is_waiting_for_draw() {
            // Drops keys pressed before this request showed, such as a second
            // tap read while the last answer was checked, but not Ctrl-C.
            if drop_waiting_keys()? {
                return Ok(None);
            }
            screen.start_wait(Instant::now());
        }
        let Some(key) = keys::read_key(&event::read()?) else {
            continue;
        };
        let mut after = screen.press_key(key, Instant::now());
        // A save or an edit can lead to another save or to the result.
        loop {
            after = match after {
                AfterKey::Stay | AfterKey::Diff => break,
                AfterKey::Quit => return Ok(None),
                AfterKey::ShowResult(result) => return Ok(Some(result)),
                AfterKey::Save(id) => screen.save_given_feedback(id),
                AfterKey::Edit => terminal.run_in_place(|| screen.edit_comment())?,
                AfterKey::View => {
                    terminal.run_in_place(|| screen.view_file())?;
                    AfterKey::Stay
                }
                AfterKey::Search(text) => {
                    let searched = terminal
                        .run_in_place(|| in_place::search_repository(screen.root, &text))?;
                    if let Err(notice) = searched {
                        screen.notice = notice.to_owned();
                    }
                    AfterKey::Stay
                }
            };
        }
        if terminal.was_left {
            // Keys typed into nvim or less after it closed must not answer anything.
            terminal.was_left = false;
            screen.wait_for_draw();
        }
    }
}

/// Shows the Review Result Screen until the user closes it.
fn run_result_screen(terminal: &mut RawTerminal, mut screen: ResultScreen) -> io::Result<()> {
    // A tap made while step 7 ran must not close the result before it is read.
    if drop_waiting_keys()? {
        return Ok(());
    }
    loop {
        (screen.cols, screen.rows) = read_size()?;
        draw(&screen.build_screen())?;
        let Some(key) = keys::read_key(&event::read()?) else {
            continue;
        };
        match screen.press_key(key) {
            AfterKey::Quit => return Ok(()),
            AfterKey::Diff => {
                let (root, files) = screen.diff.as_ref().expect("d works only with a diff");
                let cols = screen.cols;
                if let Err(notice) =
                    terminal.run_in_place(|| in_place::show_diff(root, files, cols))?
                {
                    screen.notice = notice.to_owned();
                }
                if drop_waiting_keys()? {
                    return Ok(());
                }
            }
            _ => {}
        }
    }
}

/// Shows the Review Session Picker on the raw terminal, or picks the only
/// review when its agent session still runs or is unknown, and returns the
/// chosen one. Returns `None` when none waits or the user quits.
/// `live_sessions` holds the agent sessions Herdr runs now, or `None` when
/// Herdr cannot tell.
fn pick_review(
    reviews: Vec<WaitingReview>,
    live_sessions: Option<HashSet<String>>,
) -> io::Result<Option<WaitingReview>> {
    let mut picker = ReviewSessionPicker::new(reviews, read_size()?, live_sessions);
    if picker.is_empty() {
        return Ok(None);
    }
    // A lone review whose agent session ended still opens the picker to say so.
    if !picker.is_choice_needed() {
        return Ok(Some(picker.into_chosen()));
    }
    if drop_waiting_keys()? {
        return Ok(None);
    }
    loop {
        (picker.cols, picker.rows) = read_size()?;
        draw(&picker.build_screen())?;
        let Some(key) = keys::read_key(&event::read()?) else {
            continue;
        };
        match picker.press_key(key) {
            AfterPick::Stay => {}
            AfterPick::Quit => return Ok(None),
            AfterPick::Open => return Ok(Some(picker.into_chosen())),
            AfterPick::Dismiss => {
                picker.dismiss_chosen();
                if picker.is_empty() {
                    return Ok(None);
                }
            }
        }
    }
}

/// Shows the chosen Review Session, or why its saved file could not be read.
fn show_picked_review(terminal: &mut RawTerminal, picked: WaitingReview) -> io::Result<()> {
    match picked.waiting {
        Waiting::Review { .. } | Waiting::WriteNotFinished { .. } => {
            show_review(terminal, &picked.root)
        }
        Waiting::Unreadable(error) => {
            let result = ReviewResult::from_error(&picked.root, &error);
            let screen = ResultScreen::new(None, None, result, None, read_size()?);
            run_result_screen(terminal, screen)
        }
    }
}

/// Returns the terminal's columns and rows.
fn read_size() -> io::Result<(usize, usize)> {
    let (cols, rows) = terminal::size()?;
    Ok((usize::from(cols), usize::from(rows)))
}

/// Reads every key already waiting and drops it. Returns true when one was
/// Ctrl-C, which still quits.
fn drop_waiting_keys() -> io::Result<bool> {
    while event::poll(Duration::ZERO)? {
        if keys::is_waiting_quit(&event::read()?) {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Draws every row over the last ones.
fn draw(rows: &[Row]) -> io::Result<()> {
    let mut out = io::stdout();
    let lines: Vec<String> = rows.iter().map(Row::to_ansi).collect();
    queue!(out, MoveTo(0, 0), Clear(ClearType::All))?;
    out.write_all(lines.join("\r\n").as_bytes())?;
    out.flush()
}

/// Holds the terminal in raw mode on its own screen, and puts it back when
/// dropped, even after an error.
struct RawTerminal {
    /// Is true after nvim, less or rg had the terminal, until the screen
    /// drops the keys typed meanwhile.
    was_left: bool,
}

impl RawTerminal {
    fn start() -> io::Result<Self> {
        let raw_terminal = RawTerminal { was_left: false };
        raw_terminal.enter()?;
        Ok(raw_terminal)
    }

    /// Turns raw mode, the alternate screen and mouse reports on.
    fn enter(&self) -> io::Result<()> {
        let mut out = io::stdout();
        terminal::enable_raw_mode()?;
        execute!(out, EnterAlternateScreen, Hide)?;
        out.write_all(MOUSE_ON.as_bytes())?;
        out.flush()
    }

    /// Hands the terminal to `run`, such as nvim, then takes it back.
    fn run_in_place<T>(&mut self, run: impl FnOnce() -> T) -> io::Result<T> {
        restore_terminal();
        let ran = run();
        self.was_left = true;
        self.enter()?;
        Ok(ran)
    }
}

impl Drop for RawTerminal {
    fn drop(&mut self) {
        restore_terminal();
    }
}

/// Turns off mouse reports, the alternate screen and raw mode.
fn restore_terminal() {
    let mut out = io::stdout();
    // Nothing could show these errors while the terminal is put back.
    let _ = out.write_all(MOUSE_OFF.as_bytes());
    let _ = execute!(out, Show, LeaveAlternateScreen);
    let _ = terminal::disable_raw_mode();
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::review_screen::result::ResultStatus;
    use crate::review_screen::test_repo::{MadeUpRepo, STORE_SOURCE, store_request};
    use crate::steps::write_user_feedback::SIGNAL_TESTS;

    /// `find_result_before_review` must run step 7 for a review that waits
    /// with every request answered, after an empty list or a failed write.
    /// Otherwise such a review stays on Ctrl+b e's list with no way to finish.
    #[test]
    fn find_result_before_review_writes_answered_review() {
        let _one_signal_test = SIGNAL_TESTS.lock().unwrap();
        let cases = [
            ("empty list", vec![], json!({}), None),
            (
                "after failed write",
                vec![store_request(1, "Save", json!("cut"))],
                json!({"1": {"answer": "accept"}}),
                Some(json!({"code": "write_failed", "detail": "A file was in the way."})),
            ),
        ];

        for (case, requests, feedback, write_error) in cases {
            let repo = MadeUpRepo::new("feat/screen", &[("store.go", STORE_SOURCE)]);
            repo.save_review("waiting_for_user_feedback", requests, feedback);
            if let Some(write_error) = write_error {
                let mut review = repo.read_review();
                review["write_error"] = write_error;
                let review_file = repo.path().join(".git/pde/review.json");
                std::fs::write(review_file, review.to_string()).unwrap();
            }
            let review_session = review_session::load(repo.path()).unwrap().unwrap();

            let result = find_result_before_review(repo.path(), &review_session);

            assert_eq!(
                result.map(|result| result.status),
                Some(ResultStatus::Done),
                "{case}"
            );
        }
    }
}
