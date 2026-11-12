//! Turns a `Queue` into the board's rows and draws them, and moves the
//! chosen row on keys, wheel turns and taps. Nothing here touches the
//! terminal, so `--print` and the tests draw the same frame the user
//! sees. Read `Board::show_reading`, `Board::handle_input` and
//! `Board::render` first.

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers, MouseEventKind};
use ratatui::Frame;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{
    Block, Borders, HighlightSpacing, List, ListItem, ListState, Paragraph, Wrap,
};

use super::queue::{Queue, SessionLine};
use crate::shared::lifecycle::Lifecycle;

/// The rows under the list that show the chosen row's full text, its
/// top border included.
const DETAIL_HEIGHT: u16 = 4;

/// How many items one wheel turn moves the choice.
const WHEEL_STEP: isize = 3;

/// What the board does after one input.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Step {
    Stay,
    /// Read the hub's state again now.
    Refresh,
    Quit,
}

/// One line of the list. Only items can be chosen.
#[derive(Debug)]
struct Row {
    is_heading: bool,
    text: String,
    /// The full text the detail lines show when the row is chosen.
    detail: String,
}

/// What the screen shows, and which row is chosen.
pub(crate) struct Board {
    top_line: String,
    rows: Vec<Row>,
    list: ListState,
    /// The title of the detail lines, such as `read 10:42`.
    note: String,
    /// Where the list was last drawn, so a tap finds its row.
    list_area: Rect,
}

impl Board {
    /// Returns a board that has read nothing yet.
    pub(crate) fn new() -> Board {
        Board {
            top_line: "hub board".to_owned(),
            rows: Vec::new(),
            list: ListState::default(),
            note: "reading".to_owned(),
            list_area: Rect::default(),
        }
    }

    /// Shows a new read, keeping the chosen row when it is still
    /// there. A failed read keeps the old rows and shows the reason.
    pub(crate) fn show_reading(&mut self, reading: Result<Queue, String>) {
        let queue = match reading {
            Ok(queue) => queue,
            Err(reason) => {
                self.note = format!("read failed: {reason}");
                return;
            }
        };
        let chosen_text = self
            .list
            .selected()
            .and_then(|index| self.rows.get(index))
            .map(|row| row.text.clone());
        self.top_line = count_line(&queue);
        self.rows = build_rows(&queue);
        self.note = format!("read {}  r reads again", queue.read_at);
        let same_row = chosen_text.and_then(|text| {
            self.rows
                .iter()
                .position(|row| !row.is_heading && row.text == text)
        });
        match same_row {
            Some(index) => self.list.select(Some(index)),
            None => {
                self.list.select(None);
                self.move_choice(0);
            }
        }
    }

    /// Marks the board as reading, until the next `show_reading`.
    pub(crate) fn note_reading(&mut self) {
        self.note = "reading".to_owned();
    }

    /// Moves the choice or quits. Raw mode turns Ctrl-C into a key, so
    /// Ctrl-C quits here, as q and Esc do.
    pub(crate) fn handle_input(&mut self, input: &Event) -> Step {
        match input {
            Event::Key(key) => self.handle_key(key),
            Event::Mouse(mouse) => {
                match mouse.kind {
                    MouseEventKind::ScrollDown => self.move_choice(WHEEL_STEP),
                    MouseEventKind::ScrollUp => self.move_choice(-WHEEL_STEP),
                    MouseEventKind::Down(_) => self.choose_at(mouse.column, mouse.row),
                    _ => {}
                }
                Step::Stay
            }
            _ => Step::Stay,
        }
    }

