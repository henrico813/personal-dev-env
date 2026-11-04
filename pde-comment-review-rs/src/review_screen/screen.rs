//! This module holds what the Review Screen shows and answers each key, with
//! the Stray Tap Guards that stop a stray tap from giving User Feedback. Read
//! the `ReviewScreen::new(root, review_session, (cols, rows))` and
//! `ReviewScreen::press_key(key, now)` functions to understand the module.
//! A key that gives User Feedback changes the Review Session in memory, and
//! `ReviewScreen::save_given_feedback` in `saving.rs` then saves it.

use std::path::Path;
use std::time::{Duration, Instant};

use super::keys::Key;
use super::result::ReviewResult;
use super::surrounding_code::{CodeNotShown, SurroundingCode, read_surrounding_code};
use crate::shared::comments::{CommentEdit, rewrite};
use crate::shared::review_session::{FeedbackRequest, RequestId, ReviewSession};
use crate::shared::user_feedback::{UserAnswer, UserFeedback, UserNote, choose_new_text};

/// How long `a`, `o`, `c` and `m` wait after a request shows, so a double tap
/// cannot answer two requests.
const SETTLE: Duration = Duration::from_millis(600);

/// How many rows the Review Screen Header takes at the top.
pub(super) const HEADER_ROWS: usize = 2;
/// How many rows Key Help takes at the bottom.
pub(super) const KEY_HELP_ROWS: usize = 2;

/// Shows for a refused answer. The reasons rewrite gives are written for
/// agents, so the screen leaves them out.
pub(super) const REFUSED: &str =
    "That change can't be made to this comment. Press o to keep it or m to ask the agent.";
/// Shows for a refused cut of a docstring that Python needs as the body.
const REFUSED_ONLY_STATEMENT: &str = "That change can't be made to this comment. This docstring is the only statement in its block. Press o to keep it or m to ask the agent.";

/// What the screen does after a key or a save.
#[derive(Debug, PartialEq)]
pub(super) enum AfterKey {
    Stay,
    Quit,
    /// Saves the User Feedback this key gave the request in memory, or removes
    /// its User Note when it has none.
    Save(RequestId),
    /// Opens the Comment Edit Page in nvim.
    Edit,
    /// Opens the request's file in nvim to read.
    View,
    /// Searches the repository for the typed text with `rg`.
    Search(String),
    /// Shows the Review Result Screen.
    ShowResult(ReviewResult),
    /// Shows the changes in the review's files, from the Review Result Screen.
    Diff,
}

/// Says what the user types at the Note and Search Prompt.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum PromptKind {
    Note,
    Search,
}

/// Holds the Note and Search Prompt while the user types.
pub(super) struct Prompt {
    pub(super) kind: PromptKind,
    pub(super) typed: String,
}

/// Shows one Feedback Request at a time from the Review Session it borrows.
pub(super) struct ReviewScreen<'a> {
    pub(super) root: &'a Path,
    pub(super) review_session: &'a mut ReviewSession,
    pub(super) index: usize,
    /// The shown request's comment and code, or why they cannot be shown.
    pub(super) code: Result<SurroundingCode, CodeNotShown>,
    pub(super) scroll: usize,
    pub(super) cols: usize,
    pub(super) rows: usize,
    /// Says why a key did nothing, until the next key.
    pub(super) notice: String,
    /// Holds what the user types, while the Note and Search Prompt is open.
    pub(super) prompt: Option<Prompt>,
    /// When the shown request was first drawn, or `None` until it is.
    drawn_at: Option<Instant>,
}

impl<'a> ReviewScreen<'a> {
    /// Returns a screen for the newest version of each request, showing the
    /// first one with no User Feedback, or `None` when every request has some.
    /// Reads each request's file under `root`, the worktree's top folder.
    pub(super) fn new(
        root: &'a Path,
        review_session: &'a mut ReviewSession,
        (cols, rows): (usize, usize),
    ) -> Option<Self> {
        let first = review_session
            .newest_requests()
            .iter()
            .position(|request| !review_session.feedback.contains_key(&request.id))?;
        let mut screen = ReviewScreen {
            root,
            review_session,
            index: first,
            // show below finds the first request's code.
            code: Err(CodeNotShown::CommentGone),
            scroll: 0,
            cols,
            rows,
            notice: String::new(),
            prompt: None,
            drawn_at: None,
        };
        screen.show(first);
        Some(screen)
    }

