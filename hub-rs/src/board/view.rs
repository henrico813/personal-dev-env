//! Holds what the board shows and acts on keys, wheel turns and taps.
//! Nothing here touches the terminal, so `--print` and the tests draw
//! the same frame the user sees. Read `Board::show_reading`,
//! `Board::handle_input` and `Board::render` first.
//!
//! The screen fits a phone first. The top line counts what waits. The
//! list keeps the current section's heading on its first line. A tap on
//! a heading folds or opens it, and a second tap on an item opens its
//! details over the whole screen. The bottom line pages up and down,
//! and its middle opens a menu of sections that ends with the help.

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers, MouseEventKind};
use jiff::Zoned;
use ratatui::Frame;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::layout::{Position, Rect};

use super::draw::{
    BottomTap, ListLayout, draw_details, draw_help, draw_list_screen, draw_menu, help_scroll_limit,
    menu_row_at, top_line,
};
use super::list::{Row, SectionList};
use super::queue::Queue;
use super::sections::{Counts, build_sections, read_failure_item};

/// How many rows one wheel turn scrolls.
const WHEEL_STEP: isize = 3;

/// What the board does after one input.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Step {
    Stay,
    /// Read the hub's state again now.
    Refresh,
    Quit,
}

/// The screen the board shows. Every page but the list fills the whole
/// screen and goes back to the list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Page {
    List,
    /// The chosen row's full text, scrolled down this many lines.
    Details(u16),
    /// The keys and taps, scrolled down this many lines.
    Help(u16),
    /// The menu of sections with this row marked. The row after the
    /// last section opens the help.
    Menu(usize),
}

/// What the screen shows, which row is chosen, and where taps land.
pub(crate) struct Board {
    color: bool,
    counts: Counts,
    list: SectionList,
    page: Page,
    read_at: Option<Zoned>,
    reading: bool,
    /// The screen at the last draw, so a tap finds what it hit.
    area: Rect,
}

impl Board {
    /// Returns a board that has read nothing yet. With `color` false,
    /// headings keep bold and capitals but get no color.
    pub(crate) fn new(color: bool) -> Board {
        Board {
            color,
            counts: Counts::default(),
            list: SectionList::new(),
            page: Page::List,
            read_at: None,
            reading: true,
            area: Rect::default(),
        }
    }

    /// Shows a new read, keeping the chosen row when it is still there
    /// and its place otherwise. A failed read keeps the old rows and
    /// adds the reason under PROBLEMS.
    pub(crate) fn show_reading(&mut self, reading: Result<Queue, String>) {
        self.reading = false;
        match reading {
            Ok(queue) => {
                let sections = build_sections(&queue);
                self.counts = Counts::new(&sections, queue.card_counts.as_ref());
                self.read_at = Some(queue.read_at);
                self.list.show_sections(sections);
            }
            Err(reason) => self.list.add_problem(read_failure_item(&reason)),
        }
        // A read can drop sections, so keep the marked row on the menu.
        if let Page::Menu(marked) = &mut self.page {
            *marked = (*marked).min(self.list.sections().len());
        }
    }

    /// Marks the board as reading, until the next `show_reading`.
    pub(crate) fn note_reading(&mut self) {
        self.reading = true;
    }

    /// Moves the choice, folds a section, opens or closes a page, or
    /// quits. Raw mode turns Ctrl-C into a key, so Ctrl-C quits here, as
    /// q does. Esc only goes back, since a phone user presses it to
    /// leave a screen, not the board.
    pub(crate) fn handle_input(&mut self, input: &Event) -> Step {
        match input {
            Event::Key(key) => self.handle_key(key),
            Event::Mouse(mouse) => {
                match mouse.kind {
                    MouseEventKind::ScrollDown => self.scroll_by(WHEEL_STEP),
                    MouseEventKind::ScrollUp => self.scroll_by(-WHEEL_STEP),
                    MouseEventKind::Down(_) => self.tap(Position::new(mouse.column, mouse.row)),
                    _ => {}
                }
                Step::Stay
            }
            _ => Step::Stay,
        }
    }