    /// Draws the count line, the list and the chosen row's full text.
    pub(crate) fn render(&mut self, frame: &mut Frame) {
        let [top_area, list_area, detail_area] = split_screen(frame.area());
        self.list_area = list_area;
        frame.render_widget(Line::from(self.top_line.as_str()), top_area);
        let items: Vec<ListItem> = self
            .rows
            .iter()
            .map(|row| {
                let style = if row.is_heading {
                    Style::new().add_modifier(Modifier::BOLD)
                } else {
                    Style::new()
                };
                ListItem::new(Line::styled(row.text.as_str(), style))
            })
            .collect();
        let list = List::new(items)
            .highlight_symbol(">")
            .highlight_spacing(HighlightSpacing::Always)
            .highlight_style(Style::new().add_modifier(Modifier::REVERSED));
        frame.render_stateful_widget(list, list_area, &mut self.list);
        let detail = self
            .list
            .selected()
            .and_then(|index| self.rows.get(index))
            .map_or("Nothing to show.", |row| row.detail.as_str());
        let detail_block = Block::new().borders(Borders::TOP).title(self.note.as_str());
        frame.render_widget(
            Paragraph::new(detail)
                .wrap(Wrap { trim: true })
                .block(detail_block),
            detail_area,
        );
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

    /// Quits, asks for a read, or moves the choice.
    fn handle_key(&mut self, key: &KeyEvent) -> Step {
        let ctrl_c =
            key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL);
        if ctrl_c || key.code == KeyCode::Char('q') || key.code == KeyCode::Esc {
            return Step::Quit;
        }
        let page = isize::try_from(self.list_area.height).unwrap_or(1).max(1);
        match key.code {
            KeyCode::Char('r') => return Step::Refresh,
            KeyCode::Char('j') | KeyCode::Down => self.move_choice(1),
            KeyCode::Char('k') | KeyCode::Up => self.move_choice(-1),
            KeyCode::PageDown => self.move_choice(page),
            KeyCode::PageUp => self.move_choice(-page),
            _ => {}
        }
        Step::Stay
    }

    /// Moves the choice `steps` items down, or up when negative,
    /// skipping headings and stopping at the first and last item.
    fn move_choice(&mut self, steps: isize) {
        let items: Vec<usize> = (0..self.rows.len())
            .filter(|&index| !self.rows[index].is_heading)
            .collect();
        let Some(last) = items.len().checked_sub(1) else {
            self.list.select(None);
            return;
        };
        let current = self
            .list
            .selected()
            .and_then(|chosen| items.iter().position(|&index| index == chosen))
            .unwrap_or(0);
        let target = current.saturating_add_signed(steps).min(last);
        self.list.select(Some(items[target]));
        // The first item's heading scrolls back into view with it.
        if target == 0 {
            *self.list.offset_mut() = 0;
        }
    }

    /// Chooses the item drawn at screen position `column`, `row`. A tap
    /// outside the list or on a heading changes nothing.
    fn choose_at(&mut self, column: u16, row: u16) {
        let area = self.list_area;
        let inside =
            (area.x..area.right()).contains(&column) && (area.y..area.bottom()).contains(&row);
        if !inside {
            return;
        }
        let index = self.list.offset() + usize::from(row - area.y);
        if self.rows.get(index).is_some_and(|row| !row.is_heading) {
            self.list.select(Some(index));
        }
    }
}

/// Returns the count line, the list's area and the detail area.
fn split_screen(area: Rect) -> [Rect; 3] {
    Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(1),
        Constraint::Length(DETAIL_HEIGHT),
    ])
    .areas(area)
}

/// Returns the line of counts, such as `you 3  prs 6  hub 2  cards 5/8`.
/// The Working section shows its own count, so the line leaves it out
/// to fit 40 columns.
fn count_line(queue: &Queue) -> String {
    let count_state = |state: Lifecycle| {
        queue
            .sessions
            .iter()
            .filter(|session| session.state == state)
            .count()
    };
    let you = queue.questions.len() + count_state(Lifecycle::WaitsOnUser);
    let hub = count_state(Lifecycle::WaitsOnHub) + unread_not_listed_for_hub(queue).count();
    let mut line = format!("you {you}  prs {}  hub {hub}", queue.reviews.len());
    if let Some(cards) = &queue.card_counts {
        line.push_str(&format!("  cards {cards}"));
    }
    line
}

