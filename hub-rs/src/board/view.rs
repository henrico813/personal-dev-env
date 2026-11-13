//! Keeps what the board shows and changes it on keys, wheel turns and
//! taps. Nothing here touches the terminal, so `--print`, `--dump`,
//! `--control` and the tests draw the same frame the user sees. Read
//! `Board::show_reading`, `Board::handle_input` and `Board::render`
//! first.

use std::collections::BTreeSet;

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers, MouseEventKind};
use ratatui::Frame;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::widgets::ListState;

use super::draw::{self, ListScreen};
use super::elements::{self, Action, Element, ScreenDump};
use super::groups::{self, Group, Item, Target};
use super::queue::Queue;
use crate::external::cards::{CardKey, CardNote};

/// How many items an open group shows before its `+ N more` line.
const GROUP_LIMIT: usize = 4;

/// How many lines one wheel turn moves.
const WHEEL_STEP: isize = 3;

/// What the board does after one input.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Step {
    Stay,
    /// Read the hub's state again now.
    Refresh,
    /// Read this card's note for the open details page.
    ReadCard(CardKey),
    Quit,
}

/// One line of the list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Line {
    Heading {
        group: Group,
        is_open: bool,
    },
    /// The item at `index` in its group's items.
    Item {
        group: Group,
        index: usize,
    },
    /// The `+ N more` line under an open group's first items.
    More {
        group: Group,
        hidden: usize,
    },
    /// The line under an open Needs you when no hub runs.
    NoHub,
}

/// What names a line across reads, since a read can move an item to
/// another place in its group.
#[derive(PartialEq, Eq)]
enum LineId {
    Heading(Group),
    More(Group),
    /// The item's `Item::id`, such as a card key.
    Item(Group, String),
    NoHub,
}

/// The page the screen shows.
enum Page {
    List,
    Details(Details),
}

/// A full-screen page about one item.
pub(super) struct Details {
    /// The page's lines, not yet wrapped.
    pub lines: Vec<String>,
    /// How many wrapped lines are scrolled off the top.
    pub scroll: usize,
    /// The card whose note is being read for this page.
    reading: Option<CardKey>,
}

/// What the screen shows, and which line is chosen.
pub(crate) struct Board {
    queue: Option<Queue>,
    groups: Vec<(Group, Vec<Item>)>,
    open: BTreeSet<Group>,
    /// Open groups that show every item, past `GROUP_LIMIT`.
    shown_in_full: BTreeSet<Group>,
    lines: Vec<Line>,
    list: ListState,
    page: Page,
    /// The bottom line's end, such as `read 10:42 0.6s`.
    status: String,
    /// The last drawn screen, so a tap finds what is under it.
    area: Rect,
    color: bool,
}

impl Board {
    /// Returns a board that has read nothing yet. `color` false draws
    /// every line in the terminal's own colors.
    pub(crate) fn new(color: bool) -> Board {
        Board {
            queue: None,
            groups: Vec::new(),
            // Ready and Draft hold most cards, so they start folded.
            open: BTreeSet::from([Group::InProgress, Group::InReview, Group::NeedsYou]),
            shown_in_full: BTreeSet::new(),
            lines: Vec::new(),
            list: ListState::default(),
            page: Page::List,
            status: "reading".to_owned(),
            area: Rect::default(),
            color,
        }
    }

    /// Shows a new read, keeping the chosen line when it is still
    /// there, else its place. A failed read keeps the old lines and
    /// shows the reason.
    pub(crate) fn show_reading(&mut self, reading: Result<Queue, String>) {
        let queue = match reading {
            Ok(queue) => queue,
            Err(reason) => {
                self.status = format!("read failed: {reason}");
                return;
            }
        };
        let chosen = self.list.selected();
        let chosen_id = chosen.and_then(|index| self.line_id(index));
        self.status = format!("read {} {:.1}s", queue.read_at, queue.took.as_secs_f32());
        self.groups = groups::build_groups(&queue);
        self.queue = Some(queue);
        self.rebuild_lines();
        let same_line = chosen_id.and_then(|id| {
            (0..self.lines.len()).find(|&index| self.line_id(index).as_ref() == Some(&id))
        });
        let last = self.lines.len().saturating_sub(1);
        let place = same_line.unwrap_or_else(|| chosen.unwrap_or(0).min(last));
        self.list.select((!self.lines.is_empty()).then_some(place));
    }

