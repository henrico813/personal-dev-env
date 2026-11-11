//! This module starts the screen for the `screen` and `open` commands. Herdr's
//! Ctrl+b e and Ctrl+b E run `screen --popup` and `screen --tab` twice. The
//! first run is the key's action, which opens the Review Popup or Review Tab,
//! and the second runs inside it and shows the Review Session. Read the
//! `run_screen_command(kind)` and `run_open_command(branch)` functions to
//! understand the module.

use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use super::herdr::{
    Herdr, build_record_name, find_popup_record_path, open_from_herdr, read_workspace_id,
    save_popup_record,
};
use super::picker::{Waiting, WaitingReview, find_waiting_reviews};
use super::result::ReviewResult;
use super::{RawTerminal, StartFailure, pick_review, show_picked_review, show_review_screen};
use crate::commands::cli::ScreenKind;
use crate::shared::git;
use crate::shared::review_session::read_waiting_list;

/// Shows when `screen` runs anywhere but inside Herdr's Ctrl+b e or Ctrl+b E.
const SCREEN_NEEDS_HERDR: &str =
    "Only Herdr's Ctrl+b e and Ctrl+b E run screen. In a terminal, run open.";

/// Runs `screen --popup` for Ctrl+b e or `screen --tab` for Ctrl+b E. As the
/// key's action it opens the Review Popup or Review Tab, and inside them it
/// shows the only waiting review or the one picked in the Review Session
/// Picker.
///
/// # Errors
///
/// Fails outside Herdr, when Herdr does not open the popup or tab, or when the
/// terminal cannot be read or drawn on.
pub fn run_screen_command(kind: ScreenKind) -> Result<(), StartFailure> {
    let Some(herdr) = Herdr::from_env() else {
        return Err(StartFailure(SCREEN_NEEDS_HERDR.to_owned()));
    };
    if env::var_os("HERDR_PLUGIN_ACTION_ID").is_some() {
        // Herdr keeps an action's errors out of sight, so the user gets a notification.
        let waiting = find_waiting_reviews_or_say_why().inspect_err(|StartFailure(message)| {
            let _ = herdr.show_notification(message);
        })?;
        return open_from_herdr(&herdr, kind, waiting.len())
            .map_err(|error| StartFailure(format!("Herdr did not open the screen. {error}")));
    }
    let entrypoint = match kind {
        ScreenKind::Popup => "popup",
        ScreenKind::Tab => "tab",
    };
    if env::var("HERDR_PLUGIN_ENTRYPOINT_ID").as_deref() != Ok(entrypoint) {
        return Err(StartFailure(SCREEN_NEEDS_HERDR.to_owned()));
    }
    match kind {
        ScreenKind::Popup => show_in_review_popup(),
        ScreenKind::Tab => {
            let shown = show_waiting_review(|_| {});
            if let Err(StartFailure(message)) = &shown {
                wait_for_enter(message);
            }
            // Herdr keeps a tab open after its program ends, so the Review Tab closes itself.
            if let Ok(pane_id) = env::var("HERDR_PANE_ID") {
                let _ = herdr.close_pane(&pane_id);
            }
            shown
        }
    }
}

/// Runs `open` in a Review Terminal, showing the Review Session of the
/// worktree with `branch` checked out. Without a branch, or when several
/// waiting worktrees have it, it shows the Review Session Picker first, the
/// same list as Ctrl+b e.
///
/// # Errors
///
/// Fails when no worktree has the branch, when the picker's waiting list
/// cannot be read, or when the terminal cannot be read or drawn on.
pub fn run_open_command(branch: Option<&str>) -> Result<(), StartFailure> {
    let Some(branch) = branch else {
        return show_waiting_review(|_| {});
    };
    let here = env::current_dir().map_err(|error| StartFailure(error.to_string()))?;
    // A damaged waiting list must not stop Git from finding the branch.
    let waiting = read_waiting_list().unwrap_or_default();
    let mut worktrees = find_worktrees_by_branch(branch, &waiting, &here);
    if worktrees.len() > 1 {
        // Two clones can each have the branch checked out, so the user picks the review.
        return pick_and_show_review(find_waiting_reviews(worktrees), |_| {});
    }
    let Some(worktree) = worktrees.pop() else {
        return Err(StartFailure(format!(
            "No worktree has branch {branch} checked out. Run open with no name to pick a waiting review."
        )));
    };
    show_review_screen(&worktree).map_err(terminal_failure)
}

