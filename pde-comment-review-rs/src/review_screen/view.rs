//! Holds what the screen shows and answers each key, with the guards that
//! stop a stray tap from giving User Feedback.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};

use super::code_around::{CodeAround, find_code_around};
use super::keys::Key;
use crate::shared::comments::{CommentEdit, rewrite};
use crate::shared::review_session::{FeedbackRequest, RequestId, ReviewOrigin, ReviewSession};
use crate::shared::user_feedback::{UserAnswer, UserFeedback, UserNote, choose_new_text};

/// How long `a`, `o`, `c` and `m` wait after a request shows, so a double tap
/// cannot answer two requests.
const SETTLE: Duration = Duration::from_millis(600);

pub(super) const HEADER_ROWS: usize = 2;
pub(super) const KEY_HELP_ROWS: usize = 2;

/// Shows for every refused answer. The reasons rewrite gives are written for
/// agents, so the screen leaves them out.
const REFUSED: &str = "That change can't be made to this comment.";

/// What the screen does after a key.
#[derive(Debug, PartialEq)]
pub(super) enum AfterKey {
    Stay,
    Quit,
}

/// Shows one Feedback Request at a time and keeps each answer in the Review
/// Session it borrows.
pub(super) struct View<'a> {
    repo: &'a Path,
    pub(super) origin: &'a ReviewOrigin,
    pub(super) requests: Vec<&'a FeedbackRequest>,
    pub(super) feedback: &'a mut BTreeMap<RequestId, UserFeedback>,
    pub(super) index: usize,
    /// The shown request's comment and code, or why they cannot be shown.
    pub(super) code: Result<CodeAround, String>,
    pub(super) scroll: usize,
    pub(super) cols: usize,
    pub(super) rows: usize,
    /// Says why a key did nothing, until the next key.
    pub(super) notice: String,
    /// The note the user is typing, while the note prompt is open.
    pub(super) typed_note: Option<String>,
    /// When the shown request was first drawn, or `None` until it is.
    drawn_at: Option<Instant>,
}

impl<'a> View<'a> {
    /// Returns a view of the newest version of each request, showing the first
    /// one with no User Feedback, or `None` when every request has some.
    pub(super) fn new(
        repo: &'a Path,
        review_session: &'a mut ReviewSession,
        (cols, rows): (usize, usize),
    ) -> Option<Self> {
        let ReviewSession {
            origin,
            lists,
            feedback,
            ..
        } = review_session;
        // A request sent again in a later list replaces its older version.
        let mut newest = BTreeMap::new();
        for request in lists.iter().flat_map(|list| &list.requests) {
            newest.insert(request.id, request);
        }
        let requests: Vec<&FeedbackRequest> = newest.into_values().collect();
        let first = requests
            .iter()
            .position(|request| !feedback.contains_key(&request.id))?;
        let mut view = View {
            repo,
            origin,
            requests,
            feedback,
            index: first,
            code: Err(String::new()),
            scroll: 0,
            cols,
            rows,
            notice: String::new(),
            typed_note: None,
            drawn_at: None,
        };
        view.show(first);
        Some(view)
    }

    /// Sets the terminal's size and keeps the scroll within the new body.
    pub(super) fn set_size(&mut self, cols: usize, rows: usize) {
        self.cols = cols;
        self.rows = rows;
        self.scroll = self.scroll.min(self.find_max_scroll());
    }

    /// Returns how many body rows fit between the header and the key help.
    pub(super) fn body_room(&self) -> usize {
        self.rows.saturating_sub(HEADER_ROWS + KEY_HELP_ROWS).max(1)
    }

    fn find_max_scroll(&self) -> usize {
        self.build_body().0.len().saturating_sub(self.body_room())
    }

    /// Returns true when the shown request has not been drawn yet.
    pub(super) fn is_waiting_for_draw(&self) -> bool {
        self.drawn_at.is_none()
    }

    /// Starts the answer keys' wait at the moment the shown request was
    /// drawn, so a slow answer check cannot shorten it.
    pub(super) fn start_wait(&mut self, drawn_at: Instant) {
        self.drawn_at = Some(drawn_at);
    }

    /// Shows request `index` scrolled to the agent's notes, past the code
    /// above. The answer keys wait until it is drawn.
    fn show(&mut self, index: usize) {
        self.index = index;
        self.typed_note = None;
        let request = self.requests[index];
        self.code = fs::read_to_string(self.repo.join(&request.file))
            .map_err(|error| format!("{}: {error}", request.file.display()))
            .and_then(|source| find_code_around(request, &source));
        self.scroll = self.build_body().1.min(self.find_max_scroll());
        self.drawn_at = None;
    }

