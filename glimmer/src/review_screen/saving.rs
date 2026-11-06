//! This module saves the User Feedback each key gives, then loads the Review
//! Session again, so the screen shows only what is saved. Once every Feedback
//! Request has an answer, it runs step 7. Read the
//! `ReviewScreen::save_given_feedback(id)` function to understand the module.

use super::result::{ReviewResult, write_and_explain};
use super::screen::{AfterKey, ReviewScreen};
use crate::shared::ErrorCode;
use crate::shared::review_session::{self, RequestId, ReviewStatus};
use crate::shared::user_feedback::UserFeedback;

/// Comes before the Review Result when another screen finished the Review
/// Session before the last key's save.
const LATE_ANSWER: &str =
    "Your last answer was not saved, because this review was finished on another screen.";

impl ReviewScreen<'_> {
    /// Saves the User Feedback the last key gave request `id` in memory, then
    /// loads the Review Session again and returns what the screen does next.
    /// After a refusal it shows request `id` as saved, since a save can fail
    /// after the User Feedback is written.
    pub(super) fn save_given_feedback(&mut self, id: RequestId) -> AfterKey {
        let shown_id = self.shown_request().id;
        let feedback = self.review_session.feedback.remove(&id);
        let requests = self.review_session.newest_requests();
        let given_request = requests
            .into_iter()
            .find(|request| request.id == id)
            .expect("press_key gives User Feedback only to a request on the screen");
        let saved = match feedback {
            Some(feedback) => {
                review_session::save_user_feedback(self.root, given_request, feedback)
            }
            None => review_session::remove_user_note(self.root, given_request),
        };
        match review_session::load(self.root) {
            Ok(Some(loaded)) => *self.review_session = loaded,
            Ok(None) => {
                let message = "The review is gone, so nothing was saved. Ask the agent to send the list again.";
                return AfterKey::ShowResult(ReviewResult::not_written(message));
            }
            Err(error) => return AfterKey::ShowResult(ReviewResult::from_error(self.root, &error)),
        }
        let status = self.review_session.status;
        match saved {
            // The screen closes once every request has an answer or a note, as the agent acts next.
            Ok(ReviewStatus::UserNotesWaiting) if status == ReviewStatus::UserNotesWaiting => {
                AfterKey::Quit
            }
            Ok(_) => self.show_after_save(shown_id),
            Err(error) => {
                if let Some(mut result) = ReviewResult::from_status(self.root, status) {
                    // The status words alone would read as if this key's answer counted.
                    if error.code == ErrorCode::LateUserFeedback {
                        result.message = format!("{LATE_ANSWER} {}", result.message);
                    }
                    return AfterKey::ShowResult(result);
                }
                let index = self.find_index_or_first_unanswered(id);
                // The request may have changed, so the user reads it again before answering.
                self.show(index);
                self.notice = ReviewResult::from_error(self.root, &error).message;
                AfterKey::Stay
            }
        }
    }

    /// Keeps showing request `shown_id` while any request has no User
    /// Feedback, or runs step 7 once every request has an answer.
    fn show_after_save(&mut self, shown_id: RequestId) -> AfterKey {
        if let Some(result) = ReviewResult::from_status(self.root, self.review_session.status) {
            return AfterKey::ShowResult(result);
        }
        let requests = self.requests();
        let feedback = &self.review_session.feedback;
        if requests
            .iter()
            .all(|request| matches!(feedback.get(&request.id), Some(UserFeedback::Answer(_))))
        {
            return AfterKey::ShowResult(write_and_explain(self.root));
        }
        let index = self.find_index_or_first_unanswered(shown_id);
        if index != self.index {
            self.show(index);
        }
        AfterKey::Stay
    }

    /// Returns where request `id` is in the loaded Review Session, or where
    /// the first request without User Feedback is when `id` is gone.
    fn find_index_or_first_unanswered(&self, id: RequestId) -> usize {
        let requests = self.requests();
        requests
            .iter()
            .position(|request| request.id == id)
            .or_else(|| {
                requests
                    .iter()
                    .position(|request| !self.review_session.feedback.contains_key(&request.id))
            })
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use std::fs::{self, Permissions};
    use std::os::unix::fs::PermissionsExt;
    use std::time::{Duration, Instant};

    use serde_json::json;

    use super::*;
    use crate::review_screen::keys::Key;
    use crate::review_screen::result::ResultStatus;
    use crate::review_screen::test_repo::{MadeUpRepo, STORE_SOURCE, store_request};
    use crate::shared::review_session::ReviewSession;
    use crate::steps::write_user_feedback::SIGNAL_TESTS;

    /// The 40x16 phone popup's columns and rows.
    const PHONE: (usize, usize) = (40, 16);

    /// Makes a repository whose Review Session asks about two comments, each
    /// with an Agent Suggestion to keep it.
    fn make_two_request_repo(feedback: serde_json::Value) -> MadeUpRepo {
        let repo = MadeUpRepo::new("feat/screen", &[("store.go", STORE_SOURCE)]);
        let requests = vec![
            store_request(1, "Save", json!("keep")),
            store_request(2, "Load", json!("keep")),
        ];
        repo.save_review("waiting_for_user_feedback", requests, feedback);
        repo
    }

    /// Loads the repository's saved Review Session.
    fn load_review(repo: &MadeUpRepo) -> ReviewSession {
        review_session::load(repo.path()).unwrap().unwrap()
    }

    /// Opens the screen with the answer keys' wait over, and returns the
    /// moment after it.
    fn open_screen<'a>(
        repo: &'a MadeUpRepo,
        review_session: &'a mut ReviewSession,
    ) -> (ReviewScreen<'a>, Instant) {
        let start = Instant::now();
        let mut screen = ReviewScreen::new(repo.path(), review_session, PHONE)
            .expect("a request waits for User Feedback");
        screen.start_wait(start);
        (screen, start + Duration::from_secs(1))
    }

    /// `save_given_feedback` must save the answer in `review.json` as soon as
    /// its key is pressed. Otherwise closing the screen loses every answer
    /// given so far.
    #[test]
    fn save_given_feedback_keeps_answer_on_disk() {
        let repo = make_two_request_repo(json!({}));
        let mut review_session = load_review(&repo);
        let (mut screen, later) = open_screen(&repo, &mut review_session);

        let after_key = screen.press_key(Key::Char('o'), later);
        let after_save = screen.save_given_feedback(RequestId(1));

        assert_eq!(after_key, AfterKey::Save(RequestId(1)));
        assert_eq!(after_save, AfterKey::Stay);
        assert_eq!(
            repo.read_review()["feedback"]["1"],
            json!({"answer": "keep"})
        );
    }

    /// `save_given_feedback` must run step 7 after the last answer and say the
    /// answers are in the code. Otherwise the review waits with every answer
    /// given and nothing written.
    #[test]
    fn save_given_feedback_writes_after_last_answer() {
        let _one_signal_test = SIGNAL_TESTS.lock().unwrap();
        let repo = MadeUpRepo::new("feat/screen", &[("store.go", STORE_SOURCE)]);
        repo.save_review(
            "waiting_for_user_feedback",
            vec![store_request(1, "Save", json!("cut"))],
            json!({}),
        );
        let mut review_session = load_review(&repo);
        let (mut screen, later) = open_screen(&repo, &mut review_session);

        screen.press_key(Key::Char('a'), later);
        let after_save = screen.save_given_feedback(RequestId(1));

        let AfterKey::ShowResult(result) = after_save else {
            panic!("the last answer shows the Review Result, not {after_save:?}");
        };
        assert_eq!(result.status, ResultStatus::Done);
        let store = fs::read_to_string(repo.path().join("store.go")).unwrap();
        assert!(!store.contains("// Saves it."), "{store}");
    }

    /// `save_given_feedback` must show the request it could not save, with
    /// what was saved and what to do. Otherwise the screen moves on as if the
    /// answer were saved, and the user never answers that request again.
    #[test]
    fn save_given_feedback_shows_request_after_failure() {
        let repo = make_two_request_repo(json!({}));
        let mut review_session = load_review(&repo);
        let (mut screen, later) = open_screen(&repo, &mut review_session);
        // A folder the program cannot write makes the save fail.
        let review_folder = repo.path().join(".git/pde");
        fs::set_permissions(&review_folder, Permissions::from_mode(0o555)).unwrap();

        screen.press_key(Key::Char('o'), later);
        let after_save = screen.save_given_feedback(RequestId(1));

        fs::set_permissions(&review_folder, Permissions::from_mode(0o755)).unwrap();
        assert_eq!(after_save, AfterKey::Stay);
        assert_eq!(screen.shown_request().id, RequestId(1));
        assert!(screen.review_session.feedback.is_empty());
        assert!(
            screen.notice.starts_with("The review could not be saved."),
            "{}",
            screen.notice
        );
    }

    /// `save_given_feedback` must show the newest version of a request that
    /// changed after the screen showed it, and save nothing. Otherwise the
    /// user's answer counts for text they never read.
    #[test]
    fn save_given_feedback_shows_changed_request_again() {
        let repo = make_two_request_repo(json!({}));
        let mut review_session = load_review(&repo);
        let (mut screen, later) = open_screen(&repo, &mut review_session);
        // Another program saves a new version of request 1 after the screen read it.
        let mut changed = store_request(1, "Save", json!("keep"));
        changed["suggestion"]["reasoning"] = json!("Says it twice.");
        let requests = vec![changed, store_request(2, "Load", json!("keep"))];
        repo.save_review("waiting_for_user_feedback", requests, json!({}));

        screen.press_key(Key::Char('o'), later);
        screen.save_given_feedback(RequestId(1));

        let shown = screen.shown_request();
        assert_eq!(shown.id, RequestId(1));
        assert_eq!(
            shown.suggestion.as_ref().unwrap().reasoning,
            "Says it twice."
        );
        assert_eq!(repo.read_review()["feedback"], json!({}));
    }

    /// `save_given_feedback` must close the screen once the last request gets
    /// a User Note. Otherwise the screen stays open on a review that now waits
    /// for the agent.
    #[test]
    fn save_given_feedback_closes_after_last_note() {
        let repo = MadeUpRepo::new("feat/screen", &[("store.go", STORE_SOURCE)]);
        repo.save_review(
            "waiting_for_user_feedback",
            vec![store_request(1, "Save", json!("keep"))],
            json!({}),
        );
        let mut review_session = load_review(&repo);
        let (mut screen, later) = open_screen(&repo, &mut review_session);

        for key in [Key::Char('m'), Key::Char('x'), Key::Enter] {
            screen.press_key(key, later);
        }
        let after_save = screen.save_given_feedback(RequestId(1));

        assert_eq!(after_save, AfterKey::Quit);
    }

    /// `save_given_feedback` must remove a saved User Note once the user
    /// empties it. Otherwise the request goes back to the agent with a note the
    /// user took back.
    #[test]
    fn save_given_feedback_removes_emptied_user_note() {
        let repo = make_two_request_repo(json!({"1": {"note": "Say why"}}));
        let mut review_session = load_review(&repo);
        let (mut screen, later) = open_screen(&repo, &mut review_session);
        // The screen opens on request 2, the first without User Feedback.
        screen.press_key(Key::Char('h'), later);
        screen.start_wait(later);
        let much_later = later + Duration::from_secs(1);

        screen.press_key(Key::Char('m'), much_later);
        for _ in "Say why".chars() {
            screen.press_key(Key::Backspace, much_later);
        }
        screen.press_key(Key::Enter, much_later);
        screen.save_given_feedback(RequestId(1));

        assert_eq!(repo.read_review()["feedback"], json!({}));
    }

    /// `save_given_feedback` must say an answer was not saved when another
    /// screen finished the review first. Otherwise the user reads the review's
    /// result as if their last answer counted.
    #[test]
    fn save_given_feedback_says_late_answer_unsaved() {
        let repo = make_two_request_repo(json!({}));
        let mut review_session = load_review(&repo);
        let (mut screen, later) = open_screen(&repo, &mut review_session);
        // Another screen finishes the review after this one opened.
        let requests = vec![
            store_request(1, "Save", json!("keep")),
            store_request(2, "Load", json!("keep")),
        ];
        repo.save_review("done", requests, json!({}));

        screen.press_key(Key::Char('o'), later);
        let after_save = screen.save_given_feedback(RequestId(1));

        let AfterKey::ShowResult(result) = after_save else {
            panic!("a finished review shows the Review Result, not {after_save:?}");
        };
        assert!(
            result.message.starts_with("Your last answer was not saved"),
            "{}",
            result.message
        );
    }

    /// `save_given_feedback` must say a failed write put every file back,
    /// that opening the review again tries again, and why it failed.
    /// Otherwise the user thinks the answers are in the code or lost, or
    /// retries a write that fails the same way each time.
    #[test]
    fn save_given_feedback_explains_failed_write() {
        let _one_signal_test = SIGNAL_TESTS.lock().unwrap();
        let repo = MadeUpRepo::new("feat/screen", &[("store.go", STORE_SOURCE)]);
        repo.save_review(
            "waiting_for_user_feedback",
            vec![store_request(1, "Save", json!("cut"))],
            json!({}),
        );
        // A file left in the way stops the write, and every file goes back.
        fs::write(repo.path().join("store.go.comment-review.tmp"), "").unwrap();
        let mut review_session = load_review(&repo);
        let (mut screen, later) = open_screen(&repo, &mut review_session);

        screen.press_key(Key::Char('a'), later);
        let after_save = screen.save_given_feedback(RequestId(1));

        let AfterKey::ShowResult(result) = after_save else {
            panic!("a failed write shows the Review Result, not {after_save:?}");
        };
        let (words, why) = result
            .message
            .split_once('\n')
            .expect("why the write failed follows on its own rows");
        assert_eq!(
            words,
            "The last write failed, and every file was put back. Open the review again to try again."
        );
        assert!(
            why.ends_with("store.go.comment-review.tmp is in the way."),
            "{why}"
        );
    }
}