    /// Draws the open page, and remembers the screen for taps.
    pub(crate) fn render(&mut self, frame: &mut Frame) {
        self.area = frame.area();
        match self.page {
            Page::List => self.render_list(frame),
            Page::Details(scroll) => draw_details(frame, &self.list, scroll),
            Page::Help(scroll) => draw_help(frame, scroll),
            Page::Menu(marked) => draw_menu(frame, &self.list, marked, self.color),
        }
    }

    /// Returns the frame as text lines, each with its end trimmed.
    pub(crate) fn render_lines(&mut self, columns: u16, rows: u16) -> Vec<String> {
        // A `TestBackend` never fails, so both results are always `Ok`.
        let Ok(mut terminal) = Terminal::new(TestBackend::new(columns, rows));
        let Ok(_) = terminal.draw(|frame| self.render(frame));
        let buffer = terminal.backend().buffer();
        (0..rows)
            .map(|y| {
                let line: String = (0..columns).map(|x| buffer[(x, y)].symbol()).collect();
                line.trim_end().to_owned()
            })
            .collect()
    }

    /// Draws the list screen and places its rows.
    fn render_list(&mut self, frame: &mut Frame) {
        let layout = ListLayout::split(frame.area());
        self.list.place(
            usize::from(layout.list.height),
            usize::from(layout.list.width),
        );
        let when = match (&self.read_at, self.reading) {
            (_, true) => "reading".to_owned(),
            (Some(read_at), false) => read_at.strftime("%H:%M").to_string(),
            (None, false) => String::new(),
        };
        let top = top_line(&self.counts, &when, self.color);
        draw_list_screen(frame, &layout, top, &self.list, self.color);
    }