    /// Marks the board as reading, until the next `show_reading`.
    pub(crate) fn note_reading(&mut self) {
        self.status = "reading".to_owned();
    }

    /// Fills the open details page with the card note it asked for. A
    /// note for a page the user already left is dropped.
    pub(crate) fn show_card_note(&mut self, key: &CardKey, note: Result<CardNote, String>) {
        let Page::Details(details) = &mut self.page else {
            return;
        };
        if details.reading.as_ref() != Some(key) {
            return;
        }
        details.reading = None;
        let card = self
            .queue
            .iter()
            .flat_map(|queue| &queue.cards)
            .find(|card| card.key == *key);
        if let Some(card) = card {
            details.lines = groups::describe_card(card, note.as_ref().ok());
        }
        if let Err(reason) = note {
            details
                .lines
                .push(format!("could not read the card: {reason}"));
        }
    }

    /// Moves the choice, folds groups, opens details or quits. Raw mode
    /// turns Ctrl-C into a key, so Ctrl-C quits here, as q does on the
    /// list.
    pub(crate) fn handle_input(&mut self, input: &Event) -> Step {
        match input {
            Event::Key(key) => self.handle_key(key),
            Event::Mouse(mouse) => match mouse.kind {
                MouseEventKind::ScrollDown => self.scroll(WHEEL_STEP),
                MouseEventKind::ScrollUp => self.scroll(-WHEEL_STEP),
                MouseEventKind::Down(_) => self.tap(mouse.column, mouse.row),
                _ => Step::Stay,
            },
            _ => Step::Stay,
        }
    }

    /// Draws the open page over the whole frame.
    pub(crate) fn render(&mut self, frame: &mut Frame) {
        self.area = frame.area();
        match &mut self.page {
            Page::List => {
                // Built field by field, since `self.list` is borrowed
                // mutably beside it.
                let screen = ListScreen {
                    groups: &self.groups,
                    lines: &self.lines,
                    status: &self.status,
                    color: self.color,
                };
                draw::draw_list(frame, &screen, &mut self.list);
            }
            Page::Details(details) => draw::draw_details(frame, details),
        }
    }

    /// Returns every element the open page holds, top first, as the
    /// last draw left it. Those the draw left out are hidden.
    pub(crate) fn elements(&self) -> Vec<Element> {
        match &self.page {
            Page::List => draw::list_elements(&self.list_screen(), self.area, &self.list),
            Page::Details(details) => draw::details_elements(details, self.area),
        }
    }

    /// Draws the open page at `columns` by `rows` and returns it as
    /// elements and text, after the `--control` or `--send` line `step`.
    pub(crate) fn dump(&mut self, columns: u16, rows: u16, step: Option<String>) -> ScreenDump {
        let lines = self.render_lines(columns, rows);
        let page = match self.page {
            Page::List => "list",
            Page::Details(_) => "details",
        };
        ScreenDump {
            step,
            page,
            elements: self.elements(),
            lines,
        }
    }

    /// Returns the frame at the size last drawn, as `dump` does, so an
    /// agent driving the live board reads the screen the user sees.
    pub(crate) fn dump_drawn(&mut self, step: Option<String>) -> ScreenDump {
        self.dump(self.area.width, self.area.height, step)
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

    /// Quits, asks for a read, moves the choice or acts on it.
    fn handle_key(&mut self, key: &KeyEvent) -> Step {
        let ctrl_c =
            key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL);
        if ctrl_c {
            return Step::Quit;
        }
        if key.code == KeyCode::Char('r') {
            return Step::Refresh;
        }
        let page_size = isize::try_from(self.area.height.saturating_sub(2))
            .unwrap_or(1)
            .max(1);
        if let Page::Details(_) = self.page {
            return match key.code {
                KeyCode::Char('q' | ' ') | KeyCode::Esc | KeyCode::Enter => {
                    self.page = Page::List;
                    Step::Stay
                }
                KeyCode::Char('j') | KeyCode::Down => self.scroll(1),
                KeyCode::Char('k') | KeyCode::Up => self.scroll(-1),
                KeyCode::PageDown => self.scroll(page_size),
                KeyCode::PageUp => self.scroll(-page_size),
                _ => Step::Stay,
            };
        }
        match key.code {
            KeyCode::Char('q') => return Step::Quit,
            KeyCode::Char('j') | KeyCode::Down => self.move_choice(1),
            KeyCode::Char('k') | KeyCode::Up => self.move_choice(-1),
            KeyCode::PageDown => self.move_choice(page_size),
            KeyCode::PageUp => self.move_choice(-page_size),
            KeyCode::Enter | KeyCode::Char(' ') => {
                if let Some(index) = self.list.selected() {
                    return self.activate(index);
                }
            }
            _ => {}
        }
        Step::Stay
    }