    /// Acts on one key. `l` and `h` wrap around at the last and first request.
    pub(super) fn press_key(&mut self, key: Key, now: Instant) -> AfterKey {
        self.notice.clear();
        if key == Key::Interrupt {
            return AfterKey::Quit;
        }
        if self.typed_note.is_some() {
            return self.type_note(key);
        }
        let count = self.requests.len();
        match key {
            Key::Char('j') | Key::ScrollDown => {
                self.scroll = (self.scroll + 1).min(self.find_max_scroll());
            }
            Key::Char('k') | Key::ScrollUp => self.scroll = self.scroll.saturating_sub(1),
            Key::Char('q') => return AfterKey::Quit,
            Key::Char('l') => self.show((self.index + 1) % count),
            Key::Char('h') => self.show((self.index + count - 1) % count),
            Key::Char('a' | 'o' | 'c' | 'm') if !self.has_waited(now) => {
                self.notice = "One moment, then press again.".to_owned();
            }
            Key::Char('m') => self.typed_note = Some(self.find_saved_note()),
            Key::Char('a') => return self.answer(UserAnswer::Accept),
            Key::Char('o') => return self.answer(UserAnswer::Keep),
            Key::Char('c') => return self.answer(UserAnswer::Cut),
            _ => {}
        }
        AfterKey::Stay
    }

    fn has_waited(&self, now: Instant) -> bool {
        self.drawn_at
            .is_some_and(|drawn_at| now.duration_since(drawn_at) >= SETTLE)
    }

    /// Types into the note. A note starts as the saved one, so Enter alone
    /// changes nothing, and an empty note removes it.
    fn type_note(&mut self, key: Key) -> AfterKey {
        let Some(typed) = &mut self.typed_note else {
            return AfterKey::Stay;
        };
        match key {
            Key::Char(character) => typed.push(character),
            Key::Backspace => {
                typed.pop();
            }
            Key::Enter => {
                let note = typed.trim().to_owned();
                self.typed_note = None;
                return self.save_note(note);
            }
            _ => {}
        }
        AfterKey::Stay
    }

    fn find_saved_note(&self) -> String {
        match self.feedback.get(&self.requests[self.index].id) {
            Some(UserFeedback::Note(UserNote(note))) => note.clone(),
            _ => String::new(),
        }
    }

    /// Saves the note and stays on its request, or closes the screen once
    /// every request has User Feedback.
    fn save_note(&mut self, note: String) -> AfterKey {
        let id = self.requests[self.index].id;
        if !note.is_empty() {
            self.feedback.insert(id, UserFeedback::Note(UserNote(note)));
        } else if matches!(self.feedback.get(&id), Some(UserFeedback::Note(_))) {
            self.feedback.remove(&id);
        }
        match self.find_next_without_feedback() {
            Some(_) => AfterKey::Stay,
            None => AfterKey::Quit,
        }
    }

    /// Keeps the answer and shows the next request without User Feedback, or
    /// says why the answer was refused and keeps nothing.
    fn answer(&mut self, answer: UserAnswer) -> AfterKey {
        if !self.can_write_answer(&answer) {
            self.notice = REFUSED.to_owned();
            return AfterKey::Stay;
        }
        let id = self.requests[self.index].id;
        self.feedback.insert(id, UserFeedback::Answer(answer));
        match self.find_next_without_feedback() {
            Some(next) => {
                self.show(next);
                AfterKey::Stay
            }
            None => AfterKey::Quit,
        }
    }

    /// Rewrites the comment's file in memory, so a cut or reword the code
    /// cannot take is refused now rather than after the last answer.
    fn can_write_answer(&self, answer: &UserAnswer) -> bool {
        let request = self.requests[self.index];
        let new_text = choose_new_text(request, answer);
        if new_text.as_deref() == Some(request.text.as_str()) {
            return true;
        }
        // rewrite needs the comment as found in its file.
        let Ok(code) = &self.code else {
            return false;
        };
        let edit = CommentEdit {
            comment: code.comment.clone(),
            new_text,
        };
        rewrite(self.repo, &[edit]).is_ok()
    }

    /// Returns the next request after the shown one, wrapping around, that
    /// has no User Feedback.
    fn find_next_without_feedback(&self) -> Option<usize> {
        let count = self.requests.len();
        (1..=count)
            .map(|step| (self.index + step) % count)
            .find(|&index| !self.feedback.contains_key(&self.requests[index].id))
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;

    const PHONE: (usize, usize) = (40, 16);

    fn samples() -> &'static Path {
        Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/samples/screen"))
    }

