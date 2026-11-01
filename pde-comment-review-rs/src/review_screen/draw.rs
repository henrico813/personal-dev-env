//! Lays out the Review Screen's rows, from the header and the agent's notes
//! down to the code around the comment and the key help.

use super::fit::{clip, cut_middle, display_width, escape_control_characters, fold, last_columns};
use super::view::{HEADER_ROWS, KEY_HELP_ROWS, View};
use crate::shared::comments::to_plain_words;
use crate::shared::review_session::SuggestionKind;
use crate::shared::user_feedback::{UserAnswer, UserFeedback, UserNote};

/// Keeps a long repository name from pushing its branch off the first row.
const MAX_REPO_COLUMNS: usize = 20;

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

impl View<'_> {
    /// Returns one row per terminal row, none wider than the terminal, since a
    /// row the terminal wraps would push the header off a phone screen.
    pub(super) fn build_screen(&self) -> Vec<Row> {
        let mut screen = vec![
            Row::new(Style::Plain, self.build_label()),
            self.build_header(),
        ];
        let room = self.body_room();
        let body = self.build_body().0;
        let hidden_below = body.len().saturating_sub(self.scroll + room);
        let mut shown: Vec<Row> = body.into_iter().skip(self.scroll).take(room).collect();
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

    /// Returns the second row, with the request's number, suggestion and file,
    /// the rows scrolled past and the User Feedback so far.
    fn build_header(&self) -> Row {
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
        Row(vec![
            (Style::Title, format!("{title}{padding}")),
            (Style::Plain, " ".to_owned()),
            (Style::Status, status.to_owned()),
        ])
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
        const PROMPT: &str = "note for AI: ";
        // The end of a long note stays in view, so what was just typed shows.
        let room = self.cols.saturating_sub(PROMPT.len()).max(1);
        let typed = escape_control_characters(typed);
        let prompt = format!("{PROMPT}{}", last_columns(&typed, room));
        [hint("type, then Enter"), Row::new(Style::Plain, prompt)]
    }

    /// Returns the request's rows with the code around them, and the index of
    /// the first row after the code above, which the screen opens at.
    pub(super) fn build_body(&self) -> (Vec<Row>, usize) {
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
        let notes_start = body.rows.len();
        if let Err(reason) = &self.code {
            body.add(Style::Note, ">", &format!("cannot show the code: {reason}"));
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
        (body.rows, notes_start)
    }
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

    use serde_json::json;

    use super::*;
    use crate::review_screen::keys::Key;
    use crate::shared::review_session::ReviewSession;

    const SAVE_ANSWER_DOC: &str = "\
// Saves the owner's answer for one comment under the review's lock, and reads
// the file again after taking the lock, so an answer another screen saved a
// moment ago is kept and a crash during the write leaves the old file whole.
// The reasons are written up at
// https://example.com/docs/comment-review/saving-answers-without-losing-any.
";

    fn row_text(row: &Row) -> String {
        row.0.iter().map(|(_, text)| text.as_str()).collect()
    }

    /// Without it, a row wider than the 40x16 phone popup pushes the header off
    /// screen, or the screen opens on code instead of the agent's reasons. A
    /// long refusal's reason could also be cut off.
    #[test]
    fn fits_phone_opens_at_notes_shows_notice() {
        let request = |id: u32, text: &str, line: usize, kind| {
            json!({
                "id": id,
                "file": "store.go",
                "name": null,
                "reason": "new",
                "text": text,
                "can_reword": true,
                "line": line,
                "suggestion": {"kind": kind, "reasoning": "Says why the lock matters, which 面 readers need.", "also_at": []},
            })
        };
        let mut review_session: ReviewSession = serde_json::from_value(json!({
            "status": "waiting_for_user_feedback",
            "origin": {
                "repo": "personal-dev-env",
                "branch": "feat/rust-review-screen-pieces-for-phones",
                "agent_session": "s",
            },
            "lists": [{"base": "4f2a9c1e", "requests": [
                request(1, SAVE_ANSWER_DOC, 17, json!({"reword": {"text": "Saves one answer under the review's lock, so a second screen loses none."}})),
                request(2, "/* the saved name */", 32, json!("cut")),
            ]}],
            "feedback": {},
        }))
        .expect("the Review Session matches its saved shape");
        let samples = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/samples/screen"));
        let start = Instant::now();
        let later = start + Duration::from_secs(1);
        let mut view = View::new(samples, &mut review_session, (40, 16))
            .expect("the requests wait for User Feedback");
        view.start_wait(start);

        let opened = view.build_screen();
        view.press_key(Key::Char('l'), start);
        view.start_wait(start);
        view.press_key(Key::Char('c'), later);
        let refused = view.build_screen();
        view.press_key(Key::Char('m'), later);
        for character in "Say why the lock matters ".repeat(4).chars() {
            view.press_key(Key::Char(character), later);
        }
        let typing = view.build_screen();

        assert!(row_text(&opened[2]).starts_with(">why: Says why"));
        let refused_rows: Vec<String> = refused.iter().map(row_text).collect();
        assert!(
            refused_rows
                .iter()
                .any(|row| row.starts_with("This answer cannot be written."))
        );
        assert!(
            refused_rows[13].trim_end().ends_with("code."),
            "{refused_rows:?}"
        );
        for screen in [&opened, &refused, &typing] {
            assert_eq!(screen.len(), 16);
            for row in screen {
                assert!(display_width(&row_text(row)) <= 40, "{:?}", row_text(row));
            }
        }
    }

    /// Without it, the screen could show the agent's new wording as the old one,
    /// or a cut as a keep, and the user would accept what they did not read.
    #[test]
    fn body_shows_old_and_new_wording() {
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
            let mut review_session: ReviewSession = serde_json::from_value(json!({
                "status": "waiting_for_user_feedback",
                "origin": {"repo": "repo", "branch": "main", "agent_session": "s"},
                "lists": [{"base": "4f2a9c1e", "requests": [{
                    "id": 1,
                    "file": "missing.go",
                    "name": null,
                    "reason": "new",
                    "text": "// Saves it.\n",
                    "can_reword": true,
                    "line": 1,
                    "suggestion": {"kind": kind, "reasoning": "", "also_at": []},
                }]}],
                "feedback": {},
            }))
            .expect("the Review Session matches its saved shape");
            let view = View::new(Path::new("/nonexistent"), &mut review_session, (40, 16))
                .expect("the request waits for User Feedback");

            let rows: Vec<String> = view.build_body().0.iter().map(row_text).collect();

            assert_eq!(rows[rows.len() - 2..], expected, "{case}");
        }
    }
}