    /// Acts on the element under a tap, so a count jumps to its group
    /// and a list line acts as Enter does. Any tap on the details page
    /// goes back, even on an empty row.
    fn tap(&mut self, column: u16, row: u16) -> Step {
        if let Page::Details(_) = self.page {
            self.page = Page::List;
            return Step::Stay;
        }
        let elements = self.elements();
        let action = elements::element_at(&elements, column, row).map(|element| element.action);
        match action {
            Some(Action::Jump(group)) => {
                self.jump_to(group);
                Step::Stay
            }
            Some(Action::Line(index)) => self.activate(index),
            Some(Action::Back | Action::None) | None => Step::Stay,
        }
    }

    /// Chooses the line at `index` and acts on it. A heading folds or
    /// opens, `+ N more` shows the rest, and an item opens its details.
    fn activate(&mut self, index: usize) -> Step {
        self.list.select(Some(index));
        match self.lines[index] {
            Line::Heading { group, .. } => {
                if !self.open.remove(&group) {
                    self.open.insert(group);
                }
                self.rebuild_lines();
                Step::Stay
            }
            Line::More { group, .. } => {
                self.shown_in_full.insert(group);
                self.rebuild_lines();
                Step::Stay
            }
            Line::Item { group, index } => self.open_details(group, index),
            Line::NoHub => Step::Stay,
        }
    }

    /// Opens an item's details page. A card the read did not get from
    /// `card show` shows what `card list` gave, and asks for its note.
    fn open_details(&mut self, group: Group, index: usize) -> Step {
        let (Some(queue), Some(item)) = (&self.queue, self.item(group, index)) else {
            return Step::Stay;
        };
        let card = match item.target {
            Target::Card(at) => Some(&queue.cards[at].key),
            _ => None,
        };
        let note = card.and_then(|key| queue.notes.get(key));
        let mut lines = groups::describe_item(queue, item.target, note);
        let reading = card.filter(|_| note.is_none()).cloned();
        if reading.is_some() {
            lines.push(String::new());
            lines.push("reading the card".to_owned());
        }
        let step = reading.clone().map_or(Step::Stay, Step::ReadCard);
        self.page = Page::Details(Details {
            lines,
            scroll: 0,
            reading,
        });
        step
    }

    /// Opens `group`, chooses its heading and puts it on the list's
    /// first line.
    fn jump_to(&mut self, group: Group) {
        self.open.insert(group);
        self.rebuild_lines();
        let is_heading =
            |line: &Line| matches!(line, Line::Heading { group: each, .. } if *each == group);
        if let Some(index) = self.lines.iter().position(is_heading) {
            self.list.select(Some(index));
            *self.list.offset_mut() = index;
        }
    }

    /// Moves the choice `steps` lines down, or up when negative,
    /// stopping at the first and last line.
    fn move_choice(&mut self, steps: isize) {
        let Some(last) = self.lines.len().checked_sub(1) else {
            return;
        };
        let current = self.list.selected().unwrap_or(0);
        self.list
            .select(Some(current.saturating_add_signed(steps).min(last)));
    }

    /// Moves the choice on the list, or scrolls the details page.
    fn scroll(&mut self, steps: isize) -> Step {
        match &mut self.page {
            Page::List => self.move_choice(steps),
            // `draw_details` stops the scroll at the page's end.
            Page::Details(details) => {
                details.scroll = details.scroll.saturating_add_signed(steps);
            }
        }
        Step::Stay
    }

