//! This module lays out the Review Screen's rows, from the Review Screen
//! Header and the Feedback Request Details down to the Surrounding Code and
//! Key Help. Read the `ReviewScreen::build_screen()` and
//! `ReviewScreen::build_body()` functions to understand the module.

use std::ops::Range;

use super::fit::{clip, cut_middle, display_width, escape_control_characters, fold, last_columns};
use super::screen::{HEADER_ROWS, KEY_HELP_ROWS, ReviewScreen};
use crate::shared::comments::to_plain_words;
use crate::shared::review_session::{RequestReason, SuggestionKind};
use crate::shared::user_feedback::{UserAnswer, UserFeedback, UserNote};

/// Keeps a long repository name from pushing its branch off the first row.
const MAX_REPO_COLUMNS: usize = 20;

/// How a piece of a row is colored.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Style {
    Plain,
    Title,
    Status,
    Faded,
    Note,
    UserNote,
    Old,
    New,
}

impl Style {
    /// Returns the terminal code that turns this style on.
    fn to_ansi(self) -> &'static str {
        match self {
            Self::Plain => "",
            Self::Title => "\x1b[1m",
            Self::Status => "\x1b[7m",
            Self::Faded => "\x1b[2m",
            Self::Note => "\x1b[33m",
            Self::UserNote => "\x1b[33;1m",
            Self::Old => "\x1b[31m",
            Self::New => "\x1b[32m",
        }
    }
}

/// Holds one screen row as pieces of text, each with a style.
pub(super) struct Row(Vec<(Style, String)>);

impl Row {
    /// Returns a row of one piece of text in one style.
    fn new(style: Style, text: String) -> Self {
        Row(vec![(style, text)])
    }

    /// Returns the row's text with its colors, ready for the terminal.
    pub(super) fn to_ansi(&self) -> String {
        let mut line = String::new();
        for (style, text) in &self.0 {
            if *style == Style::Plain {
                line.push_str(text);
            } else {
                line.push_str(&format!("{}{text}\x1b[0m", style.to_ansi()));
            }
        }
        line
    }
}