/// Returns the list in its sections, what needs the user first. A
/// section with nothing in it is left out.
fn build_rows(queue: &Queue) -> Vec<Row> {
    let sessions_in = |wanted: fn(Lifecycle) -> bool| -> Vec<Row> {
        queue
            .sessions
            .iter()
            .filter(|session| wanted(session.state))
            .map(session_row)
            .collect()
    };

    let mut for_you: Vec<Row> = queue
        .questions
        .iter()
        .map(|question| Row {
            is_heading: false,
            text: format!("? {}", question.title),
            detail: format!("{}: {}", question.title, question.text),
        })
        .collect();
    for_you.extend(sessions_in(|state| state == Lifecycle::WaitsOnUser));
    let reviews = queue
        .reviews
        .iter()
        .map(|review| Row {
            is_heading: false,
            text: review.clone(),
            detail: review.clone(),
        })
        .collect();
    let mut for_hub = sessions_in(|state| state == Lifecycle::WaitsOnHub);
    for_hub.extend(unread_not_listed_for_hub(queue).map(|name| Row {
        is_heading: false,
        text: format!("report from {name}"),
        detail: format!("{name} wrote a report the hub has not read."),
    }));
    let cards = queue
        .cards_without_session
        .iter()
        .map(|card| Row {
            is_heading: false,
            text: card.clone(),
            detail: format!("Card {card} has no session."),
        })
        .collect();
    let problems = queue
        .problems
        .iter()
        .map(|problem| Row {
            is_heading: false,
            text: problem.clone(),
            detail: problem.clone(),
        })
        .collect();

    let sections = [
        ("Needs you", for_you),
        ("Pull requests to review", reviews),
        ("Waiting on the hub", for_hub),
        ("Cards with no session", cards),
        ("Working", sessions_in(|state| state == Lifecycle::Working)),
        (
            "Parked",
            sessions_in(|state| matches!(state, Lifecycle::ToPark | Lifecycle::Parked)),
        ),
        (
            "Safe to close",
            sessions_in(|state| state == Lifecycle::ToClose),
        ),
        (
            "Kept",
            sessions_in(|state| matches!(state, Lifecycle::Kept(_))),
        ),
        ("Could not read", problems),
    ];
    let mut rows = Vec::new();
    for (title, items) in sections {
        if items.is_empty() {
            continue;
        }
        rows.push(Row {
            is_heading: true,
            text: format!("{title} ({})", items.len()),
            detail: title.to_owned(),
        });
        rows.extend(items);
    }
    rows
}

/// Returns the names with an unread report whose session is not
/// already under "Waiting on the hub", such as a closed worker's. An
/// unread report makes an idle session wait on the hub, so its row
/// already says so.
fn unread_not_listed_for_hub(queue: &Queue) -> impl Iterator<Item = &String> {
    queue.unread.iter().filter(|name| {
        !queue
            .sessions
            .iter()
            .any(|session| &session.name == *name && session.state == Lifecycle::WaitsOnHub)
    })
}

