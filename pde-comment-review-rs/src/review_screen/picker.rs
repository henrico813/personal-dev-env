//! This module holds the Review Session Picker, the list for choosing a Review
//! Session when more than one waits. Read the `find_waiting_reviews(worktrees)`
//! and `ReviewSessionPicker::build_screen()` functions to understand the
//! module.

use std::path::PathBuf;

use super::draw::{Row, Style, fit_label};
use super::fit::{clip, escape_control_characters};
use super::keys::Key;
use crate::shared::review_session::{self, ReviewOrigin, ReviewStatus};
use crate::shared::user_feedback::UserFeedback;
use crate::shared::{Error, ErrorCode};

/// Holds one worktree on the waiting list whose Review Session needs the user.
pub(super) struct WaitingReview {
    pub(super) root: PathBuf,
    pub(super) waiting: Waiting,
}

/// Says what the picker knows about a waiting worktree.
pub(super) enum Waiting {
    Review {
        origin: ReviewOrigin,
        answered: usize,
        request_count: usize,
    },
    /// Says the last write did not finish, which the screen explains when the
    /// user picks it.
    WriteNotFinished { origin: ReviewOrigin },
    /// Holds why its saved Review Session could not be read, which the screen
    /// shows when the user picks it.
    Unreadable(Error),
}

/// Returns each waiting worktree's Review Session that waits for User Feedback
/// or whose last write did not finish, newest first. Skips a worktree with no
/// Review Session or one that waits for nothing from the user.
pub(super) fn find_waiting_reviews(worktrees: Vec<PathBuf>) -> Vec<WaitingReview> {
    let mut waiting = Vec::new();
    // The waiting list keeps the order reviews were sent, and the newest shows first.
    for root in worktrees.into_iter().rev() {
        let loaded = match review_session::load(&root) {
            Ok(Some(review_session)) => review_session,
            Ok(None) => continue,
            // A folder that is no longer a worktree holds no Review Session.
            Err(error) if error.code == ErrorCode::NotAGitCheckout => continue,
            Err(error) => {
                waiting.push(WaitingReview {
                    root,
                    waiting: Waiting::Unreadable(error),
                });
                continue;
            }
        };
        let review = match loaded.status {
            ReviewStatus::WaitingForUserFeedback => {
                let requests = loaded.newest_requests();
                let answered = requests
                    .iter()
                    .filter(|request| {
                        matches!(
                            loaded.feedback.get(&request.id),
                            Some(UserFeedback::Answer(_))
                        )
                    })
                    .count();
                let request_count = requests.len();
                Waiting::Review {
                    origin: loaded.origin,
                    answered,
                    request_count,
                }
            }
            ReviewStatus::Done | ReviewStatus::UserNotesWaiting => continue,
            // Only the user can check what a write that did not finish left behind.
            ReviewStatus::WritingUserFeedback => Waiting::WriteNotFinished {
                origin: loaded.origin,
            },
        };
        waiting.push(WaitingReview {
            root,
            waiting: review,
        });
    }
    waiting
}

/// What the picker does after a key.
#[derive(Debug, PartialEq)]
pub(super) enum AfterPick {
    Stay,
    Quit,
    Open,
}

/// Lists the waiting Review Sessions grouped by the agent session that
/// sent them.
pub(super) struct ReviewSessionPicker {
    /// Holds each group's agent session, empty for none, and its reviews, in
    /// the order shown.
    groups: Vec<(String, Vec<WaitingReview>)>,
    index: usize,
    pub(super) cols: usize,
    pub(super) rows: usize,
}