impl ReviewScreen<'_> {
    /// Returns one row per terminal row, none wider than the terminal, since a
    /// row the terminal wraps would push the header off a phone screen.
    pub(super) fn build_screen(&self) -> Vec<Row> {
        let mut screen = Vec::from(self.build_header());
        let room = self.body_room();
        let mut body = self.build_body();
        // The faded code row just above the details shows when the scroll stops
        // early, and then the def line would show twice.
        let shows_code_above =
            self.scroll < body.details_start && body.details_start <= self.scroll + room;
        if let Some(repeated) = body.repeated_definition_rows.clone()
            && shows_code_above
        {
            body.rows.drain(repeated);
        }
        let hidden_below = body.rows.len().saturating_sub(self.scroll + room);
        let mut shown: Vec<Row> = body.rows.into_iter().skip(self.scroll).take(room).collect();
        // The "more" row covers the last body row, so that row counts as hidden.
        if let Some(last) = shown.last_mut()
            && hidden_below > 0
        {
            let more = format!("  ... {} more (j)", hidden_below + 1);
            *last = Row::new(Style::Faded, clip(&more, self.cols).to_owned());
        }
        screen.extend(shown);
        while screen.len() < self.rows.saturating_sub(KEY_HELP_ROWS) {
            screen.push(Row::new(Style::Plain, String::new()));
        }
        if !self.notice.is_empty() {
            // A long notice wraps upward over the body, never over the header.
            let notice = fold(&escape_control_characters(&self.notice), self.cols);
            let room_below_header = screen.len().saturating_sub(HEADER_ROWS);
            let notice = &notice[notice.len().saturating_sub(room_below_header)..];
            let first = screen.len() - notice.len();
            for (row, text) in screen[first..].iter_mut().zip(notice) {
                *row = Row::new(Style::Note, text.clone());
            }
        }
        screen.extend(self.build_key_help());
        screen
    }

    /// Returns "repo · branch", cutting the middle of a long branch so both
    /// its ends stay readable.
    fn build_label(&self) -> String {
        let repo = escape_control_characters(&self.origin.repo);
        let branch = self.origin.branch.rsplit('/').next().unwrap_or_default();
        let branch = escape_control_characters(branch);
        let label = format!("{repo} · {branch}");
        if display_width(&label) <= self.cols {
            return label;
        }
        let repo = if display_width(&repo) > MAX_REPO_COLUMNS {
            format!("{}…", clip(&repo, MAX_REPO_COLUMNS - 1))
        } else {
            repo
        };
        let room = self.cols.saturating_sub(display_width(&repo) + 3);
        let label = format!("{repo} · {}", cut_middle(&branch, room));
        clip(&label, self.cols).to_owned()
    }

    /// Returns the Review Screen Header. Its first row is the label, and its
    /// second has the request's number, suggestion and file, the rows scrolled
    /// past and the User Feedback so far.
    fn build_header(&self) -> [Row; HEADER_ROWS] {
        let request = self.requests[self.index];
        let suggestion = match request
            .suggestion
            .as_ref()
            .map(|suggestion| &suggestion.kind)
        {
            Some(SuggestionKind::Cut) => "cut",
            Some(SuggestionKind::Reword { .. }) => "reword",
            Some(SuggestionKind::Keep) | None => "keep",
        };
        let file_name = request
            .file
            .file_name()
            .unwrap_or(request.file.as_os_str())
            .to_string_lossy();
        let mut title = format!(
            "{}/{} {suggestion}? {file_name}",
            self.index + 1,
            self.requests.len()
        );
        if self.scroll > 0 {
            title.push_str(&format!(" ↑{}", self.scroll));
        }
        let status = match self.feedback.get(&request.id) {
            None => "?",
            Some(UserFeedback::Note(_)) => "note",
            Some(UserFeedback::Answer(UserAnswer::Keep)) => "kept",
            Some(UserFeedback::Answer(UserAnswer::Cut)) => "cut",
            Some(UserFeedback::Answer(UserAnswer::Accept)) => "accepted",
            Some(UserFeedback::Answer(UserAnswer::Edited { .. })) => "edited",
        };
        let width = self.cols.saturating_sub(display_width(status) + 1);
        let title = escape_control_characters(&title);
        let title = clip(&title, width);
        let padding = " ".repeat(width - display_width(title));
        [
            Row::new(Style::Plain, self.build_label()),
            Row(vec![
                (Style::Title, format!("{title}{padding}")),
                (Style::Plain, " ".to_owned()),
                (Style::Status, status.to_owned()),
            ]),
        ]
    }

    /// Returns the key help, or the note prompt while the user types a note.
    fn build_key_help(&self) -> [Row; KEY_HELP_ROWS] {
        let hint = |keys: &str| Row::new(Style::Faded, clip(keys, self.cols).to_owned());
        let Some(typed) = &self.typed_note else {
            return [
                hint("a accept o keep c cut m note"),
                hint("j k scroll l h next q quit"),
            ];
        };
        /// Starts the note prompt's row, before the typed note.
        const PROMPT: &str = "note for AI: ";
        // The end of a long note stays in view, so what was just typed shows.
        let room = self.cols.saturating_sub(PROMPT.len()).max(1);
        let typed = escape_control_characters(typed);
        let prompt = format!("{PROMPT}{}", last_columns(&typed, room));
        [hint("type, then Enter"), Row::new(Style::Plain, prompt)]
    }

    /// Returns the request's rows with their Surrounding Code.
    pub(super) fn build_body(&self) -> Body {
        let request = self.requests[self.index];
        // One column holds the -, + or > mark.
        let mut body = BodyRows {
            rows: Vec::new(),
            width: self.cols.saturating_sub(1).max(10),
        };
        let (above, below) = match &self.code {
            Ok(code) => (code.above.as_slice(), code.below.as_slice()),
            Err(_) => (&[][..], &[][..]),
        };
        for line in above {
            body.add(Style::Faded, " ", line);
        }
        let details_start = body.rows.len();
        if let Err(not_shown) = &self.code {
            body.add(Style::Note, ">", not_shown.message());
        }
        let warning = match request.reason {
            RequestReason::New => None,
            RequestReason::WasCut => Some("warning: you cut this comment before"),
            RequestReason::DiffersFromApproved => Some("warning: you approved other text before"),
            RequestReason::AgentWantsChange => Some("warning: you approved the old comment"),
        };
        if let Some(warning) = warning {
            body.add(Style::Note, ">", warning);
        }
        if let Some(UserFeedback::Note(UserNote(note))) = self.feedback.get(&request.id) {
            body.add(Style::UserNote, ">", &format!("note: {note}"));
        }
        if let Some(suggestion) = &request.suggestion {
            if !suggestion.also_at.is_empty() {
                let places = suggestion.also_at.join(", ");
                body.add(Style::Note, ">", &format!("also at: {places}"));
            }
            if !suggestion.reasoning.is_empty() {
                body.add(Style::Note, ">", &format!("why: {}", suggestion.reasoning));
            }
        }
        let mut repeated_definition_rows = None;
        if let Ok(code) = &self.code
            && let Some(definition_line) = &code.definition_line
        {
            let start = body.rows.len();
            body.add(Style::Plain, " ", definition_line.trim_start());
            if above.last() == Some(definition_line) {
                repeated_definition_rows = Some(start..body.rows.len());
            }
        }
        let old_words = to_plain_words(&request.file, &request.text);
        match request
            .suggestion
            .as_ref()
            .map(|suggestion| &suggestion.kind)
        {
            Some(SuggestionKind::Reword { text }) => {
                body.add_lines(Style::Old, "-", &old_words);
                body.add_lines(Style::New, "+", text);
            }
            Some(SuggestionKind::Cut) => {
                body.add_lines(Style::Old, "-", &old_words);
                body.add(Style::New, "+", "(cut)");
            }
            Some(SuggestionKind::Keep) | None => {
                body.add(Style::Note, ">", "no change proposed");
                body.add_lines(Style::Plain, " ", &old_words);
            }
        }
        for line in below {
            body.add(Style::Faded, " ", line);
        }
        Body {
            rows: body.rows,
            details_start,
            repeated_definition_rows,
        }
    }
}