    /// Returns the newest version of each request, in ID order.
    pub(super) fn requests(&self) -> Vec<&FeedbackRequest> {
        self.review_session.newest_requests()
    }

    /// Returns the request on screen.
    pub(super) fn shown_request(&self) -> &FeedbackRequest {
        self.requests()[self.index]
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

    /// Returns the furthest the body can scroll, with its last row at the bottom.
    fn find_max_scroll(&self) -> usize {
        self.build_body()
            .rows
            .len()
            .saturating_sub(self.body_room())
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

    /// Makes the answer keys wait for the next draw again, as after nvim or
    /// less closes, so keys typed into them answer nothing.
    pub(super) fn wait_for_draw(&mut self) {
        self.drawn_at = None;
    }

    /// Shows request `index` scrolled to the agent's notes, past the code
    /// above. The answer keys wait until it is drawn.
    pub(super) fn show(&mut self, index: usize) {
        self.index = index;
        self.prompt = None;
        let code = read_surrounding_code(self.root, self.shown_request());
        self.code = code;
        self.scroll = self.build_body().details_start.min(self.find_max_scroll());
        self.drawn_at = None;
    }

    /// Acts on one key. `l` and `h` wrap around at the last and first request.
    pub(super) fn press_key(&mut self, key: Key, now: Instant) -> AfterKey {
        self.notice.clear();
        if key == Key::Interrupt {
            return AfterKey::Quit;
        }
        if self.prompt.is_some() {
            return self.type_into_prompt(key);
        }
        let count = self.requests().len();
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
            Key::Char('m') => {
                self.prompt = Some(Prompt {
                    kind: PromptKind::Note,
                    typed: self.find_saved_note(),
                });
            }
            Key::Char('s') => {
                self.prompt = Some(Prompt {
                    kind: PromptKind::Search,
                    typed: String::new(),
                });
            }
            Key::Char('a') => return self.answer(UserAnswer::Accept),
            Key::Char('o') => return self.answer(UserAnswer::Keep),
            Key::Char('c') => return self.answer(UserAnswer::Cut),
            Key::Char('e') if !self.shown_request().can_reword => {
                self.notice = REFUSED.to_owned();
            }
            Key::Char('e' | 'v') => match &self.code {
                // nvim needs the comment in its file, for the edit page and for the line to open.
                Err(not_shown) => self.notice = not_shown.message().to_owned(),
                Ok(_) if key == Key::Char('e') => return AfterKey::Edit,
                Ok(_) => return AfterKey::View,
            },
            _ => {}
        }
        AfterKey::Stay
    }

    /// Returns true once `SETTLE` has passed since the shown request was drawn.
    fn has_waited(&self, now: Instant) -> bool {
        self.drawn_at
            .is_some_and(|drawn_at| now.duration_since(drawn_at) >= SETTLE)
    }

    /// Types into the prompt. A note starts as the saved one, so Enter alone
    /// changes nothing, and an empty note removes it.
    fn type_into_prompt(&mut self, key: Key) -> AfterKey {
        let Some(prompt) = &mut self.prompt else {
            return AfterKey::Stay;
        };
        match key {
            Key::Char(character) => prompt.typed.push(character),
            Key::Backspace => {
                prompt.typed.pop();
            }
            Key::Enter => {
                let typed = prompt.typed.trim().to_owned();
                let kind = prompt.kind;
                self.prompt = None;
                return match kind {
                    PromptKind::Note => self.give_note(typed),
                    PromptKind::Search if typed.is_empty() => AfterKey::Stay,
                    PromptKind::Search => AfterKey::Search(typed),
                };
            }
            _ => {}
        }
        AfterKey::Stay
    }

    /// Returns the shown request's saved User Note, or an empty one.
    fn find_saved_note(&self) -> String {
        match self.review_session.feedback.get(&self.shown_request().id) {
            Some(UserFeedback::Note(UserNote(note))) => note.clone(),
            _ => String::new(),
        }
    }

    /// Gives the shown request the note and stays on it, or removes its User
    /// Note when the note is empty. A note left as it was saves nothing.
    fn give_note(&mut self, note: String) -> AfterKey {
        let saved_note = self.find_saved_note();
        let id = self.shown_request().id;
        let feedback = &mut self.review_session.feedback;
        if note == saved_note {
            return AfterKey::Stay;
        }
        if !note.is_empty() {
            feedback.insert(id, UserFeedback::Note(UserNote(note)));
        } else {
            feedback.remove(&id);
        }
        AfterKey::Save(id)
    }

    /// Gives the shown request the answer and shows the next request without
    /// User Feedback, or says why the answer was refused and keeps nothing.
    pub(super) fn answer(&mut self, answer: UserAnswer) -> AfterKey {
        if let Err(refusal) = self.check_answer(&answer) {
            self.notice = refusal.to_owned();
            return AfterKey::Stay;
        }
        let id = self.shown_request().id;
        self.review_session
            .feedback
            .insert(id, UserFeedback::Answer(answer));
        if let Some(next) = self.find_next_without_feedback() {
            self.show(next);
        }
        AfterKey::Save(id)
    }

    /// Rewrites the comment's file in memory, so a cut or reword the code
    /// cannot take is refused now rather than after the last answer. A refusal
    /// holds the message the user reads.
    fn check_answer(&self, answer: &UserAnswer) -> Result<(), &'static str> {
        let request = self.shown_request();
        let new_text = choose_new_text(request, answer);
        if new_text.as_deref() == Some(request.text.as_str()) {
            return Ok(());
        }
        // rewrite needs the comment as found in its file.
        let Ok(code) = &self.code else {
            return Err(REFUSED);
        };
        let is_cut = new_text.is_none();
        let edit = CommentEdit {
            comment: code.comment.clone(),
            new_text,
        };
        match rewrite(self.root, &[edit]) {
            Ok(_) => Ok(()),
            Err(_) if is_cut && code.is_only_statement_in_block => Err(REFUSED_ONLY_STATEMENT),
            Err(_) => Err(REFUSED),
        }
    }