/// Returns a session's row, such as `docs-fix  unread report`. A why
/// of `-` says nothing, so the row shows the name alone.
fn session_row(session: &SessionLine) -> Row {
    let why = (session.why != "-").then_some(session.why.as_str());
    let card = session
        .card
        .as_ref()
        .map_or(String::new(), |card| format!(", card {card}"));
    let state = session.state.word();
    Row {
        is_heading: false,
        text: match why {
            Some(why) => format!("{}  {why}", session.name),
            None => session.name.clone(),
        },
        detail: match why {
            Some(why) => format!("{} is {state}: {why}{card}", session.name),
            None => format!("{} is {state}{card}", session.name),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crossterm::event::{KeyEventKind, KeyEventState, MouseButton, MouseEvent};

    use crate::board::queue::Question;

    /// A queue with one or two items in each section the user acts on.
    fn sample_queue() -> Queue {
        let session = |name: &str, state, why: &str, card: Option<&str>| SessionLine {
            name: name.to_owned(),
            state,
            why: why.to_owned(),
            card: card.map(str::to_owned),
        };
        Queue {
            read_at: "10:42".to_owned(),
            sessions: vec![
                session("docs-fix", Lifecycle::WaitsOnHub, "unread report", None),
                session("blocked-fix", Lifecycle::WaitsOnUser, "needs you", None),
                session(
                    "api-build",
                    Lifecycle::Working,
                    "-",
                    Some("DEMO-KB-012 active"),
                ),
            ],
            unread: vec!["docs-fix".to_owned(), "old-worker".to_owned()],
            cards_without_session: vec!["DEMO-KB-016 active".to_owned()],
            questions: vec![Question {
                title: "Deploy tonight?".to_owned(),
                text: "Deploy the new model tonight?".to_owned(),
            }],
            reviews: vec![
                "#301  uploads: retry a failed upload twice, then show the error".to_owned(),
            ],
            card_counts: Some("5/8".to_owned()),
            problems: Vec::new(),
        }
    }

    /// The user reads the board on a phone, 40 by 16. What needs them
    /// must come first, each section must show with its count, and a
    /// session must show once, or an item that needs them goes unseen.
    #[test]
    fn render_lines_orders_sections_lists_sessions_once() {
        let mut board = Board::new();
        board.show_reading(Ok(sample_queue()));

        let lines = board.render_lines(40, 16);

        let expected = [
            "you 2  prs 1  hub 2  cards 5/8",
            " Needs you (2)",
            ">? Deploy tonight?",
            " blocked-fix  needs you",
            " Pull requests to review (1)",
            " #301  uploads: retry a failed upload tw",
            " Waiting on the hub (2)",
            " docs-fix  unread report",
            " report from old-worker",
            " Cards with no session (1)",
            " DEMO-KB-016 active",
            " Working (1)",
            "read 10:42  r reads again───────────────",
            "Deploy tonight?: Deploy the new model",
            "tonight?",
            "",
        ];
        assert_eq!(lines, expected);
    }

    /// Raw mode turns Ctrl-C into a key, so a board that misses one
    /// of these keys leaves the user no way out but closing the pane.
    #[test]
    fn handle_input_quits_on_exit_keys() {
        let cases = [
            ("q", KeyCode::Char('q'), KeyModifiers::NONE),
            ("escape", KeyCode::Esc, KeyModifiers::NONE),
            ("ctrl-c", KeyCode::Char('c'), KeyModifiers::CONTROL),
        ];

        for (case_name, code, modifiers) in cases {
            let mut board = Board::new();
            let key = KeyEvent {
                code,
                modifiers,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            };

            let step = board.handle_input(&Event::Key(key));

            assert_eq!(step, Step::Quit, "{case_name}");
        }
    }

    /// On a phone the user taps a row instead of using arrow keys,
    /// often after scrolling. A tap must choose the row drawn under the
    /// finger, and the list cuts long rows, so its full text must show
    /// in the detail lines below.
    #[test]
    fn handle_input_tap_shows_row_in_full() {
        let mut board = Board::new();
        board.show_reading(Ok(sample_queue()));
        board.render_lines(40, 16);
        // Choose the last item, so the next draw scrolls the list by one.
        let page_down = KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE);
        board.handle_input(&Event::Key(page_down));
        board.render_lines(40, 16);
        // The pull request's row is now drawn on screen row 4.
        let tap = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 10,
            row: 4,
            modifiers: KeyModifiers::NONE,
        };

        board.handle_input(&Event::Mouse(tap));
        let lines = board.render_lines(40, 16);

        assert_eq!(lines[4], ">#301  uploads: retry a failed upload tw");
        assert!(
            lines[13..]
                .join(" ")
                .trim_end()
                .ends_with("then show the error")
        );
    }

    /// The board reads again every 10 seconds. A read that moved the
    /// choice back to the top would pull the row away while the user
    /// reads it.
    #[test]
    fn show_reading_keeps_the_chosen_row() {
        let mut board = Board::new();
        board.show_reading(Ok(sample_queue()));
        let down = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE);
        board.handle_input(&Event::Key(down));
        board.handle_input(&Event::Key(down));
        let answered = Queue {
            questions: Vec::new(),
            ..sample_queue()
        };

        board.show_reading(Ok(answered));
        let lines = board.render_lines(40, 16);

        assert_eq!(lines[4], ">#301  uploads: retry a failed upload tw");
    }
}