/// Shows a waiting Review Session in the Review Popup, keeping the popup record
/// for as long as the popup is open.
fn show_in_review_popup() -> Result<(), StartFailure> {
    let record = find_popup_record_path().zip(read_workspace_id());
    if let Some((path, workspace_id)) = &record {
        // The record helps the PR approval tool, so the popup opens without it.
        let _ = save_popup_record(path, workspace_id, "Comment review");
    }
    let shown = show_waiting_review(|picked| {
        if let (
            Some((path, workspace_id)),
            Waiting::Review { origin, .. } | Waiting::WriteNotFinished { origin },
        ) = (&record, &picked.waiting)
        {
            let _ = save_popup_record(path, workspace_id, &build_record_name(origin));
        }
    });
    if let Some((path, _)) = &record {
        let _ = fs::remove_file(path);
    }
    shown
}

/// Shows the only waiting Review Session, or the one the user picks. `on_pick`
/// runs with the chosen one before it shows.
fn show_waiting_review(on_pick: impl FnOnce(&WaitingReview)) -> Result<(), StartFailure> {
    pick_and_show_review(find_waiting_reviews_or_say_why()?, on_pick)
}

/// Shows the only one of `reviews`, or the one the user picks in the Review
/// Session Picker. `on_pick` runs with the chosen review before it shows.
fn pick_and_show_review(
    reviews: Vec<WaitingReview>,
    on_pick: impl FnOnce(&WaitingReview),
) -> Result<(), StartFailure> {
    if reviews.is_empty() {
        println!("No comment reviews are waiting.");
        return Ok(());
    }
    let mut terminal = RawTerminal::start().map_err(terminal_failure)?;
    let Some(picked) = pick_review(reviews).map_err(terminal_failure)? else {
        return Ok(());
    };
    on_pick(&picked);
    show_picked_review(&mut terminal, picked).map_err(terminal_failure)
}

/// Returns the waiting reviews, or why the waiting list could not be read.
fn find_waiting_reviews_or_say_why() -> Result<Vec<WaitingReview>, StartFailure> {
    let here = env::current_dir().unwrap_or_default();
    let worktrees = read_waiting_list().map_err(|error| explain(&here, &error))?;
    Ok(find_waiting_reviews(worktrees))
}

/// Returns every worktree on the waiting list with `branch` checked out, or
/// when none is listed, those of the clone that holds `here`. The second look
/// finds a Review Session whose waiting-list change failed.
pub(crate) fn find_worktrees_by_branch(
    branch: &str,
    waiting: &[PathBuf],
    here: &Path,
) -> Vec<PathBuf> {
    let on_waiting_list: Vec<PathBuf> = waiting
        .iter()
        .filter(|worktree| {
            git::read_current_branch(worktree).is_ok_and(|current| current == branch)
        })
        .cloned()
        .collect();
    if !on_waiting_list.is_empty() {
        return on_waiting_list;
    }
    // Outside a Git checkout there is no clone to look in.
    git::find_worktrees_on_branch(here, branch).unwrap_or_default()
}

/// Returns the user's words for an error, as the Review Result Screen says it.
fn explain(root: &Path, error: &crate::shared::Error) -> StartFailure {
    StartFailure(ReviewResult::from_error(root, error).message)
}

fn terminal_failure(error: io::Error) -> StartFailure {
    StartFailure(format!("The terminal failed. {error}"))
}

/// Keeps the Review Tab open until Enter, so the user can read why it failed.
fn wait_for_enter(message: &str) {
    println!("{message}\r\nPress Enter to close.");
    let _ = io::stdin().read_line(&mut String::new());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::review_screen::test_repo::MadeUpRepo;

    /// `find_worktrees_by_branch` must find each waiting worktree with the
    /// branch from any folder, and a worktree of the clone that is not on the
    /// waiting list. Otherwise `open NAME` opens another clone's review, or
    /// never opens one saved before a failed waiting-list change.
    #[test]
    fn find_worktrees_by_branch_finds_every_checkout() {
        let here = MadeUpRepo::new("main", &[]);
        here.run_git(&[
            "worktree",
            "add",
            "--quiet",
            "-b",
            "feat/off-list",
            "off-list",
        ]);
        let off_list = here.path().join("off-list");
        let listed = MadeUpRepo::new("feat/listed", &[]);
        let first_clone = MadeUpRepo::new("feat/twice", &[]);
        let second_clone = MadeUpRepo::new("feat/twice", &[]);
        let waiting = [&listed, &first_clone, &second_clone].map(|repo| repo.path().to_owned());
        let cases = [
            ("listed elsewhere", "feat/listed", vec![listed.path()]),
            (
                "two clones",
                "feat/twice",
                vec![first_clone.path(), second_clone.path()],
            ),
            ("unlisted", "feat/off-list", vec![off_list.as_path()]),
        ];

        for (case, branch, expected) in cases {
            let found = find_worktrees_by_branch(branch, &waiting, here.path());

            assert_eq!(found, expected, "{case}");
        }
    }
}