    /// Quits, asks for a read, or hands the key to the open page.
    fn handle_key(&mut self, key: &KeyEvent) -> Step {
        let ctrl_c =
            key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL);
        if ctrl_c || key.code == KeyCode::Char('q') {
            return Step::Quit;
        }
        if key.code == KeyCode::Char('r') {
            return Step::Refresh;
        }
        match self.page {
            Page::List => self.handle_list_key(key.code),
            Page::Details(_) | Page::Help(_) => self.handle_page_key(key.code),
            Page::Menu(marked) => self.handle_menu_key(key.code, marked),
        }
        Step::Stay
    }

    /// Moves the choice, folds or opens, or opens the menu or the help.
    fn handle_list_key(&mut self, code: KeyCode) {
        let page_size = self.list.page_size();
        match code {
            KeyCode::Char('j') | KeyCode::Down => self.list.move_choice(1),
            KeyCode::Char('k') | KeyCode::Up => self.list.move_choice(-1),
            KeyCode::PageDown => self.list.move_choice(page_size),
            KeyCode::PageUp => self.list.move_choice(-page_size),
            KeyCode::Char('g') | KeyCode::Home => self.list.move_choice(isize::MIN),
            KeyCode::Char('G') | KeyCode::End => self.list.move_choice(isize::MAX),
            KeyCode::Char('n' | ']') => self.list.choose_next_heading(),
            KeyCode::Char('N' | '[') => self.list.choose_previous_heading(),
            KeyCode::Enter | KeyCode::Char('i') => self.open_chosen(),
            KeyCode::Char('s') => self.open_menu(),
            KeyCode::Char('?') => self.page = Page::Help(0),
            _ => {}
        }
    }

    /// Scrolls the details or the help, or goes back to the list.
    fn handle_page_key(&mut self, code: KeyCode) {
        let page_size = self.list.page_size();
        match code {
            KeyCode::Esc | KeyCode::Enter | KeyCode::Char('i' | '?') => self.page = Page::List,
            KeyCode::Char('j') | KeyCode::Down => self.scroll_by(1),
            KeyCode::Char('k') | KeyCode::Up => self.scroll_by(-1),
            KeyCode::PageDown => self.scroll_by(page_size),
            KeyCode::PageUp => self.scroll_by(-page_size),
            _ => {}
        }
    }

    /// Moves the menu's mark, goes to the marked row, opens the help,
    /// or goes back to the list.
    fn handle_menu_key(&mut self, code: KeyCode, marked: usize) {
        let last = self.list.sections().len();
        match code {
            KeyCode::Char('j') | KeyCode::Down => self.page = Page::Menu((marked + 1).min(last)),
            KeyCode::Char('k') | KeyCode::Up => self.page = Page::Menu(marked.saturating_sub(1)),
            KeyCode::Enter => self.open_menu_row(marked),
            KeyCode::Esc | KeyCode::Char('s') => self.page = Page::List,
            KeyCode::Char('?') => self.page = Page::Help(0),
            _ => {}
        }
    }

    /// Acts on a tap. On the details or the help any tap goes back. On
    /// the menu a tap on a row opens it, and any other tap goes back. In
    /// the list a tap on a heading folds or opens it, a tap on an item
    /// chooses it, and a tap on the chosen item opens its details. The
    /// bottom line's thirds page up, open the menu, and page down.
    fn tap(&mut self, at: Position) {
        match self.page {
            Page::List => self.tap_list(at),
            Page::Details(_) | Page::Help(_) => self.page = Page::List,
            Page::Menu(_) => match menu_row_at(self.area, at) {
                Some(row) => self.open_menu_row(row),
                None => self.page = Page::List,
            },
        }
    }

    /// Acts on a tap on the list screen.
    fn tap_list(&mut self, at: Position) {
        let layout = ListLayout::split(self.area);
        if layout.list.contains(at) {
            let Some(index) = self.list.row_on_line(usize::from(at.y - layout.list.y)) else {
                return;
            };
            let was_chosen = self.list.chosen() == Some(index);
            self.list.choose(index);
            match self.list.chosen_row() {
                Some(Row::Heading { .. }) => self.list.toggle_chosen_fold(),
                // Rows move between reads, so the first tap on an item
                // only chooses it and the user sees what a second tap opens.
                Some(Row::Item { .. }) if was_chosen => self.page = Page::Details(0),
                _ => {}
            }
            return;
        }
        let page_size = self.list.page_size();
        match layout.bottom_tap(at) {
            Some(BottomTap::PageUp) => self.list.move_choice(-page_size),
            Some(BottomTap::Menu) => self.open_menu(),
            Some(BottomTap::PageDown) => self.list.move_choice(page_size),
            None => {}
        }
    }

    /// Opens the menu with the chosen row's section marked.
    fn open_menu(&mut self) {
        self.page = Page::Menu(self.list.chosen_section().unwrap_or(0));
    }

    /// Goes to the section on menu row `row`, or opens the help on the
    /// row after the last section. Any other row goes back to the list.
    fn open_menu_row(&mut self, row: usize) {
        let help_row = self.list.sections().len();
        match self.list.sections().get(row).map(|section| section.id) {
            Some(id) => {
                self.list.go_to_section(id);
                self.page = Page::List;
            }
            None if row == help_row => self.page = Page::Help(0),
            None => self.page = Page::List,
        }
    }

    /// Scrolls the details or the help when one is open, else the list.
    fn scroll_by(&mut self, steps: isize) {
        match &mut self.page {
            Page::Details(scroll) => {
                let steps = i16::try_from(steps).unwrap_or(i16::MAX);
                *scroll = scroll.saturating_add_signed(steps);
            }
            Page::Help(scroll) => {
                let steps = i16::try_from(steps).unwrap_or(i16::MAX);
                *scroll = scroll
                    .saturating_add_signed(steps)
                    .min(help_scroll_limit(self.area));
            }
            Page::List => self.list.scroll_by(steps),
            Page::Menu(_) => {}
        }
    }

    /// Folds or opens a chosen heading, or opens a chosen item's
    /// details.
    fn open_chosen(&mut self) {
        match self.list.chosen_row() {
            Some(Row::Heading { .. }) => self.list.toggle_chosen_fold(),
            Some(Row::Item { .. }) => self.page = Page::Details(0),
            None => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crossterm::event::{KeyEventKind, KeyEventState, MouseButton, MouseEvent};

    use jiff::civil::DateTime;

    use crate::board::queue::{CardLine, PullRequest, Question, SessionLine, Urgency};
    use crate::external::cards::{BoardCounts, Stage};
    use crate::shared::lifecycle::{KeptReason, Lifecycle};

    /// A queue with an item in most sections and four cards, read at
    /// `10:42`.
    fn sample_queue() -> Queue {
        let tonight: DateTime = jiff::civil::date(2026, 11, 11).at(20, 0, 0, 0);
        let session = |name: &str, state, why: &str, card: Option<&str>| SessionLine {
            name: name.to_owned(),
            state,
            why: why.to_owned(),
            card: card.map(str::to_owned),
        };
        let card = |key: &str, stage, title: &str, next_step: &str, workers: &[&str]| CardLine {
            key: key.parse().unwrap(),
            stage,
            project: "Demo".to_owned(),
            title: title.to_owned(),
            next_step: next_step.to_owned(),
            workers: workers.iter().map(|name| (*name).to_owned()).collect(),
        };
        let question = |title: &str, urgency| Question {
            title: title.to_owned(),
            lines: vec![format!("{title}?"), "Recommended: A.".to_owned()],
            urgency,
        };
        Queue {
            read_at: "2026-11-11T10:42[UTC]".parse().unwrap(),
            sessions: vec![
                session("docs-fix", Lifecycle::WaitsOnHub, "unread report", None),
                session("blocked-fix", Lifecycle::WaitsOnUser, "blocked", None),
                session(
                    "api-build",
                    Lifecycle::Working,
                    "-",
                    Some("DEMO-KB-012 active"),
                ),
                session(
                    "ship-docs",
                    Lifecycle::Kept(KeptReason::ApprovalWaits),
                    "approval waits",
                    None,
                ),
                session("old-plan", Lifecycle::Parked, "-", None),
            ],
            unread: vec!["docs-fix".to_owned(), "old-worker".to_owned()],
            cards: vec![
                card(
                    "DEMO-KB-010",
                    Stage::Review,
                    "Retry a failed upload twice",
                    "review PR #301",
                    &[],
                ),
                card(
                    "DEMO-KB-012",
                    Stage::Active,
                    "Faster search",
                    "open the card",
                    &["api-build"],
                ),
                card(
                    "DEMO-KB-016",
                    Stage::Active,
                    "Free up disk space",
                    "open the card",
                    &[],
                ),
                card(
                    "DEMO-KB-018",
                    Stage::Draft,
                    "Board for the phone",
                    "open the card",
                    &["docs-fix"],
                ),
            ],
            questions: vec![
                question("Deploy tonight", Urgency::Now),
                question("Demo: name the app", Urgency::Later(tonight)),
                question("Session review picks", Urgency::Optional),
            ],
            pull_requests: vec![PullRequest {
                number: "#301".to_owned(),
                what: "uploads: retry a failed upload twice, then show the error".to_owned(),
                cells: vec!["#301".to_owned(), "uploads: retry".to_owned()],
            }],
            review_notes: vec!["Merge in order 301, 302.".to_owned()],
            card_counts: Some(BoardCounts {
                active: 5,
                review: 4,
                busy: 9,
                limit: 8,
            }),
            problems: Vec::new(),
        }
    }

    /// Returns a key press with no modifier.
    fn press(code: KeyCode) -> Event {
        Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    /// Returns one turn of the wheel toward the list's end.
    fn wheel_down() -> Event {
        Event::Mouse(MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: 10,
            row: 5,
            modifiers: KeyModifiers::NONE,
        })
    }

    /// Returns a tap at screen column `column`, row `row`.
    fn tap_at(column: u16, row: u16) -> Event {
        Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column,
            row,
            modifiers: KeyModifiers::NONE,
        })
    }

    /// Returns a tap on the first screen line that holds `text`.
    fn tap_on(lines: &[String], text: &str) -> Event {
        tap_at(10, screen_row(lines, text))
    }

    /// Returns the first screen line that holds `text`.
    fn screen_row(lines: &[String], text: &str) -> u16 {
        let found = lines.iter().position(|line| line.contains(text));
        let row = found.unwrap_or_else(|| panic!("no line holds {text:?} in {lines:#?}"));
        u16::try_from(row).unwrap()
    }

    /// The user reads the board on a phone, 40 by 16, with no color or
    /// bold. The top line must count what waits, and headings must read
    /// as headings from their rules and capitals. What needs the user
    /// must come first with approvals on top. The cards in review and
    /// the cards worked on come next, each with its key and its step or
    /// worker, so the first screen says what is in review and what is
    /// worked on. A long row wraps rather than losing its end.
    #[test]
    fn render_lines_draws_phone_screen_needs_first() {
        let mut board = Board::new(false);
        board.show_reading(Ok(sample_queue()));

        let lines = board.render_lines(40, 16);

        let expected = [
            "YOU 3+2  PR 1  HUB 2  busy 9/8!  10:42",
            "── NEEDS YOU 3 ─────────────────────────",
            ">A ship-docs  approval waits",
            " B blocked-fix  blocked",
            " ? Deploy tonight",
            "── IN REVIEW 1 ─────────────────────────",
            " V Demo: Retry a failed upload twice",
            "   DEMO-KB-010  review PR #301",
            "── WORKING ON NOW 3 ────────────────────",
            " ! Demo: Board for the phone",
            "   DEMO-KB-018  in draft, by docs-fix",
            " * Demo: Faster search",
            "   DEMO-KB-012  by api-build",
            " - Demo: Free up disk space",
            "   DEMO-KB-016  no worker",
            "^ up           2/23 menu          down v",
        ];
        assert_eq!(lines, expected);
    }

    /// Raw mode turns Ctrl-C into a key, so a board that misses q or
    /// Ctrl-C leaves the user no way out but closing the pane. A phone
    /// user presses Esc to leave a screen, so Esc must close the
    /// details and never drop them to the shell.
    #[test]
    fn handle_input_esc_closes_quit_keys_quit() {
        let mut board = Board::new(false);
        board.show_reading(Ok(sample_queue()));
        board.handle_input(&press(KeyCode::Enter));
        let details = board.render_lines(40, 16);
        let ctrl_c = Event::Key(KeyEvent {
            code: KeyCode::Char('c'),
            modifiers: KeyModifiers::CONTROL,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        });

        let escape_in_details = board.handle_input(&press(KeyCode::Esc));
        let lines = board.render_lines(40, 16);
        let escape_in_list = board.handle_input(&press(KeyCode::Esc));

        assert_eq!(details[0], "NEEDS YOU 3");
        assert_eq!(escape_in_details, Step::Stay);
        assert!(lines[0].starts_with("YOU"), "details still open");
        assert_eq!(escape_in_list, Step::Stay);
        assert_eq!(board.handle_input(&press(KeyCode::Char('q'))), Step::Quit);
        assert_eq!(board.handle_input(&ctrl_c), Step::Quit);
    }

    /// Rows move when the board reads again, so a single tap must only
    /// choose. A second tap on the chosen row opens its whole text over
    /// the screen, and any tap goes back, since a phone has no Esc key
    /// in reach. A tap on any line of a wrapped row counts as a tap on
    /// that row.
    #[test]
    fn handle_input_wrapped_row_taps_toggle_details() {
        let mut board = Board::new(false);
        board.show_reading(Ok(sample_queue()));
        let lines = board.render_lines(40, 16);
        let card_first_line = usize::from(screen_row(&lines, "Retry a failed upload"));
        let tap = tap_on(&lines, "DEMO-KB-010  review");

        board.handle_input(&tap);
        let after_one_tap = board.render_lines(40, 16);
        board.handle_input(&tap);
        let after_two_taps = board.render_lines(40, 16);
        board.handle_input(&tap);
        let after_three_taps = board.render_lines(40, 16);

        let row = card_first_line;
        assert_eq!(after_one_tap[row], ">V Demo: Retry a failed upload twice");
        assert_eq!(after_two_taps[0], "IN REVIEW 1");
        assert_eq!(after_two_taps[1], "DEMO-KB-010 Retry a failed upload twice");
        assert_eq!(after_three_taps[row], after_one_tap[row]);
    }

    /// The board reads again every 10 seconds, and a row's text can
    /// change between reads. A read that moved the choice back to the
    /// top would pull the row away while the user reads it. When the
    /// chosen card is gone, the card now in its place must be chosen
    /// from its first line, or no row shows chosen while Enter still
    /// opens one.
    #[test]
    fn show_reading_keeps_choice_or_its_place() {
        let mut board = Board::new(false);
        board.show_reading(Ok(sample_queue()));
        let lines = board.render_lines(40, 16);
        board.handle_input(&tap_on(&lines, "Faster search"));
        let mut answered = sample_queue();
        answered.questions.remove(0);
        answered.cards[1].title = "Faster search on the docs".to_owned();
        // Dropping a row above and the chosen card puts the old row
        // number on the second line of the next card.
        let mut moved = sample_queue();
        moved.questions.remove(0);
        moved
            .sessions
            .retain(|session| session.name != "blocked-fix");
        moved.cards.remove(1);

        board.show_reading(Ok(answered));
        let kept = board.render_lines(40, 16);
        board.show_reading(Ok(moved));
        let replaced = board.render_lines(40, 16);

        let search = usize::from(screen_row(&kept, "Faster search"));
        assert_eq!(kept[search], ">* Demo: Faster search on the docs");
        let disk = usize::from(screen_row(&replaced, "Free up disk space"));
        assert_eq!(replaced[disk], ">- Demo: Free up disk space");
    }

    /// A phone sends a burst of wheel turns for one flick. If the wheel
    /// moved the choice, a flick would race it far down the list, so
    /// the wheel scrolls the list and the choice stays put. The list
    /// stops at its last row, so a flick never leaves an empty screen.
    #[test]
    fn handle_input_wheel_stops_at_end() {
        let mut board = Board::new(false);
        let mut queue = sample_queue();
        queue.sessions.retain(|session| session.name != "old-plan");
        queue.cards.clear();
        board.show_reading(Ok(queue));
        board.render_lines(40, 11);

        for _ in 0..3 {
            board.handle_input(&wheel_down());
        }
        let lines = board.render_lines(40, 11);

        // The list stops where its last row, the folded sessions, is drawn.
        assert_eq!(lines[1], "── PULL REQUESTS 1 ─────────────────────");
        assert_eq!(lines[8], "+─ WORKING SESSIONS 1 ──────────────────");
        assert_eq!(lines[10], "^ up           2/12 menu          down v");
    }

    /// When the list starts inside a section, that section's heading
    /// must stay on the list's first line. Without it, a user who
    /// scrolled cannot tell which section the rows on screen belong to.
    #[test]
    fn render_lines_pins_heading_of_scrolled_section() {
        let mut board = Board::new(false);
        board.show_reading(Ok(sample_queue()));
        board.render_lines(40, 16);

        board.handle_input(&wheel_down());
        let lines = board.render_lines(40, 16);

        assert_eq!(lines[1], "── NEEDS YOU 3 ─────────────────────────");
        assert_eq!(lines[2], " ? Deploy tonight");
    }

    /// The bottom line is how a phone user moves far. At 40 columns the
    /// side thirds are 13 wide, so column 13 starts the middle third,
    /// which opens the menu of sections, and column 27 starts the right
    /// third, which pages down. A wrong edge sends the user the wrong
    /// way.
    #[test]
    fn handle_input_bottom_thirds_menu_and_page() {
        let mut board = Board::new(false);
        board.show_reading(Ok(sample_queue()));
        board.render_lines(40, 16);

        board.handle_input(&tap_at(13, 15));
        let menu = board.render_lines(40, 16);
        board.handle_input(&press(KeyCode::Esc));
        board.render_lines(40, 16);
        board.handle_input(&tap_at(27, 15));
        let after_page = board.render_lines(40, 16);

        assert_eq!(menu[0], "GO TO A SECTION");
        assert_eq!(after_page[15], "^ up           15/23 menu         down v");
    }

    /// The menu is how a phone user moves between sections and finds
    /// the help without docs. One tap on a folded section must open it
    /// with its items on screen, or the user lands on a folded line and
    /// has to look for the rows. The last row must open the help.
    #[test]
    fn handle_input_menu_opens_section_or_help() {
        let mut board = Board::new(false);
        board.show_reading(Ok(sample_queue()));
        board.render_lines(40, 16);
        board.handle_input(&press(KeyCode::Char('s')));
        let menu = board.render_lines(40, 16);

        board.handle_input(&tap_on(&menu, "LATER"));
        let list = board.render_lines(40, 16);
        board.handle_input(&press(KeyCode::Char('s')));
        board.handle_input(&tap_on(&menu, "? keys and taps"));
        let help = board.render_lines(40, 16);

        let later = usize::from(screen_row(&list, "LATER"));
        assert_eq!(list[later], "── LATER 1 from 20:00 ──────────────────");
        assert_eq!(list[later + 1], " ? 20:00 Demo: name the app");
        assert_eq!(help[0], "KEYS AND TAPS");
    }

    /// The board reads every 10 seconds, and a tap can arrive before the
    /// next draw. After a read with fewer rows, the list must fill the
    /// screen again and a tap must land on the row drawn under it.
    /// Otherwise the user sees a blank screen, chooses a row they never
    /// saw, or the board crashes.
    #[test]
    fn show_reading_shrunk_list_stays_tappable() {
        let mut board = Board::new(false);
        board.show_reading(Ok(sample_queue()));
        board.render_lines(40, 10);
        for _ in 0..3 {
            board.handle_input(&wheel_down());
        }
        board.render_lines(40, 10);
        let mut shrunk = sample_queue();
        shrunk
            .sessions
            .retain(|session| session.name == "blocked-fix");
        shrunk.unread.clear();
        shrunk.cards.clear();

        board.show_reading(Ok(shrunk));
        // The question is drawn on screen row 2 after the read, under
        // the pinned heading.
        board.handle_input(&tap_at(10, 2));
        let lines = board.render_lines(40, 10);

        assert_eq!(lines[1], "── NEEDS YOU 2 ─────────────────────────");
        assert_eq!(lines[2], ">? Deploy tonight");
    }

    /// Side groups start folded, and the user opens one with a tap on
    /// its heading near the bottom of the screen. The heading must move
    /// to the top so its rows show, and a read every 10 seconds must
    /// not fold it again under the user.
    #[test]
    fn handle_input_opened_heading_stays_on_top() {
        let mut board = Board::new(false);
        let mut queue = sample_queue();
        queue.cards.clear();
        board.show_reading(Ok(queue));
        let lines = board.render_lines(40, 10);
        let later_heading = tap_on(&lines, "LATER");

        board.handle_input(&later_heading);
        let mut queue = sample_queue();
        queue.cards.clear();
        board.show_reading(Ok(queue));
        let lines = board.render_lines(40, 10);

        assert_eq!(lines[1], "── LATER 1 from 20:00 ──────────────────");
        assert_eq!(lines[2], " ? 20:00 Demo: name the app");
    }

    /// Card rows wrap onto two lines on a phone. A key press must step
    /// over the second line to the next row, or j sticks on the first
    /// wrapped card and the user cannot reach the rows below by key.
    /// The list must scroll until the chosen card's last line shows, or
    /// its key and next step stay below the screen.
    #[test]
    fn handle_input_j_shows_whole_next_card() {
        let mut board = Board::new(false);
        board.show_reading(Ok(sample_queue()));
        let lines = board.render_lines(40, 9);
        board.handle_input(&tap_on(&lines, "Retry a failed upload"));

        board.handle_input(&press(KeyCode::Char('j')));
        board.handle_input(&press(KeyCode::Char('j')));
        let lines = board.render_lines(40, 9);

        let next_card = usize::from(screen_row(&lines, "Board for the phone"));
        assert_eq!(lines[next_card], ">! Demo: Board for the phone");
        assert_eq!(
            lines[next_card + 1],
            "   DEMO-KB-018  in draft, by docs-fix"
        );
    }

    /// The help is longer than a phone screen, and one flick sends many
    /// wheel turns. The help must stop with its last line at the bottom,
    /// or a flick leaves the user on an empty page.
    #[test]
    fn handle_input_help_scroll_stops_at_end() {
        let mut board = Board::new(false);
        board.show_reading(Ok(sample_queue()));
        board.handle_input(&press(KeyCode::Char('?')));
        board.render_lines(40, 16);

        for _ in 0..10 {
            board.handle_input(&wheel_down());
        }
        let lines = board.render_lines(40, 16);

        assert_eq!(lines[14], " P parked  C safe to close  K kept");
    }
}