    /// Lists each group's heading, then its first `GROUP_LIMIT` items
    /// and a `+ N more` line when it is open. An open Needs you says
    /// that no hub runs when none did at the read.
    fn rebuild_lines(&mut self) {
        let no_hub = self.queue.as_ref().is_some_and(|queue| queue.no_hub);
        let mut lines = Vec::new();
        for (group, items) in &self.groups {
            let is_open = self.open.contains(group);
            lines.push(Line::Heading {
                group: *group,
                is_open,
            });
            if !is_open {
                continue;
            }
            if *group == Group::NeedsYou && no_hub {
                lines.push(Line::NoHub);
            }
            let shown = if self.shown_in_full.contains(group) {
                items.len()
            } else {
                items.len().min(GROUP_LIMIT)
            };
            lines.extend((0..shown).map(|index| Line::Item {
                group: *group,
                index,
            }));
            if shown < items.len() {
                lines.push(Line::More {
                    group: *group,
                    hidden: items.len() - shown,
                });
            }
        }
        self.lines = lines;
    }

    /// Returns what the list page draws from.
    fn list_screen(&self) -> ListScreen<'_> {
        ListScreen {
            groups: &self.groups,
            lines: &self.lines,
            status: &self.status,
            color: self.color,
        }
    }

    /// Returns the item at `index` in `group`.
    fn item(&self, group: Group, index: usize) -> Option<&Item> {
        self.groups
            .iter()
            .find(|(each, _)| *each == group)
            .and_then(|(_, items)| items.get(index))
    }

    /// Returns what names the line at `index` across reads, or `None`
    /// past the end.
    fn line_id(&self, index: usize) -> Option<LineId> {
        match *self.lines.get(index)? {
            Line::Heading { group, .. } => Some(LineId::Heading(group)),
            Line::More { group, .. } => Some(LineId::More(group)),
            Line::Item { group, index } => {
                Some(LineId::Item(group, self.item(group, index)?.id.clone()))
            }
            Line::NoHub => Some(LineId::NoHub),
        }
    }
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;

    use std::time::Duration;

    use crossterm::event::{MouseButton, MouseEvent};

    use crate::board::queue::{CardLine, Question, SessionLine};
    use crate::external::cards::Stage;

    /// A queue with five cards in work, one in review, a question, a
    /// session that waits on the user, and one ready and one draft card.
    fn sample_queue() -> Queue {
        let card = |key: &str, stage, title: &str, workers: &[&str]| CardLine {
            key: key.parse().unwrap(),
            stage,
            project: "Demo".to_owned(),
            title: title.to_owned(),
            workers: workers.iter().map(|name| (*name).to_owned()).collect(),
        };
        let reviewed_note = CardNote {
            title: "Retry a failed upload twice, then show the error".to_owned(),
            project: "Demo".to_owned(),
            prs: vec!["https://github.com/demo/app/pull/301".to_owned()],
            next_step: Some("merge after the checks pass".to_owned()),
            criteria: vec!["- [ ] A failed upload retries.".to_owned()],
            ..CardNote::default()
        };
        Queue {
            read_at: "10:42".to_owned(),
            took: Duration::from_millis(600),
            cards: vec![
                card(
                    "DEMO-KB-001",
                    Stage::Active,
                    "Faster search",
                    &["api-build"],
                ),
                card("DEMO-KB-002", Stage::Active, "Board for the phone", &[]),
                card("DEMO-KB-003", Stage::Active, "Trim the build cache", &[]),
                card(
                    "DEMO-KB-004",
                    Stage::Active,
                    "Lint every Dockerfile in the repository",
                    &[],
                ),
                card("DEMO-KB-005", Stage::Active, "Rotate the backup keys", &[]),
                card(
                    "DEMO-KB-010",
                    Stage::Review,
                    "Retry a failed upload twice, then show the error",
                    &["docs-fix"],
                ),
                card("DEMO-KB-020", Stage::Ready, "Sort ready cards", &[]),
                card("OPS-KB-001", Stage::Draft, "Watch the disks", &[]),
            ],
            questions: vec![Question {
                hub: "kanban-hub-1".to_owned(),
                title: "Deploy tonight?".to_owned(),
                lines: vec![
                    "Deploy the new model tonight?".to_owned(),
                    "- yes".to_owned(),
                    "- no".to_owned(),
                    "Recommended: yes".to_owned(),
                ],
            }],
            waiting: vec![SessionLine {
                name: "blocked-fix".to_owned(),
                why: "needs you".to_owned(),
                card: None,
            }],
            no_hub: false,
            notes: [("DEMO-KB-010".parse().unwrap(), reviewed_note)].into(),
            problems: Vec::new(),
        }
    }

    /// Returns a board that shows `sample_queue`, drawn once at 40x16.
    pub(crate) fn sample_board() -> Board {
        let mut board = Board::new(false);
        board.show_reading(Ok(sample_queue()));
        board.render_lines(40, 16);
        board
    }

    /// A key press with no modifier.
    fn press(code: KeyCode) -> Event {
        Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    /// A tap on the first character of `text` on the screen.
    fn tap_on(lines: &[String], text: &str) -> Event {
        let row = lines.iter().position(|line| line.contains(text)).unwrap();
        let before = &lines[row][..lines[row].find(text).unwrap()];
        Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: u16::try_from(before.chars().count()).unwrap(),
            row: u16::try_from(row).unwrap(),
            modifiers: KeyModifiers::NONE,
        })
    }

    /// The user reads the board on a phone, 40 by 16. Cards in work, in
    /// review and what needs the user must show on the first screen, a
    /// few rows each, while Ready and Draft stay folded. A row cut
    /// without `~`, or a group out of order, hides what the user looks
    /// for.
    #[test]
    fn render_lines_groups_folds_and_cuts_rows() {
        let mut board = Board::new(false);
        board.show_reading(Ok(sample_queue()));

        let lines = board.render_lines(40, 16);

        let expected = [
            "◐5 progress  ◕1 review  ?2 you",
            "v In progress 5",
            " ◐ Faster search                  Demo",
            " ◐ Board for the phone            Demo",
            " ◐ Trim the build cache           Demo",
            " ◐ Lint every Dockerfile in the ~ Demo",
            "   + 1 more",
            "v In review 1",
            " ◕ Retry a failed upload twice, ~ Demo",
            "v Needs you 2",
            " ? Deploy tonight?                kanban",
            " ? blocked-fix  needs you         worker",
            "> Ready 1",
            "> Draft 1",
            "",
            "q quit  tap open  read 10:42 0.6s",
        ];
        assert_eq!(lines, expected);
    }

    /// A tap is the user's main input on a phone. A tap on a heading
    /// must fold or open its group, and a tap on `+ N more` must show
    /// the rest, or the cards past the first four cannot be reached.
    #[test]
    fn handle_input_taps_fold_and_show_more() {
        let mut board = sample_board();
        let lines = board.render_lines(40, 16);

        board.handle_input(&tap_on(&lines, "v In progress"));
        let folded = board.render_lines(40, 16);
        board.handle_input(&tap_on(&folded, "> In progress"));
        let opened = board.render_lines(40, 16);
        board.handle_input(&tap_on(&opened, "+ 1 more"));
        let in_full = board.render_lines(40, 16);

        assert_eq!(folded[1..3], ["> In progress 5", "v In review 1"]);
        assert_eq!(opened, lines);
        assert_eq!(in_full[6], " ◐ Rotate the backup keys         Demo");
        assert_eq!(in_full[7], "v In review 1");
    }

    /// Each count on the top line jumps to its group. With many cards
    /// in work, what needs the user starts below the first screen, so
    /// the tap must scroll its group into view, or the user never sees
    /// what waits on them.
    #[test]
    fn handle_input_count_tap_shows_its_group() {
        let mut board = Board::new(false);
        board.show_reading(Ok(sample_queue()));
        let lines = board.render_lines(40, 8);

        board.handle_input(&tap_on(&lines, "?2 you"));
        let jumped = board.render_lines(40, 8);

        assert!(!lines.iter().any(|line| line.contains("Needs you")));
        assert!(jumped.iter().any(|line| line == "v Needs you 2"));
        assert!(jumped.iter().any(|line| line.contains("blocked-fix")));
    }

    /// A tap on a scrolled list must open the row drawn under the
    /// finger. If the tap ignored the scroll, the user would open a
    /// different card than the one they tapped.
    #[test]
    fn handle_input_scrolled_tap_opens_drawn_row() {
        let mut board = Board::new(false);
        board.show_reading(Ok(sample_queue()));
        let lines = board.render_lines(40, 8);
        board.handle_input(&tap_on(&lines, "?2 you"));
        let scrolled = board.render_lines(40, 8);

        board.handle_input(&tap_on(&scrolled, "blocked-fix"));
        let details = board.render_lines(40, 8);

        assert_eq!(details[1], "blocked-fix waits on you: needs you");
    }

    /// The row cuts a card to one line, so its details page is where
    /// the user sees who works on it, what to do next and its pull
    /// request. The read already holds this card's note, so opening the
    /// page must not run `card show` again and make the user wait.
    #[test]
    fn handle_input_tap_shows_details_without_reading() {
        let mut board = sample_board();
        let lines = board.render_lines(40, 16);

        let step = board.handle_input(&tap_on(&lines, "Retry a failed"));
        let details = board.render_lines(40, 16);

        assert_eq!(step, Step::Stay);
        let expected = [
            "< back",
            "Retry a failed upload twice, then show",
            "the error",
            "",
            "DEMO-KB-010  review  Demo",
            "by docs-fix",
            "next: merge after the checks pass",
            "https://github.com/demo/app/pull/301",
            "",
            "- [ ] A failed upload retries.",
        ];
        assert_eq!(details[..10], expected);
    }

    /// The read runs `card show` only for cards a session, a parked
    /// record or a report names, to stay fast. Any other card's page
    /// must ask for its note and then show it, or ready and draft cards
    /// never show their criteria.
    #[test]
    fn handle_input_unread_card_loads_its_note() {
        let mut board = sample_board();
        let lines = board.render_lines(40, 16);
        let key: CardKey = "DEMO-KB-001".parse().unwrap();
        let note = CardNote {
            title: "Faster search".to_owned(),
            project: "Demo".to_owned(),
            criteria: vec!["- Search takes under a second.".to_owned()],
            ..CardNote::default()
        };

        let step = board.handle_input(&tap_on(&lines, "Faster search"));
        let reading = board.render_lines(40, 16);
        board.show_card_note(&key, Ok(note));
        let filled = board.render_lines(40, 16);

        assert_eq!(step, Step::ReadCard(key));
        assert_eq!(reading[6], "reading the card");
        assert_eq!(
            filled[4..7],
            ["by api-build", "", "- Search takes under a second.",]
        );
    }

    /// `hub board --dump` and `--control` show agents the elements, not
    /// the screen, and a tap finds its element by place. An element
    /// whose id, place or words differ from the drawn row sends an
    /// agent's tap to another row, or shows it words the user never
    /// sees.
    #[test]
    fn dump_lists_ids_and_places_as_drawn() {
        let mut board = sample_board();
        let list = board.dump(40, 16, None);
        board.handle_input(&tap_on(&list.lines, "Retry a failed"));
        let details = board.dump(40, 16, None);

        let ids: Vec<&str> = list
            .elements
            .iter()
            .filter(|element| !element.hidden)
            .map(|element| element.id.as_str())
            .collect();
        let expected = [
            "count:in-progress",
            "count:in-review",
            "count:needs-you",
            "group:in-progress",
            "card:DEMO-KB-001",
            "card:DEMO-KB-002",
            "card:DEMO-KB-003",
            "card:DEMO-KB-004",
            "more:in-progress",
            "group:in-review",
            "card:DEMO-KB-010",
            "group:needs-you",
            "question:Deploy tonight?",
            "session:blocked-fix",
            "group:ready",
            "group:draft",
            "hint",
        ];
        assert_eq!(ids, expected);
        assert_eq!(details.page, "details");
        for dump in [&list, &details] {
            for element in &dump.elements {
                let Some(place) = element.place else {
                    continue;
                };
                let row = &dump.lines[usize::from(place.row)];
                let drawn: String = row.chars().skip(usize::from(place.column)).collect();
                assert!(drawn.starts_with(&element.text), "{}", element.id);
            }
        }
    }

    /// An agent learns what to open from the elements, not the screen.
    /// The cards of a folded group and behind `+ N more` must be listed,
    /// hidden, after the line that hides them and with the step that
    /// shows them. Without them an agent never opens those cards and
    /// still reports that it opened everything.
    #[test]
    fn dump_lists_hidden_cards_with_show_steps() {
        let mut board = sample_board();

        let dump = serde_json::to_value(board.dump(40, 16, None)).unwrap();

        let elements = dump["elements"].as_array().unwrap();
        // Each hidden element with the element before it.
        let hidden: Vec<[&serde_json::Value; 4]> = elements
            .windows(2)
            .filter(|pair| pair[1]["hidden"] == true)
            .map(|pair| {
                let element = &pair[1];
                [
                    &pair[0]["id"],
                    &element["id"],
                    &element["hidden_by"],
                    &element["show"],
                ]
            })
            .collect();
        let expected = [
            [
                "more:in-progress",
                "card:DEMO-KB-005",
                "more",
                "tap more:in-progress",
            ],
            ["group:ready", "card:DEMO-KB-020", "fold", "tap group:ready"],
            ["group:draft", "card:OPS-KB-001", "fold", "tap group:draft"],
        ];
        assert_eq!(hidden, expected);
        let hidden_places = elements
            .iter()
            .filter(|element| element["hidden"] == true)
            .filter(|element| element.get("column").is_some());
        assert_eq!(hidden_places.count(), 0);
    }

    /// On a short screen the list scrolls. Each row scrolled off must
    /// name the scroll toward it, and so must a card under a line
    /// scrolled off, since a tap on that line fails. A wrong direction
    /// sends an agent's steps away from the row forever.
    #[test]
    fn dump_names_scroll_toward_hidden_rows() {
        let mut board = Board::new(false);
        board.show_reading(Ok(sample_queue()));
        let lines = board.render_lines(40, 6);
        board.handle_input(&tap_on(&lines, "?2 you"));

        let dump = board.dump(40, 6, None);

        let hiding = |id: &str| {
            let element = dump.elements.iter().find(|element| element.id == id);
            let hiding = element.and_then(|element| element.hiding.as_ref()).unwrap();
            (hiding.hidden_by.word(), hiding.show.as_str())
        };
        assert_eq!(hiding("card:DEMO-KB-001"), ("scroll", "press pageup"));
        assert_eq!(hiding("card:DEMO-KB-005"), ("more", "press pageup"));
        assert_eq!(hiding("group:draft"), ("scroll", "press pagedown"));
        assert_eq!(hiding("card:OPS-KB-001"), ("fold", "press pagedown"));
    }

    /// Raw mode turns Ctrl-C into a key, so a board that misses it
    /// leaves the user no way out but closing the pane. On a details
    /// page, q, Esc and a tap go back to the list, as the bottom line
    /// says, or a user who only meant to close the page loses the
    /// board. A phone's Esc is easy to hit by mistake, so Esc on the
    /// list does nothing.
    #[test]
    fn handle_input_quits_or_returns_to_list() {
        struct Case {
            name: &'static str,
            on_details: bool,
            input: Event,
            expected: Step,
        }
        let ctrl_c = Event::Key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
        let tap = Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 0,
            row: 5,
            modifiers: KeyModifiers::NONE,
        });
        let cases = [
            Case {
                name: "ctrl-c-on-list",
                on_details: false,
                input: ctrl_c,
                expected: Step::Quit,
            },
            Case {
                name: "q-on-list",
                on_details: false,
                input: press(KeyCode::Char('q')),
                expected: Step::Quit,
            },
            Case {
                name: "esc-on-list",
                on_details: false,
                input: press(KeyCode::Esc),
                expected: Step::Stay,
            },
            Case {
                name: "q-on-details",
                on_details: true,
                input: press(KeyCode::Char('q')),
                expected: Step::Stay,
            },
            Case {
                name: "esc-on-details",
                on_details: true,
                input: press(KeyCode::Esc),
                expected: Step::Stay,
            },
            Case {
                name: "tap-on-details",
                on_details: true,
                input: tap,
                expected: Step::Stay,
            },
        ];

        for case in cases {
            let mut board = sample_board();
            if case.on_details {
                board.handle_input(&press(KeyCode::Char('j')));
                board.handle_input(&press(KeyCode::Enter));
            }

            let step = board.handle_input(&case.input);
            let lines = board.render_lines(40, 16);

            assert_eq!(step, case.expected, "{}", case.name);
            assert_eq!(lines[0], "◐5 progress  ◕1 review  ?2 you", "{}", case.name);
        }
    }

    /// The board reads again every 10 seconds. A read that moved the
    /// choice would pull the row away while the user reads it, and a
    /// press of Enter would open another item.
    #[test]
    fn show_reading_keeps_the_chosen_row() {
        let mut board = sample_board();
        let lines = board.render_lines(40, 16);
        board.handle_input(&tap_on(&lines, "blocked-fix"));
        board.handle_input(&press(KeyCode::Esc));
        let answered = Queue {
            questions: Vec::new(),
            ..sample_queue()
        };

        board.show_reading(Ok(answered));
        board.handle_input(&press(KeyCode::Enter));
        let details = board.render_lines(40, 16);

        assert_eq!(details[1], "blocked-fix waits on you: needs you");
    }
}