    /// Returns the next request after the shown one, wrapping around, that
    /// has no User Feedback.
    pub(super) fn find_next_without_feedback(&self) -> Option<usize> {
        let requests = self.requests();
        let count = requests.len();
        (1..=count)
            .map(|step| (self.index + step) % count)
            .find(|&index| {
                !self
                    .review_session
                    .feedback
                    .contains_key(&requests[index].id)
            })
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;

    /// The 40x16 phone popup's columns and rows.
    const PHONE: (usize, usize) = (40, 16);

    /// The folder of sample files the made-up requests ask about.
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

    /// Returns the moment `millis` milliseconds after `start`.
    fn after(start: Instant, millis: u64) -> Instant {
        start + Duration::from_millis(millis)
    }

    /// Returns the User Note saved for request `id`, or an empty text.
    fn find_note(screen: &ReviewScreen, id: RequestId) -> String {
        match screen.review_session.feedback.get(&id) {
            Some(UserFeedback::Note(UserNote(note))) => note.clone(),
            _ => String::new(),
        }
    }

    /// Presses `c` on one request that suggests a cut, and returns the notice
    /// and whether the screen kept no User Feedback.
    fn press_cut(file: &str, text: &str, line: usize) -> (String, bool) {
        let mut review_session = make_review_session(&[(file, text, line, json!("cut"))]);
        let start = Instant::now();
        let mut screen = ReviewScreen::new(samples(), &mut review_session, PHONE)
            .expect("the request waits for User Feedback");
        screen.start_wait(start);

        screen.press_key(Key::Char('c'), after(start, 700));

        (
            screen.notice.clone(),
            screen.review_session.feedback.is_empty(),
        )
    }

    /// `press_key` must ignore answer keys for 600 ms after the next request
    /// is drawn, not after the key that showed it. Otherwise a second tap read
    /// during a slow answer check answers a request the user has not seen.
    #[test]
    fn press_key_ignores_answers_until_request_drawn() {
        let mut review_session = make_review_session(&[
            ("a.go", "// One.\n", 1, json!("keep")),
            ("b.go", "// Two.\n", 1, json!("keep")),
            ("c.go", "// Three.\n", 1, json!("keep")),
        ]);
        let start = Instant::now();
        let mut screen = ReviewScreen::new(samples(), &mut review_session, PHONE)
            .expect("every request waits for User Feedback");
        screen.start_wait(start);

        screen.press_key(Key::Char('o'), after(start, 100));
        screen.press_key(Key::Char('o'), after(start, 700));
        screen.press_key(Key::Char('o'), after(start, 750));
        // The second request is drawn only after a slow answer check.
        screen.start_wait(after(start, 2000));
        screen.press_key(Key::Char('o'), after(start, 2100));

        assert_eq!(screen.review_session.feedback.len(), 1);
        assert_eq!(screen.index, 1);
        assert_eq!(screen.notice, "One moment, then press again.");
    }

    /// `press_key` must refuse a cut the code cannot take, keep nothing, and
    /// say what to press next. Otherwise the user is stuck on a request with
    /// no way forward, or reads a reason written for agents.
    #[test]
    fn press_key_refuses_cut_with_next_step() {
        let cases = [
            ("comment must stay", "store.go", "/* the saved name */", 32),
            ("file is gone", "missing.go", "// Gone.\n", 1),
        ];

        for (case, file, text, line) in cases {
            let (notice, kept_nothing) = press_cut(file, text, line);

            assert!(kept_nothing, "{case}");
            assert_eq!(
                notice,
                "That change can't be made to this comment. Press o to keep it or m to ask the agent.",
                "{case}"
            );
        }
    }

    /// `press_key` must say why `c` does nothing on a docstring that is the
    /// only statement in its block. Otherwise the user cannot tell that
    /// Python needs the docstring as the function's body.
    #[test]
    fn press_key_explains_only_statement_docstring_cut() {
        let (notice, kept_nothing) =
            press_cut("notes.py", "        \"\"\"Forgets every note.\"\"\"\n", 25);

        assert!(kept_nothing);
        assert_eq!(
            notice,
            "That change can't be made to this comment. This docstring is the only statement in its block. Press o to keep it or m to ask the agent."
        );
    }

    /// `press_key` must start the note prompt with the saved User Note, so
    /// Enter alone keeps it. Otherwise opening the prompt to read a note
    /// erases it.
    #[test]
    fn press_key_enter_keeps_saved_note() {
        let mut review_session = make_review_session(&[
            ("a.go", "// One.\n", 1, json!("keep")),
            ("b.go", "// Two.\n", 1, json!("keep")),
        ]);
        let first = review_session.lists[0].requests[0].id;
        let saved = UserFeedback::Note(UserNote("Say why".to_owned()));
        review_session.feedback.insert(first, saved);
        let start = Instant::now();
        let mut screen = ReviewScreen::new(samples(), &mut review_session, PHONE)
            .expect("the second request waits for User Feedback");
        screen.press_key(Key::Char('h'), start);
        screen.start_wait(start);

        screen.press_key(Key::Char('m'), after(start, 700));
        screen.press_key(Key::Enter, after(start, 700));

        assert_eq!(find_note(&screen, first), "Say why");
    }

    /// `press_key` must type `q` into the note instead of closing the screen.
    /// Otherwise a note with a q in it quits and loses the note.
    #[test]
    fn press_key_types_q_into_note() {
        let mut review_session = make_review_session(&[
            ("a.go", "// One.\n", 1, json!("keep")),
            ("b.go", "// Two.\n", 1, json!("keep")),
        ]);
        let first = review_session.lists[0].requests[0].id;
        let start = Instant::now();
        let mut screen = ReviewScreen::new(samples(), &mut review_session, PHONE)
            .expect("every request waits for User Feedback");
        screen.start_wait(start);

        let keys = [Key::Char('m'), Key::Char('q'), Key::Enter];
        let after_keys: Vec<AfterKey> = keys
            .into_iter()
            .map(|key| screen.press_key(key, after(start, 700)))
            .collect();

        assert_eq!(
            after_keys,
            [AfterKey::Stay, AfterKey::Stay, AfterKey::Save(first)]
        );
        assert_eq!(find_note(&screen, first), "q");
    }

    /// `press_key` must quit on Ctrl-C while the note prompt is open.
    /// Otherwise Ctrl-C types into the note, and the user cannot close the
    /// screen while typing.
    #[test]
    fn press_key_interrupt_quits_while_typing_note() {
        let mut review_session = make_review_session(&[("a.go", "// One.\n", 1, json!("keep"))]);
        let start = Instant::now();
        let mut screen = ReviewScreen::new(samples(), &mut review_session, PHONE)
            .expect("the request waits for User Feedback");
        screen.start_wait(start);
        screen.press_key(Key::Char('m'), after(start, 700));

        let after_interrupt = screen.press_key(Key::Interrupt, after(start, 800));

        assert_eq!(after_interrupt, AfterKey::Quit);
    }
}