impl ReviewSessionPicker {
    /// Groups the reviews, given newest first, by agent session. A group sits
    /// where its newest review would, and reviews with no session come last.
    pub(super) fn new(reviews: Vec<WaitingReview>, (cols, rows): (usize, usize)) -> Self {
        let mut groups: Vec<(String, Vec<WaitingReview>)> = Vec::new();
        let mut unknown = Vec::new();
        for review in reviews {
            let session = match &review.waiting {
                Waiting::Review { origin, .. } | Waiting::WriteNotFinished { origin }
                    if !origin.agent_session.is_empty() =>
                {
                    origin.agent_session.clone()
                }
                _ => {
                    unknown.push(review);
                    continue;
                }
            };
            match groups.iter_mut().find(|(group, _)| *group == session) {
                Some((_, group)) => group.push(review),
                None => groups.push((session, vec![review])),
            }
        }
        if !unknown.is_empty() {
            groups.push((String::new(), unknown));
        }
        Self {
            groups,
            index: 0,
            cols,
            rows,
        }
    }

    /// Returns the reviews in the order shown.
    fn reviews(&self) -> impl Iterator<Item = &WaitingReview> {
        self.groups.iter().flat_map(|(_, reviews)| reviews)
    }

    pub(super) fn into_chosen(self) -> WaitingReview {
        self.groups
            .into_iter()
            .flat_map(|(_, reviews)| reviews)
            .nth(self.index)
            .expect("the picker keeps its index inside its reviews")
    }

    /// Acts on one key. `j` and `k` move, Enter opens, `q` or Ctrl-C closes.
    pub(super) fn press_key(&mut self, key: Key) -> AfterPick {
        let last = self.reviews().count().saturating_sub(1);
        match key {
            Key::Interrupt | Key::Char('q') => return AfterPick::Quit,
            Key::Enter => return AfterPick::Open,
            Key::Char('j') | Key::ScrollDown => self.index = (self.index + 1).min(last),
            Key::Char('k') | Key::ScrollUp => self.index = self.index.saturating_sub(1),
            _ => {}
        }
        AfterPick::Stay
    }