/// Holds the body's rows and where the screen opens on them.
pub(super) struct Body {
    pub(super) rows: Vec<Row>,
    /// Holds the index of the first row after the code above, which the
    /// screen opens at.
    pub(super) details_start: usize,
    /// Holds the def line's rows when the code just above the comment is the
    /// same line.
    repeated_definition_rows: Option<Range<usize>>,
}

/// Collects body rows, each folded to fit after its one-column mark.
struct BodyRows {
    rows: Vec<Row>,
    width: usize,
}

impl BodyRows {
    /// Adds one line as rows that fit the width. Rows after the first get a
    /// blank mark.
    fn add(&mut self, style: Style, mark: &str, line: &str) {
        let line = escape_control_characters(&line.replace('\t', "  "));
        for (index, row) in fold(&line, self.width).into_iter().enumerate() {
            let mark = if index == 0 { mark } else { " " };
            self.rows.push(Row::new(style, format!("{mark}{row}")));
        }
    }

    /// Adds each line of `text`, so a paragraph break or an example line
    /// keeps its own row.
    fn add_lines(&mut self, style: Style, mark: &str, text: &str) {
        for line in text.lines() {
            self.add(style, mark, line);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::time::{Duration, Instant};

    use serde_json::{Value, json};

    use super::*;
    use crate::review_screen::keys::Key;
    use crate::shared::review_session::ReviewSession;

    /// The 40x16 phone popup's columns and rows.
    const PHONE: (usize, usize) = (40, 16);
    /// A screen wide enough that no row in a body test folds.
    const WIDE: (usize, usize) = (80, 24);

    /// The long Go comment in `store.go`, with a URL that cannot fold.
    const SAVE_ANSWER_DOC: &str = "\
// Saves the owner's answer for one comment under the review's lock, and reads
// the file again after taking the lock, so an answer another screen saved a
// moment ago is kept and a crash during the write leaves the old file whole.
// The reasons are written up at
// https://example.com/docs/comment-review/saving-answers-without-losing-any.
";

    /// The folder of sample files the made-up requests ask about.
    fn samples() -> &'static Path {
        Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/samples/screen"))
    }

    /// Returns a new Feedback Request about `text` at `line` of `file`, with
    /// no reasoning.
    fn make_request(file: &str, text: &str, line: usize, kind: Value) -> Value {
        json!({
            "id": 1,
            "file": file,
            "name": null,
            "reason": "new",
            "text": text,
            "can_reword": true,
            "line": line,
            "suggestion": {"kind": kind, "reasoning": "", "also_at": []},
        })
    }

    /// Returns a Review Session on a long branch that asks each request in
    /// order, numbering them from 1.
    fn make_review_session(mut requests: Vec<Value>) -> ReviewSession {
        for (index, request) in requests.iter_mut().enumerate() {
            request["id"] = json!(index + 1);
        }
        serde_json::from_value(json!({
            "status": "waiting_for_user_feedback",
            "origin": {
                "repo": "personal-dev-env",
                "branch": "feat/rust-review-screen-pieces-for-phones",
                "agent_session": "s",
            },
            "lists": [{"base": "4f2a9c1e", "requests": requests}],
            "feedback": {},
        }))
        .expect("the Review Session matches its saved shape")
    }

    /// Returns a Review Session with the long Go reword, then the Go comment
    /// that can only be kept, then a docstring sent again after a cut.
    fn make_phone_review_session() -> ReviewSession {
        let reasoning = "Says why the lock matters, which 面 readers need.";
        let mut reword = make_request(
            "store.go",
            SAVE_ANSWER_DOC,
            17,
            json!({"reword": {"text": "Saves one answer under the review's lock, so a second screen loses none."}}),
        );
        reword["suggestion"]["reasoning"] = json!(reasoning);
        let keep_only = make_request("store.go", "/* the saved name */", 32, json!("cut"));
        let mut docstring = make_request(
            "notes.py",
            "        \"\"\"Writes every note to notes.json.\"\"\"\n",
            20,
            json!({"reword": {"text": "Writes every note to notes.json, replacing the file the last save wrote."}}),
        );
        docstring["reason"] = json!("was_cut");
        docstring["suggestion"]["reasoning"] = json!(reasoning);
        make_review_session(vec![reword, keep_only, docstring])
    }

    /// Returns a row's text without its colors.
    fn row_text(row: &Row) -> String {
        row.0.iter().map(|(_, text)| text.as_str()).collect()
    }

    /// Returns the body's rows as text, and the index of the first row the
    /// screen opens at.
    fn build_body_text(review_session: &mut ReviewSession) -> (Vec<String>, usize) {
        let screen = ReviewScreen::new(samples(), review_session, WIDE)
            .expect("the request waits for User Feedback");
        let body = screen.build_body();
        (body.rows.iter().map(row_text).collect(), body.details_start)
    }

    /// `build_screen` must give 16 rows of at most 40 columns on the phone
    /// popup, with a long notice, a warning, a Function Definition Line or the
    /// note prompt. Otherwise the terminal wraps a row and pushes the Review
    /// Screen Header off the phone screen.
    #[test]
    fn build_screen_fits_phone_popup() {
        let mut review_session = make_phone_review_session();
        let start = Instant::now();
        let later = start + Duration::from_secs(1);
        let mut screen = ReviewScreen::new(samples(), &mut review_session, PHONE)
            .expect("the requests wait for User Feedback");
        screen.start_wait(start);

        // The long Go reword opens first.
        let opened = screen.build_screen();
        // A cut of the comment that can only be kept shows a long notice.
        screen.press_key(Key::Char('l'), start);
        screen.start_wait(start);
        screen.press_key(Key::Char('c'), later);
        let refused = screen.build_screen();
        // The docstring shows its warning and its def line.
        screen.press_key(Key::Char('l'), later);
        screen.start_wait(start);
        let detailed = screen.build_screen();
        // A long note shows its end in the prompt.
        screen.press_key(Key::Char('m'), later);
        for character in "Say why the lock matters ".repeat(4).chars() {
            screen.press_key(Key::Char(character), later);
        }
        let typing = screen.build_screen();

        for rows in [&opened, &refused, &detailed, &typing] {
            assert_eq!(rows.len(), 16);
            for row in rows {
                assert!(display_width(&row_text(row)) <= 40, "{:?}", row_text(row));
            }
        }
    }

    /// `build_screen` must open each request at its Feedback Request Details,
    /// past the code above. Otherwise the phone screen opens on code, and the
    /// user must scroll to find the agent's reasons.
    #[test]
    fn build_screen_opens_at_request_details() {
        let mut review_session = make_phone_review_session();
        let screen = ReviewScreen::new(samples(), &mut review_session, PHONE)
            .expect("the requests wait for User Feedback");

        let opened = screen.build_screen();

        assert!(row_text(&opened[HEADER_ROWS]).starts_with(">why: Says why"));
    }

    /// `build_screen` must wrap a long Review Notice upward from just above
    /// Key Help. Otherwise the end of the notice, which says what to press
    /// next, is cut off.
    #[test]
    fn build_screen_wraps_long_notice_upward() {
        let mut review_session = make_phone_review_session();
        let start = Instant::now();
        let mut screen = ReviewScreen::new(samples(), &mut review_session, PHONE)
            .expect("the requests wait for User Feedback");
        // Request 2 is a comment that can only be kept.
        screen.press_key(Key::Char('l'), start);
        screen.start_wait(start);

        screen.press_key(Key::Char('c'), start + Duration::from_secs(1));
        let rows: Vec<String> = screen.build_screen().iter().map(row_text).collect();

        assert!(
            rows.iter()
                .any(|row| row.starts_with("That change can't be made"))
        );
        assert!(rows[13].trim_end().ends_with("agent."), "{rows:?}");
    }

    /// `build_screen` must show a docstring's Function Definition Line once,
    /// whether or not the code just above the comment is on screen. Otherwise
    /// the user reads the same `def` line twice, or not at all.
    #[test]
    fn build_screen_shows_definition_line_once() {
        let docstring = "        \"\"\"Writes every note to notes.json.\"\"\"\n";
        // A short body makes the scroll stop on the code above the comment.
        let stops_early = make_request("notes.py", docstring, 20, json!("cut"));
        // A warning and long reasons let the scroll reach the details.
        let mut reaches_details = make_request("notes.py", docstring, 20, json!("cut"));
        reaches_details["reason"] = json!("was_cut");
        reaches_details["suggestion"]["reasoning"] = json!("Repeats the name. ".repeat(6));
        let cases = [
            ("stops early", stops_early),
            ("reaches details", reaches_details),
        ];

        for (case, request) in cases {
            let mut review_session = make_review_session(vec![request]);
            let screen = ReviewScreen::new(samples(), &mut review_session, PHONE)
                .expect("the request waits for User Feedback");

            let rows = screen.build_screen();

            let definition_rows = rows
                .iter()
                .filter(|row| row_text(row).trim() == "def save(self):")
                .count();
            assert_eq!(definition_rows, 1, "{case}");
        }
    }

    /// `build_body` must show the old comment after `-` and the new one after
    /// `+`, with `+(cut)` for a cut and `>no change proposed` for a keep.
    /// Otherwise the user accepts a change they did not read.
    #[test]
    fn build_body_shows_old_and_new_comment() {
        let cases = [
            (
                "reword",
                json!({"reword": {"text": "Saves it once."}}),
                ["-Saves it.", "+Saves it once."],
            ),
            ("cut", json!("cut"), ["-Saves it.", "+(cut)"]),
            ("keep", json!("keep"), [">no change proposed", " Saves it."]),
        ];

        for (case, kind, expected) in cases {
            let request = make_request("missing.go", "// Saves it.\n", 1, kind);
            let mut review_session = make_review_session(vec![request]);

            let (rows, _) = build_body_text(&mut review_session);

            assert_eq!(rows[rows.len() - 2..], expected, "{case}");
        }
    }

    /// `build_body` must show a docstring's Function Definition Line just
    /// above the old comment. Otherwise the user cannot tell which function
    /// the docstring describes without scrolling up.
    #[test]
    fn build_body_shows_definition_line_above_docstring() {
        let request = make_request(
            "notes.py",
            "        \"\"\"Writes every note to notes.json.\"\"\"\n",
            20,
            json!("cut"),
        );
        let mut review_session = make_review_session(vec![request]);

        let (rows, _) = build_body_text(&mut review_session);

        let old_comment = rows
            .iter()
            .position(|row| row.starts_with('-'))
            .expect("a cut shows the old comment");
        assert_eq!(rows[old_comment - 1], " def save(self):");
    }

    /// `build_body` must open on a warning that says why a request was sent
    /// again. Otherwise the user answers a comment they cut or approved before
    /// without knowing it.
    #[test]
    fn build_body_warns_why_request_was_sent() {
        let cases = [
            ("was_cut", ">warning: you cut this comment before"),
            (
                "differs_from_approved",
                ">warning: you approved other text before",
            ),
            (
                "agent_wants_change",
                ">warning: you approved the old comment",
            ),
        ];

        for (reason, expected) in cases {
            let mut request = make_request("store.go", "/* the saved name */", 32, json!("keep"));
            request["reason"] = json!(reason);
            let mut review_session = make_review_session(vec![request]);

            let (rows, details_start) = build_body_text(&mut review_session);

            assert_eq!(rows[details_start], expected, "{reason}");
        }
    }

    /// `build_body` must say in plain words why the code is not shown, and
    /// what to press next. Otherwise the user reads a raw error written for
    /// programmers and does not know what to do.
    #[test]
    fn build_body_says_next_step_without_code() {
        let cases = [
            (
                "file is gone",
                "missing.go",
                ">The file is gone. Press m to ask the agent.",
            ),
            (
                "comment is gone",
                "store.go",
                ">The comment is no longer in its file. Press m to ask the agent.",
            ),
        ];

        for (case, file, expected) in cases {
            let request = make_request(file, "// Not in the file.\n", 1, json!("keep"));
            let mut review_session = make_review_session(vec![request]);

            let (rows, details_start) = build_body_text(&mut review_session);

            assert_eq!(rows[details_start], expected, "{case}");
        }
    }
}