    /// Returns a Review Session with one request for each `(file, text, line,
    /// suggestion)`.
    fn make_review_session(requests: &[(&str, &str, usize, Value)]) -> ReviewSession {
        let requests: Vec<Value> = requests
            .iter()
            .enumerate()
            .map(|(index, (file, text, line, kind))| {
                json!({
                    "id": index + 1,
                    "file": file,
                    "name": null,
                    "reason": "new",
                    "text": text,
                    "can_reword": true,
                    "line": line,
                    "suggestion": {"kind": kind, "reasoning": "", "also_at": []},
                })
            })
            .collect();
        serde_json::from_value(json!({
            "status": "waiting_for_user_feedback",
            "origin": {"repo": "repo", "branch": "feat/screen", "agent_session": "s"},
            "lists": [{"base": "4f2a9c1e", "requests": requests}],
            "feedback": {},
        }))
        .expect("the Review Session matches its saved shape")
    }

    fn after(start: Instant, millis: u64) -> Instant {
        start + Duration::from_millis(millis)
    }

    /// Without it, a second tap read after a slow answer check answers the
    /// next request before the user has seen it.
    #[test]
    fn answer_keys_wait_after_next_request_draws() {
        let mut review_session = make_review_session(&[
            ("a.go", "// One.\n", 1, json!("keep")),
            ("b.go", "// Two.\n", 1, json!("keep")),
            ("c.go", "// Three.\n", 1, json!("keep")),
        ]);
        let start = Instant::now();
        let mut view = View::new(samples(), &mut review_session, PHONE)
            .expect("every request waits for User Feedback");
        view.start_wait(start);

        view.press_key(Key::Char('o'), after(start, 100));
        view.press_key(Key::Char('o'), after(start, 700));
        view.press_key(Key::Char('o'), after(start, 750));
        // The second request is drawn only after a slow answer check.
        view.start_wait(after(start, 2000));
        view.press_key(Key::Char('o'), after(start, 2100));

        assert_eq!(view.feedback.len(), 1);
        assert_eq!(view.index, 1);
        assert_eq!(view.notice, "One moment, then press again.");
    }

    /// Without it, a cut or reword the code cannot take would be kept, or the
    /// user would read a reason written for agents.
    #[test]
    fn refused_answer_shows_one_plain_message() {
        let cases = [
            ("comment must stay", "store.go", "/* the saved name */", 32),
            ("file is gone", "missing.go", "// Gone.\n", 1),
        ];

        for (case, file, text, line) in cases {
            let mut review_session = make_review_session(&[(file, text, line, json!("cut"))]);
            let start = Instant::now();
            let mut view = View::new(samples(), &mut review_session, PHONE)
                .expect("the request waits for User Feedback");
            view.start_wait(start);

            view.press_key(Key::Char('c'), after(start, 700));

            assert!(view.feedback.is_empty(), "{case}");
            assert_eq!(
                view.notice, "That change can't be made to this comment.",
                "{case}"
            );
        }
    }

    /// Without it, Enter alone on a saved note would erase it, and a note with
    /// a q in it would close the screen.
    #[test]
    fn note_keeps_saved_text_and_typed_q() {
        let mut review_session = make_review_session(&[
            ("a.go", "// One.\n", 1, json!("keep")),
            ("b.go", "// Two.\n", 1, json!("keep")),
        ]);
        let first = review_session.lists[0].requests[0].id;
        let saved = UserFeedback::Note(UserNote("Say why".to_owned()));
        review_session.feedback.insert(first, saved);
        let start = Instant::now();
        let mut view = View::new(samples(), &mut review_session, PHONE)
            .expect("the second request waits for User Feedback");
        view.press_key(Key::Char('h'), start);
        view.start_wait(start);

        let keys = [
            Key::Char('m'),
            Key::Enter,
            Key::Char('m'),
            Key::Char('q'),
            Key::Enter,
        ];
        let after_keys: Vec<AfterKey> = keys
            .into_iter()
            .map(|key| view.press_key(key, after(start, 700)))
            .collect();

        assert!(
            after_keys
                .iter()
                .all(|after_key| *after_key == AfterKey::Stay)
        );
        let note = match view.feedback.get(&first) {
            Some(UserFeedback::Note(UserNote(note))) => note.as_str(),
            _ => "",
        };
        assert_eq!(note, "Say whyq");
    }

    /// Without it, Ctrl-C would type into the note, and the user could not
    /// close the screen while the note prompt is open.
    #[test]
    fn ctrl_c_quits_while_typing_a_note() {
        let mut review_session = make_review_session(&[("a.go", "// One.\n", 1, json!("keep"))]);
        let start = Instant::now();
        let mut view = View::new(samples(), &mut review_session, PHONE)
            .expect("the request waits for User Feedback");
        view.start_wait(start);
        view.press_key(Key::Char('m'), after(start, 700));

        let after_interrupt = view.press_key(Key::Interrupt, after(start, 800));

        assert_eq!(after_interrupt, AfterKey::Quit);
    }
}