    /// Returns one row per terminal row. Each review takes two rows, its label
    /// and how many answers it has, and the list scrolls to keep both rows of
    /// the chosen review in view.
    pub(super) fn build_screen(&self) -> Vec<Row> {
        let mut list = Vec::new();
        let mut chosen_row = 0;
        let mut position = 0;
        for (session, reviews) in &self.groups {
            let title = if session.is_empty() {
                "Unknown session".to_owned()
            } else {
                format!("Session {}", escape_control_characters(clip(session, 8)))
            };
            list.push(Row::new(Style::Title, clip(&title, self.cols).to_owned()));
            for review in reviews {
                let marker = if position == self.index {
                    chosen_row = list.len();
                    "> "
                } else {
                    "  "
                };
                let width = self.cols.saturating_sub(marker.len());
                let folder = review
                    .root
                    .file_name()
                    .map(|name| escape_control_characters(&name.to_string_lossy()))
                    .unwrap_or_default();
                let (label, details) = match &review.waiting {
                    Waiting::Review {
                        origin,
                        answered,
                        request_count,
                    } => (
                        fit_label(origin, width),
                        format!("    {folder} · {answered}/{request_count} answered"),
                    ),
                    Waiting::WriteNotFinished { origin } => (
                        fit_label(origin, width),
                        format!("    {folder} · write did not finish"),
                    ),
                    Waiting::Unreadable(_) => (
                        clip(&folder, width).to_owned(),
                        "    can't be read".to_owned(),
                    ),
                };
                list.push(Row::new(Style::Plain, format!("{marker}{label}")));
                list.push(Row::new(Style::Faded, clip(&details, self.cols).to_owned()));
                position += 1;
            }
        }
        // One row for the title above the list and one for the Key Help below it.
        let room = self.rows.saturating_sub(2).max(2);
        let first = (chosen_row + 2).saturating_sub(room);
        let mut screen = vec![Row::new(
            Style::Title,
            clip("Comment reviews waiting", self.cols).to_owned(),
        )];
        screen.extend(list.into_iter().skip(first).take(room));
        while screen.len() < self.rows.saturating_sub(1) {
            screen.push(Row::new(Style::Plain, String::new()));
        }
        screen.push(Row::new(
            Style::Faded,
            clip("j k move Enter open q quit", self.cols).to_owned(),
        ));
        screen
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use serde_json::json;

    use super::*;
    use crate::review_screen::test_repo::MadeUpRepo;

    /// Returns the text of each row, without its colors.
    fn row_texts(rows: &[Row]) -> Vec<String> {
        rows.iter()
            .map(|row| row.0.iter().map(|(_, text)| text.as_str()).collect())
            .collect()
    }

    /// Returns a waiting review in folder `name` on branch `name`, from
    /// `agent_session`.
    fn make_waiting(name: &str, agent_session: &str, answered: usize) -> WaitingReview {
        WaitingReview {
            root: PathBuf::from("/worktrees").join(name),
            waiting: Waiting::Review {
                origin: ReviewOrigin {
                    repo: "pde".to_owned(),
                    branch: format!("feat/{name}"),
                    agent_session: agent_session.to_owned(),
                },
                answered,
                request_count: 2,
            },
        }
    }

    /// `find_waiting_reviews` must keep each review that waits for User
    /// Feedback, whose last write did not finish, or that can't be read, say
    /// which, and give them newest first, the order the picker shows.
    /// Otherwise Ctrl+b e opens a review that is done or waits for the agent,
    /// or hides one only the user can answer or check.
    #[test]
    fn find_waiting_reviews_keeps_reviews_needing_user() {
        let no_review = MadeUpRepo::new("feat/none", &[]);
        let done = MadeUpRepo::new("feat/done", &[]);
        done.save_review("done", vec![], json!({}));
        let notes_wait = MadeUpRepo::new("feat/notes", &[]);
        notes_wait.save_review("user_notes_waiting", vec![], json!({}));
        let empty_list = MadeUpRepo::new("feat/empty", &[]);
        empty_list.save_review("waiting_for_user_feedback", vec![], json!({}));
        let writing = MadeUpRepo::new("feat/writing", &[]);
        writing.save_review("writing_user_feedback", vec![], json!({}));
        let unreadable = MadeUpRepo::new("feat/unreadable", &[]);
        unreadable.save_review("waiting_for_user_feedback", vec![], json!({}));
        fs::write(unreadable.path().join(".git/pde/review.json"), "{").unwrap();
        let worktrees = [
            &no_review,
            &done,
            &notes_wait,
            &empty_list,
            &writing,
            &unreadable,
        ]
        .map(|repo| repo.path().to_owned())
        .to_vec();

        let waiting = find_waiting_reviews(worktrees);

        let found: Vec<_> = waiting
            .iter()
            .map(|review| {
                let kind = match review.waiting {
                    Waiting::Review { .. } => "waits",
                    Waiting::WriteNotFinished { .. } => "write did not finish",
                    Waiting::Unreadable(_) => "can't be read",
                };
                (review.root.as_path(), kind)
            })
            .collect();
        assert_eq!(
            found,
            [
                (unreadable.path(), "can't be read"),
                (writing.path(), "write did not finish"),
                (empty_list.path(), "waits"),
            ]
        );
    }

    /// `ReviewSessionPicker::build_screen` must group the reviews by agent
    /// session and say how many answers each has. Otherwise the user cannot
    /// tell which agent's review to open.
    #[test]
    fn build_screen_groups_reviews_by_agent_session() {
        let reviews = vec![
            make_waiting("one", "ses_aaaa", 1),
            make_waiting("two", "ses_bbbb", 0),
            make_waiting("three", "ses_aaaa", 2),
        ];

        let picker = ReviewSessionPicker::new(reviews, (40, 16));
        let rows = row_texts(&picker.build_screen());

        assert_eq!(
            rows[..9],
            [
                "Comment reviews waiting",
                "Session ses_aaaa",
                "> pde · one",
                "    one · 1/2 answered",
                "  pde · three",
                "    three · 2/2 answered",
                "Session ses_bbbb",
                "  pde · two",
                "    two · 0/2 answered",
            ]
        );
    }
}
