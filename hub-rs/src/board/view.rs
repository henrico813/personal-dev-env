//! Keeps what the board shows and changes it on keys, wheel turns and
//! taps. The board opens on the glance page, and one view at a time
//! opens over it. Nothing here touches the terminal, so `--print`,
//! `--dump`, `--control` and the tests draw the same frame the user
//! sees. Read `Board::show_reading`, `Board::handle_input` and
//! `Board::render` first.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers, MouseEventKind};
use jiff::{SignedDuration, Timestamp};
use ratatui::Frame;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{ListState, Paragraph};

use super::away;
use super::draw::{self, ListScreen};
use super::elements::{self, Action, Element, ScreenDump};
use super::glance::{self, Glance};
use super::groups::{self, Group, Item, Target};
use super::panels::{self, GLANCE_HELP, Panel, PanelText};
use super::queue::{NO_HUB, Queue};
use super::sessions::{self, SessionGroup};
use crate::external::cards::{CardKey, CardNote};

/// How many items an open group shows before its `+ N more` line.
const GROUP_LIMIT: usize = 4;

/// How many lines one wheel turn moves.
const WHEEL_STEP: isize = 3;

/// How long input must pause before Enter jumps or y cleans, so a
/// held key never acts.
const REST: Duration = Duration::from_millis(700);

/// How long the clean dialog must be open before y runs the clean
/// pass, so a y typed ahead never closes sessions.
const CONFIRM_WAIT: Duration = Duration::from_secs(1);

/// The clean dialog's bottom line.
const CONFIRM_HINT: &str = "y close and park  n cancel";

/// What the board does after one input.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Step {
    Stay,
    /// Read the hub's state again now.
    Refresh,
    /// Read this card's note for the open details page.
    ReadCard(CardKey),
    /// Show this session in Herdr, through `Herdr::focus_agent`.
    Jump(String),
    /// Close and park these sessions, the ones the clean dialog
    /// listed, in the run they were read from, through
    /// `run_clean_listed`.
    Clean {
        run: PathBuf,
        names: Vec<String>,
    },
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
    Confirm(Confirm),
    /// The clean pass's lines, or `cleaning` until they come. Keys and
    /// taps act as on a details page.
    Cleaned(Details),
}

/// The clean dialog c opens. It keeps the list c made, so a read
/// while it is open cannot change what y acts on.
struct Confirm {
    /// When c opened it, for `CONFIRM_WAIT`.
    opened: Instant,
    /// The run the rows were read from, where y cleans.
    run: PathBuf,
    /// The top line, such as `close 1  park 1`.
    title: String,
    /// One row per session, such as `old-search          close`.
    rows: Vec<String>,
    /// The sessions y cleans, the ones `rows` names.
    names: Vec<String>,
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
    /// The view open over the glance page, or `None` on the glance
    /// page.
    view: Option<Panel>,
    /// How many lines of the away, next or machine view are scrolled
    /// off the top.
    scroll: usize,
    /// The start of the user's visit, which AWAY shows events after, or
    /// `None` with no saved time.
    since: Option<Timestamp>,
    /// How long the user was away before this visit.
    away_for: Option<SignedDuration>,
    /// The session the jump bar names, by name, so a read that moves
    /// its row keeps it.
    chosen_session: Option<String>,
    /// The sessions panel's scroll and reversed row.
    session_list: ListState,
    /// When the last input came, for the `REST` before Enter or y acts.
    last_input: Option<Instant>,
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
            // Todo and Draft hold most cards, and Blocked holds nothing
            // for the user to do, so they start folded.
            open: BTreeSet::from([Group::Active, Group::InReview, Group::NeedsYou]),
            shown_in_full: BTreeSet::new(),
            lines: Vec::new(),
            list: ListState::default(),
            page: Page::List,
            view: None,
            scroll: 0,
            since: None,
            away_for: None,
            chosen_session: None,
            session_list: ListState::default(),
            last_input: None,
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
        let chosen_id = chosen.and_then(|index| self.find_line_id(index));
        self.status = format!("read {} {:.1}s", queue.read_at, queue.took.as_secs_f32());
        self.groups = groups::build_groups(&queue);
        // A session gone since the last read can no longer be jumped to.
        self.chosen_session = self
            .chosen_session
            .take()
            .filter(|name| queue.sessions.iter().any(|row| row.name == *name));
        self.queue = Some(queue);
        self.rebuild_lines();
        let same_line = chosen_id.and_then(|id| {
            (0..self.lines.len()).find(|&index| self.find_line_id(index).as_ref() == Some(&id))
        });
        let last = self.lines.len().saturating_sub(1);
        let place = same_line.unwrap_or_else(|| chosen.unwrap_or(0).min(last));
        self.list.select((!self.lines.is_empty()).then_some(place));
    }

    /// Shows the events after `since` in AWAY, and `away_for`, how long
    /// the user was away, on the glance page's top line. With `since`
    /// `None`, AWAY shows the newest events.
    pub(crate) fn show_visit(
        &mut self,
        since: Option<Timestamp>,
        away_for: Option<SignedDuration>,
    ) {
        self.since = since;
        self.away_for = away_for;
    }

    /// Marks the board as reading, until the next `show_reading`.
    pub(crate) fn note_reading(&mut self) {
        self.status = "reading".to_owned();
    }

    /// Shows the clean pass's lines on their own page, over any page
    /// open, since the user asked for them with y.
    pub(crate) fn show_clean_output(&mut self, lines: Vec<String>) {
        self.page = Page::Cleaned(Details {
            lines,
            scroll: 0,
            reading: None,
        });
    }

    /// Shows `text` at the end of the bottom line, until the next read.
    pub(crate) fn show_status(&mut self, text: String) {
        self.status = text;
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
        self.handle_input_at(input, Instant::now())
    }

    /// Does what `handle_input` does for an input that came at `now`.
    /// Enter on the sessions panel and y in the dialog act only after
    /// `REST`.
    pub(crate) fn handle_input_at(&mut self, input: &Event, now: Instant) -> Step {
        let rested = self
            .last_input
            .is_none_or(|last| now.saturating_duration_since(last) >= REST);
        self.last_input = Some(now);
        match input {
            Event::Key(key) => self.handle_key(key, now, rested),
            Event::Mouse(mouse) => match mouse.kind {
                MouseEventKind::ScrollDown => self.scroll(WHEEL_STEP),
                MouseEventKind::ScrollUp => self.scroll(-WHEEL_STEP),
                MouseEventKind::Down(_) => self.tap(mouse.column, mouse.row),
                _ => Step::Stay,
            },
            _ => Step::Stay,
        }
    }

    /// Draws the open page over the whole frame. That page is a details
    /// page, the clean dialog, the open view or the glance page.
    pub(crate) fn render(&mut self, frame: &mut Frame) {
        self.area = frame.area();
        if let Page::Details(details) | Page::Cleaned(details) = &mut self.page {
            draw::draw_details(frame, details);
            return;
        }
        if let Page::Confirm(_) = self.page {
            self.draw_confirm(frame);
            return;
        }
        match self.view {
            None => glance::draw_glance(frame, self.area, &self.build_glance()),
            Some(panel) => self.draw_view(frame, panel),
        }
    }

    /// Returns every element the open page holds, top first, as the
    /// last draw left it. Those the draw left out are hidden.
    pub(crate) fn collect_elements(&self) -> Vec<Element> {
        if let Page::Details(details) | Page::Cleaned(details) = &self.page {
            return draw::build_details_elements(details, self.area);
        }
        if let Page::Confirm(_) = self.page {
            return self.build_confirm_elements();
        }
        match self.view {
            None => glance::build_glance_elements(self.area, &self.build_glance()),
            Some(Panel::Queue) => {
                draw::build_list_elements(&self.build_list_screen(), self.area, &self.list)
            }
            Some(panel) => self.build_view_elements(panel),
        }
    }

    /// Draws the open page at `columns` by `rows` and returns it as
    /// elements and text, after the `--control` or `--send` line `step`.
    pub(crate) fn dump(&mut self, columns: u16, rows: u16, step: Option<String>) -> ScreenDump {
        let lines = self.render_lines(columns, rows);
        let page = match self.page {
            Page::List => "list",
            Page::Details(_) => "details",
            Page::Confirm(_) => "confirm",
            Page::Cleaned(_) => "clean",
        };
        ScreenDump {
            step,
            page,
            elements: self.collect_elements(),
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

    /// Returns what the glance page draws from.
    fn build_glance(&self) -> Glance<'_> {
        Glance {
            groups: &self.groups,
            queue: self.queue.as_ref(),
            status: &self.status,
            since: self.since,
            away_for: self.away_for,
            color: self.color,
        }
    }

    /// Returns a view's breadcrumb, such as `< away`, with the count of
    /// new events after it on the away view.
    fn format_view_title(&self, panel: Panel) -> String {
        match (panel, &self.queue) {
            (Panel::Away, Some(queue)) => {
                let count = self.select_away_events(queue).len();
                format!("< away  {}", away::describe_new_count(self.since, count))
            }
            _ => format!("< {}", panel.slug()),
        }
    }

    /// Returns the events the away view and block show.
    fn select_away_events<'a>(&self, queue: &'a Queue) -> &'a [away::AwayEvent] {
        away::find_events_since(&queue.events, self.since)
    }

    /// Returns the lines of the away, next or machine view, `width`
    /// columns wide. The queue and sessions views draw lists of their
    /// own.
    fn format_view_lines(&self, panel: Panel, width: usize) -> Vec<PanelText> {
        let Some(queue) = &self.queue else {
            return Vec::new();
        };
        match panel {
            Panel::Away => panels::format_away_lines(self.select_away_events(queue), width),
            Panel::Next => panels::format_next_lines(&self.groups, queue.no_hub, usize::MAX, width),
            Panel::Machine => panels::format_machine_lines(&queue.gauges, &queue.feed, width),
            Panel::Queue | Panel::Sessions => Vec::new(),
        }
    }

    /// Draws `panel`'s view over the whole frame, with the breadcrumb on
    /// top, the view's lines, and the bottom hint.
    fn draw_view(&mut self, frame: &mut Frame, panel: Panel) {
        if panel == Panel::Queue {
            // Built field by field, since `self.list` is borrowed
            // mutably beside it.
            let screen = ListScreen {
                groups: &self.groups,
                lines: &self.lines,
                status: &self.status,
                color: self.color,
            };
            draw::draw_list(frame, self.area, &screen, &mut self.list);
            return;
        }
        let [title, body, hint] = draw::split_screen(self.area);
        let bold = Style::new().add_modifier(Modifier::BOLD);
        frame.render_widget(
            Paragraph::new(self.format_view_title(panel)).style(bold),
            title,
        );
        frame.render_widget(
            Paragraph::new(draw::format_view_hint(&self.status)).style(draw::pick_dim_style()),
            hint,
        );
        if panel == Panel::Sessions {
            let rows = self.format_session_rows(usize::from(body.width));
            self.session_list.select(self.find_chosen_line(&rows));
            sessions::draw_sessions(
                frame,
                body,
                &rows,
                self.chosen_session.as_deref(),
                &mut self.session_list,
            );
            return;
        }
        let lines = self.format_view_lines(panel, usize::from(body.width));
        // Stop the scroll where the last line shows at the bottom.
        let last_top = lines.len().saturating_sub(usize::from(body.height));
        self.scroll = self.scroll.min(last_top);
        panels::draw_panel_text(frame, body, &lines, self.scroll, self.color);
    }

    /// Returns the elements of a view other than the queue's, as
    /// `draw_view` drew them.
    fn build_view_elements(&self, panel: Panel) -> Vec<Element> {
        let [title, body, hint] = draw::split_screen(self.area);
        let mut elements = vec![draw::build_back_element(
            &self.format_view_title(panel),
            title,
        )];
        if panel == Panel::Sessions {
            let rows = self.format_session_rows(usize::from(body.width));
            let sessions = self.queue.as_ref().map_or(&[][..], |queue| &queue.sessions);
            elements.extend(sessions::build_session_elements(
                &rows,
                sessions,
                body,
                &self.session_list,
                self.chosen_session.as_deref(),
            ));
        } else {
            let lines = self.format_view_lines(panel, usize::from(body.width));
            elements.extend(panels::build_text_elements(
                panel,
                &lines,
                body,
                self.scroll,
            ));
        }
        elements.push(draw::build_text_element(
            "hint",
            &draw::format_view_hint(&self.status),
            hint,
        ));
        elements
    }

    /// Returns how many lines PageDown moves, the rows the open page or
    /// view shows, so it never passes a hidden line whose step is
    /// `press pagedown`.
    fn measure_page_size(&self) -> isize {
        // The top line and the hint take two rows.
        let rows = self.area.height.saturating_sub(2);
        let rows = match (&self.page, self.view) {
            // The jump bar takes the body's last row.
            (Page::List, Some(Panel::Sessions)) => rows.saturating_sub(1),
            _ => rows,
        };
        isize::try_from(rows).unwrap_or(1).max(1)
    }

    /// Opens `panel`'s view over the glance page, at its top.
    fn open_view(&mut self, panel: Panel) {
        self.view = Some(panel);
        self.scroll = 0;
    }

    /// Opens the help page of the open view, or of the glance page.
    fn open_help(&mut self) {
        let help = self.view.map_or(GLANCE_HELP, Panel::help);
        self.page = Page::Details(Details {
            lines: help.iter().map(|line| (*line).to_owned()).collect(),
            scroll: 0,
            reading: None,
        });
    }

    /// Quits, asks for a read, moves the choice or acts on it. Enter on
    /// the sessions panel and y in the clean dialog act only when
    /// `rested`, and `now` times the clean dialog.
    fn handle_key(&mut self, key: &KeyEvent, now: Instant, rested: bool) -> Step {
        let ctrl_c =
            key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL);
        if ctrl_c {
            return Step::Quit;
        }
        if let Page::Confirm(_) = self.page {
            return self.answer_confirm(key, now, rested);
        }
        if key.code == KeyCode::Char('r') {
            return Step::Refresh;
        }
        let page_size = self.measure_page_size();
        if let Page::Details(_) | Page::Cleaned(_) = self.page {
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
            KeyCode::Tab => {
                match self.view.map_or(Some(Panel::Away), Panel::find_next_panel) {
                    Some(panel) => self.open_view(panel),
                    None => self.view = None,
                }
                return Step::Stay;
            }
            KeyCode::Char(digit @ '1'..='5') => {
                self.open_view(Panel::ALL[usize::from(digit as u8 - b'1')]);
                return Step::Stay;
            }
            KeyCode::Char('?') => {
                self.open_help();
                return Step::Stay;
            }
            KeyCode::Char('c') => {
                self.open_confirm(now);
                return Step::Stay;
            }
            KeyCode::Char('q') | KeyCode::Esc if self.view.is_some() => {
                self.view = None;
                return Step::Stay;
            }
            KeyCode::Char('q') => return Step::Quit,
            _ => {}
        }
        match self.view {
            // The glance page does not scroll, and Esc on it does
            // nothing, since a phone's Esc is easy to hit by mistake.
            None => return Step::Stay,
            Some(Panel::Queue) => {}
            Some(Panel::Away | Panel::Next | Panel::Machine) => {
                match key.code {
                    KeyCode::Char('j') | KeyCode::Down => self.scroll(1),
                    KeyCode::Char('k') | KeyCode::Up => self.scroll(-1),
                    KeyCode::PageDown => self.scroll(page_size),
                    KeyCode::PageUp => self.scroll(-page_size),
                    _ => Step::Stay,
                };
                return Step::Stay;
            }
            Some(Panel::Sessions) => {
                return match key.code {
                    KeyCode::Char('j') | KeyCode::Down => self.move_session(1),
                    KeyCode::Char('k') | KeyCode::Up => self.move_session(-1),
                    KeyCode::PageDown => self.move_session(page_size),
                    KeyCode::PageUp => self.move_session(-page_size),
                    KeyCode::Enter if rested => self.build_jump_step(),
                    _ => Step::Stay,
                };
            }
        }
        match key.code {
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

    /// Acts on the element under a tap. A block opens its view, a
    /// breadcrumb goes back, a count jumps to its group, a queue line
    /// acts as Enter does, a session row only chooses, and the bar
    /// jumps. Any tap on another page goes back to the list.
    fn tap(&mut self, column: u16, row: u16) -> Step {
        if let Page::Details(_) | Page::Cleaned(_) | Page::Confirm(_) = self.page {
            self.page = Page::List;
            return Step::Stay;
        }
        let elements = self.collect_elements();
        let action =
            elements::find_element_at(&elements, column, row).map(|element| element.action);
        match action {
            Some(Action::Jump(group)) => {
                self.jump_to(group);
                Step::Stay
            }
            Some(Action::Line(index)) => self.activate(index),
            Some(Action::Choose(index)) => {
                self.choose_session(index);
                Step::Stay
            }
            Some(Action::Go) => self.build_jump_step(),
            Some(Action::Open(panel)) => {
                self.open_view(panel);
                Step::Stay
            }
            Some(Action::Help) => {
                self.open_help();
                Step::Stay
            }
            Some(Action::Back) => {
                self.view = None;
                Step::Stay
            }
            Some(Action::None) | None => Step::Stay,
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

    /// Opens an item's details page. A card whose note the read did
    /// not get shows what `Cards::list_cards` gave, and asks for its
    /// note.
    fn open_details(&mut self, group: Group, index: usize) -> Step {
        let (Some(queue), Some(item)) = (&self.queue, self.find_item(group, index)) else {
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

    /// Moves the choice on the queue or the sessions view, scrolls the
    /// away, next or machine view, or scrolls a details or clean output
    /// page. The glance page does not scroll.
    fn scroll(&mut self, steps: isize) -> Step {
        match &mut self.page {
            Page::List => match self.view {
                None => {}
                Some(Panel::Queue) => self.move_choice(steps),
                Some(Panel::Sessions) => {
                    self.move_session(steps);
                }
                // `draw_view` stops the scroll at the view's end.
                Some(Panel::Away | Panel::Next | Panel::Machine) => {
                    self.scroll = self.scroll.saturating_add_signed(steps);
                }
            },
            // `draw_details` stops the scroll at the page's end.
            Page::Details(details) | Page::Cleaned(details) => {
                details.scroll = details.scroll.saturating_add_signed(steps);
            }
            Page::Confirm(_) => {}
        }
        Step::Stay
    }

    /// Opens the clean dialog, or says at the end of the bottom line why
    /// there is nothing to clean.
    fn open_confirm(&mut self, now: Instant) {
        let Some(queue) = &self.queue else {
            return;
        };
        let Some(run) = &queue.run else {
            self.status = NO_HUB.to_owned();
            return;
        };
        let names: Vec<String> = queue
            .sessions
            .iter()
            .filter(|row| SessionGroup::from_state(row.state) == SessionGroup::SafeToClose)
            .map(|row| row.name.clone())
            .collect();
        if names.is_empty() {
            self.status = "nothing to close".to_owned();
        } else {
            let (title, rows) = sessions::format_clean_dialog_lines(&queue.sessions);
            self.page = Page::Confirm(Confirm {
                opened: now,
                run: run.clone(),
                title,
                rows,
                names,
            });
        }
    }

    /// Asks for the clean pass on y once the dialog has been open
    /// `CONFIRM_WAIT` and no input came in the `REST` before it, so a
    /// held y never cleans. A y sooner does nothing, and any other key
    /// closes the dialog.
    fn answer_confirm(&mut self, key: &KeyEvent, now: Instant, rested: bool) -> Step {
        let Page::Confirm(confirm) = &self.page else {
            return Step::Stay;
        };
        if key.code != KeyCode::Char('y') {
            self.page = Page::List;
            return Step::Stay;
        }
        let open_for = now.saturating_duration_since(confirm.opened);
        if open_for < CONFIRM_WAIT || !rested {
            return Step::Stay;
        }
        let names = confirm.names.clone();
        let run = confirm.run.clone();
        self.show_clean_output(vec!["cleaning".to_owned()]);
        Step::Clean { run, names }
    }

    /// Draws the clean dialog over the whole frame.
    fn draw_confirm(&self, frame: &mut Frame) {
        let [top, middle, bottom] = draw::split_screen(frame.area());
        let (title, rows) = self.format_confirm_lines(usize::from(middle.height));
        let bold = Style::new().add_modifier(Modifier::BOLD);
        frame.render_widget(Paragraph::new(title).style(bold), top);
        let shown: Vec<ratatui::text::Line> = rows.into_iter().map(Into::into).collect();
        frame.render_widget(Paragraph::new(shown), middle);
        frame.render_widget(Paragraph::new(CONFIRM_HINT).style(bold), bottom);
    }

    /// Returns the clean dialog's elements as `draw_confirm` drew them.
    fn build_confirm_elements(&self) -> Vec<Element> {
        let [top, middle, bottom] = draw::split_screen(self.area);
        let (title, rows) = self.format_confirm_lines(usize::from(middle.height));
        let mut elements = vec![draw::build_text_element("title", &title, top)];
        elements.extend(rows.iter().zip(middle.y..middle.bottom()).enumerate().map(
            |(number, (row, y))| {
                draw::build_text_element(&format!("confirm:{number}"), row, Rect { y, ..middle })
            },
        ));
        elements.push(draw::build_text_element("hint", CONFIRM_HINT, bottom));
        elements
    }

    /// Returns the clean dialog's top line and the rows that fit in
    /// `room` rows. When some do not fit, the last row is `+ N more`, so
    /// the user knows y acts on more than the screen shows.
    fn format_confirm_lines(&self, room: usize) -> (String, Vec<String>) {
        let Page::Confirm(confirm) = &self.page else {
            return (String::new(), Vec::new());
        };
        let mut rows = confirm.rows.clone();
        if rows.len() > room {
            let shown = room.saturating_sub(1);
            let more = rows.len() - shown;
            rows.truncate(shown);
            rows.push(format!("+ {more} more"));
        }
        (confirm.title.clone(), rows)
    }

    /// Chooses the session `steps` rows down the sessions view, or up
    /// when negative, stopping at the first and last. With none chosen
    /// it chooses the first.
    fn move_session(&mut self, steps: isize) -> Step {
        let Some(queue) = &self.queue else {
            return Step::Stay;
        };
        let order = sessions::order_shown_sessions(&queue.sessions);
        let Some(last) = order.len().checked_sub(1) else {
            return Step::Stay;
        };
        let current = self.chosen_session.as_ref().and_then(|name| {
            order
                .iter()
                .position(|&index| queue.sessions[index].name == *name)
        });
        let place = current.map_or(0, |at| at.saturating_add_signed(steps).min(last));
        self.chosen_session = Some(queue.sessions[order[place]].name.clone());
        Step::Stay
    }

    /// Chooses the session at `index` in the queue's sessions. It never
    /// jumps.
    fn choose_session(&mut self, index: usize) {
        self.chosen_session = self
            .queue
            .as_ref()
            .and_then(|queue| queue.sessions.get(index))
            .map(|row| row.name.clone());
    }

    /// Returns a jump to the chosen session, or `Step::Stay` with none
    /// chosen.
    fn build_jump_step(&self) -> Step {
        self.chosen_session.clone().map_or(Step::Stay, Step::Jump)
    }

    /// Returns the sessions panel's lines, `width` columns wide.
    fn format_session_rows(&self, width: usize) -> Vec<sessions::PanelLine> {
        self.queue.as_ref().map_or_else(Vec::new, |queue| {
            sessions::format_session_lines(&queue.sessions, width)
        })
    }

    /// Returns the place in `rows` of the chosen session's row.
    fn find_chosen_line(&self, rows: &[sessions::PanelLine]) -> Option<usize> {
        let queue = self.queue.as_ref()?;
        let name = self.chosen_session.as_ref()?;
        rows.iter().position(|line| {
            line.session
                .is_some_and(|index| queue.sessions[index].name == *name)
        })
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

    /// Returns what the queue view draws from.
    fn build_list_screen(&self) -> ListScreen<'_> {
        ListScreen {
            groups: &self.groups,
            lines: &self.lines,
            status: &self.status,
            color: self.color,
        }
    }

    /// Returns the item at `index` in `group`.
    fn find_item(&self, group: Group, index: usize) -> Option<&Item> {
        self.groups
            .iter()
            .find(|(each, _)| *each == group)
            .and_then(|(_, items)| items.get(index))
    }

    /// Returns what names the line at `index` across reads, or `None`
    /// past the end.
    fn find_line_id(&self, index: usize) -> Option<LineId> {
        match *self.lines.get(index)? {
            Line::Heading { group, .. } => Some(LineId::Heading(group)),
            Line::More { group, .. } => Some(LineId::More(group)),
            Line::Item { group, index } => Some(LineId::Item(
                group,
                self.find_item(group, index)?.id.clone(),
            )),
            Line::NoHub => Some(LineId::NoHub),
        }
    }
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;

    use std::time::Duration;

    use crossterm::event::{MouseButton, MouseEvent};

    use crate::board::away::AwayEvent;
    use crate::board::machine::{FeedLine, Gauges, Usage};
    use crate::board::queue::{CardLine, SessionLine, SessionRow};
    use crate::external::cards::Stage;
    use crate::shared::lifecycle::{KeptReason, Lifecycle};
    use crate::shared::questions::OpenQuestion;

    /// A queue with five cards in progress, one needing the user's
    /// review, a question, a session that waits on the user as the
    /// worker of DEMO-KB-003, one card ready to start and one the user
    /// has not refined. Its sessions fill every session group, one
    /// waiting on the hub and one to park. Its watch gauge is
    /// past 90%, its feed holds two reports, and AWAY has those and a
    /// `notes.md` line, the newest at `10:40`.
    pub(crate) fn make_sample_queue() -> Queue {
        let make_session = |name: &str, state, why: &str, stage, tokens| {
            SessionRow::new(name.to_owned(), state, why.to_owned(), stage, tokens)
        };
        let make_report = |at: &str, unread, name: &str| FeedLine {
            at: at.to_owned(),
            stamp: format!("2026-11-15T{at}:00Z").parse().unwrap(),
            unread,
            name: name.to_owned(),
        };
        let make_event = |time: &str, text: &str| AwayEvent {
            at: format!("2026-11-15T{time}:00Z").parse().unwrap(),
            time: time.to_owned(),
            text: text.to_owned(),
        };
        let make_card = |key: &str, stage, title: &str, workers: &[&str]| CardLine {
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
                make_card(
                    "DEMO-KB-001",
                    Stage::Active,
                    "Faster search",
                    &["api-build"],
                ),
                make_card(
                    "DEMO-KB-002",
                    Stage::Active,
                    "Board for the phone",
                    &["card-sync"],
                ),
                make_card(
                    "DEMO-KB-003",
                    Stage::Active,
                    "Trim the build cache",
                    &["blocked-fix"],
                ),
                make_card(
                    "DEMO-KB-004",
                    Stage::Active,
                    "Lint every Dockerfile in the repository",
                    &[],
                ),
                make_card("DEMO-KB-005", Stage::Active, "Rotate the backup keys", &[]),
                make_card(
                    "DEMO-KB-010",
                    Stage::InReview,
                    "Retry a failed upload twice, then show the error",
                    &["docs-fix"],
                ),
                make_card("DEMO-KB-020", Stage::Todo, "Sort ready cards", &[]),
                make_card("OPS-KB-001", Stage::Draft, "Watch the disks", &[]),
            ],
            questions: vec![OpenQuestion {
                key: "DEMO-KB-001".parse().unwrap(),
                project: "Demo".to_owned(),
                title: "Deploy tonight?".to_owned(),
                lines: vec![
                    "Deploy the new model tonight?".to_owned(),
                    "- yes".to_owned(),
                    "- no".to_owned(),
                    "Recommended: yes".to_owned(),
                ],
                sessions: Vec::new(),
                asked_on: None,
            }],
            waiting: vec![SessionLine {
                name: "blocked-fix".to_owned(),
                why: "needs you".to_owned(),
                card: None,
            }],
            sessions: vec![
                make_session(
                    "api-build",
                    Lifecycle::Working,
                    "-",
                    Some(Stage::Active),
                    Some(160_000),
                ),
                make_session(
                    "blocked-fix",
                    Lifecycle::WaitsOnUser,
                    "needs you",
                    None,
                    Some(40_000),
                ),
                make_session(
                    "card-sync",
                    Lifecycle::WaitsOnHub,
                    "no reply yet",
                    Some(Stage::Active),
                    Some(20_000),
                ),
                make_session(
                    "docs-fix",
                    Lifecycle::Kept(KeptReason::RecentRequest),
                    "request in the last hour",
                    Some(Stage::InReview),
                    Some(90_000),
                ),
                make_session(
                    "old-search",
                    Lifecycle::ToClose,
                    "card done",
                    Some(Stage::Done),
                    None,
                ),
                make_session(
                    "ship-docs",
                    Lifecycle::ToPark,
                    "-",
                    Some(Stage::InReview),
                    Some(30_000),
                ),
                make_session(
                    "upload-retry",
                    Lifecycle::Parked,
                    "-",
                    Some(Stage::InReview),
                    None,
                ),
            ],
            gauges: Gauges {
                watch: Some(Usage {
                    used: 116,
                    limit: 128,
                }),
                disk: Some(Usage {
                    used: 73_000_000,
                    limit: 100_000_000,
                }),
            },
            feed: vec![
                make_report("10:40", true, "api-build"),
                make_report("10:12", false, "docs-fix"),
            ],
            events: vec![
                make_event("10:40", "api-build reported"),
                make_event("10:31", "launched card-sync on DEMO-KB-002"),
                make_event("10:12", "docs-fix reported"),
            ],
            no_hub: false,
            run: Some(PathBuf::from("/runs/sample")),
            notes: [("DEMO-KB-010".parse().unwrap(), reviewed_note)].into(),
            problems: Vec::new(),
        }
    }

    /// Returns a board that shows `make_sample_queue` on the glance page,
    /// drawn once at 40x16.
    pub(crate) fn make_sample_board() -> Board {
        let mut board = Board::new(false);
        board.show_reading(Ok(make_sample_queue()));
        board.render_lines(40, 16);
        board
    }

    /// Returns `make_sample_board` with the queue view open, drawn once at
    /// 40x16.
    pub(crate) fn make_queue_board() -> Board {
        let mut board = make_sample_board();
        board.handle_input(&press(KeyCode::Char('2')));
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

    /// The user asked the board three things. What happened while they
    /// were away? What is in the queue, and does it run now? What comes
    /// next? The phone's first screen, 40 by 16, must answer all
    /// three with no key pressed. A block cut off, or a queue row without
    /// its status word, leaves one question open. A line about a card
    /// starts with its card key, such as `DEMO-KB-001`, so the user
    /// can tell which plan goes with the card.
    #[test]
    fn render_lines_glance_answers_three_questions() {
        let mut board = Board::new(false);
        board.show_reading(Ok(make_sample_queue()));

        let lines = board.render_lines(40, 16);

        let expected = [
            "read 10:42 0.6s",
            "AWAY  newest                         [1]",
            " 10:40 api-build reported",
            " 10:31 DEMO-KB-002 launched card-sync o~",
            " 10:12 docs-fix reported",
            "QUEUE  1 run · 2 you · 1 idle        [2]",
            " you  DEMO-KB-001 Deploy tonight? Demo",
            " you  blocked-fix  needs you      worker",
            " run  DEMO-KB-001 Faster search   Demo",
            " idle DEMO-KB-002 Board for the ~ Demo",
            " rev  DEMO-KB-010 Retry a failed~ Demo",
            "NEXT                                 [3]",
            " you: answer DEMO-KB-001 Deploy tonight?",
            " then: review 1 card",
            " hub: start DEMO-KB-020 Sort ready cards",
            "? help  4 sessions  5 machine  q quit",
        ];
        assert_eq!(lines, expected);
    }

    /// AWAY must show only what came after the user's last visit, and the
    /// top line how long they were away. Older events listed as new would
    /// bury the news under what the user already read.
    #[test]
    fn render_lines_glance_shows_events_since_visit() {
        let mut board = Board::new(false);
        board.show_reading(Ok(make_sample_queue()));
        let since = "2026-11-15T10:31:00Z".parse().ok();

        board.show_visit(since, Some(SignedDuration::from_mins(89)));
        let lines = board.render_lines(40, 16);

        assert_eq!(lines[0], "away 1h29m · read 10:42 0.6s");
        assert_eq!(lines[1], "AWAY  1 new                          [1]");
        assert_eq!(lines[2], " 10:40 api-build reported");
        assert_eq!(lines[3], "");
    }

    /// A phone in landscape, or a pane split in two, can leave the board
    /// 10 rows. AWAY and NEXT must give up rows before QUEUE, which
    /// holds what waits on the user, and each block must keep its
    /// heading so a tap can open it.
    #[test]
    fn render_lines_short_glance_keeps_queue_rows() {
        let mut board = make_sample_board();

        let lines = board.render_lines(40, 10);

        let expected = [
            "read 10:42 0.6s",
            "AWAY  newest                         [1]",
            "   + 3 more",
            "QUEUE  1 run · 2 you · 1 idle        [2]",
            " you  DEMO-KB-001 Deploy tonight? Demo",
            " you  blocked-fix  needs you      worker",
            "   + 3 more",
            "NEXT                                 [3]",
            "   + 3 more",
            "? help  4 sessions  5 machine  q quit",
        ];
        assert_eq!(lines, expected);
    }

    /// A block shows only the rows that fit, so the rest must be one tap
    /// away. A tap on the block's `+ N more` line opens it full screen
    /// under a breadcrumb, and a tap on the breadcrumb comes back to the
    /// same glance page. Without the way back, a phone user is stuck in
    /// the view.
    #[test]
    fn handle_input_block_tap_opens_breadcrumb_closes() {
        let mut board = Board::new(false);
        board.show_reading(Ok(make_many_questions_queue()));
        let glance = board.render_lines(40, 16);

        board.handle_input(&tap_on(&glance, "+ 5 more"));
        let opened = board.render_lines(40, 16);
        board.handle_input(&tap_on(&opened, "< queue"));
        let back = board.render_lines(40, 16);

        assert_eq!(glance[10], "   + 5 more");
        assert_eq!(opened[0], "< queue  1 run · 6 you · 1 idle");
        assert_eq!(back, glance);
    }

    /// An agent learns what to open from the elements, not the screen.
    /// The rows a glance block cuts must be listed, hidden, with the tap
    /// on the block that shows them, or the agent never reads them.
    #[test]
    fn dump_glance_lists_cut_rows_with_block_tap() {
        let mut board = Board::new(false);
        board.show_reading(Ok(make_many_questions_queue()));

        let dump = board.dump(40, 16, None);

        let hidden: Vec<(&str, &str)> = dump
            .elements
            .iter()
            .filter_map(|element| {
                let hiding = element.hiding.as_ref()?;
                Some((element.text.as_str(), hiding.show.as_str()))
            })
            .collect();
        let expected = [
            (" you  DEMO-KB-001 Question 4?     Demo", "tap block:queue"),
            (
                " you  blocked-fix  needs you      worker",
                "tap block:queue",
            ),
            (" run  DEMO-KB-001 Faster search   Demo", "tap block:queue"),
            (" idle DEMO-KB-002 Board for the ~ Demo", "tap block:queue"),
            (" rev  DEMO-KB-010 Retry a failed~ Demo", "tap block:queue"),
        ];
        assert_eq!(hidden, expected);
    }

    /// The user reads the board on a phone, 40 by 16. What needs the
    /// user comes first, then cards in progress or needing the user's
    /// review, a few rows each, while cards not yet started stay
    /// folded. Each row's status word tells whether a worker runs on
    /// it now or waits on the user. A row cut without `~`, a wrong
    /// word, or a group out of order hides what the user looks for.
    #[test]
    fn render_lines_groups_folds_and_cuts_rows() {
        let mut board = make_queue_board();

        let lines = board.render_lines(40, 16);

        let expected = [
            "< queue  1 run · 2 you · 1 idle",
            "v Needs you 2",
            " you  DEMO-KB-001 Deploy tonight? Demo",
            " you  blocked-fix  needs you      worker",
            "v Active 5",
            " run  DEMO-KB-001 Faster search   Demo",
            " idle DEMO-KB-002 Board for the ~ Demo",
            " you  DEMO-KB-003 Trim the build~ Demo",
            " none DEMO-KB-004 Lint every Doc~ Demo",
            "   + 1 more",
            "v In review 1",
            " rev  DEMO-KB-010 Retry a failed~ Demo",
            "> Todo 1",
            "> Draft 1",
            "",
            "esc back  ? help  read 10:42 0.6s",
        ];
        assert_eq!(lines, expected);
    }

    /// A tap is the user's main input on a phone. A tap on a heading
    /// must fold or open its group, and a tap on `+ N more` must show
    /// the rest, or the cards past the first four cannot be reached.
    #[test]
    fn handle_input_taps_fold_and_show_more() {
        let mut board = make_queue_board();
        let lines = board.render_lines(40, 16);

        board.handle_input(&tap_on(&lines, "v Active"));
        let folded = board.render_lines(40, 16);
        board.handle_input(&tap_on(&folded, "> Active"));
        let opened = board.render_lines(40, 16);
        board.handle_input(&tap_on(&opened, "+ 1 more"));
        let in_full = board.render_lines(40, 16);

        assert_eq!(folded[4..6], ["> Active 5", "v In review 1"]);
        assert_eq!(opened, lines);
        assert_eq!(in_full[9], " none DEMO-KB-005 Rotate the bac~ Demo");
        assert_eq!(in_full[10], "v In review 1");
    }

    /// `make_sample_queue` with four more questions, so Needs you fills a
    /// short screen and the cards in work start below it. The glance
    /// page's QUEUE block at 40x16 cuts five rows.
    fn make_many_questions_queue() -> Queue {
        let mut queue = make_sample_queue();
        queue.questions.extend((1..=4).map(|number| OpenQuestion {
            key: "DEMO-KB-001".parse().unwrap(),
            project: "Demo".to_owned(),
            title: format!("Question {number}?"),
            lines: Vec::new(),
            sessions: Vec::new(),
            asked_on: None,
        }));
        queue
    }

    /// Returns a board that shows `queue` with the queue view open,
    /// drawn once at 40 by `rows`.
    fn draw_short_queue_view(queue: Queue, rows: u16) -> (Board, Vec<String>) {
        let mut board = Board::new(false);
        board.show_reading(Ok(queue));
        board.render_lines(40, rows);
        board.handle_input(&press(KeyCode::Char('2')));
        let lines = board.render_lines(40, rows);
        (board, lines)
    }

    /// Each count on the top line jumps to its group. With many
    /// questions, the cards in work start below the first screen, so the
    /// tap must scroll their group into view, or the user never sees what
    /// runs.
    #[test]
    fn handle_input_count_tap_shows_its_group() {
        let (mut board, lines) = draw_short_queue_view(make_many_questions_queue(), 8);

        board.handle_input(&tap_on(&lines, "1 run"));
        let jumped = board.render_lines(40, 8);

        assert!(!lines.iter().any(|line| line.contains("Active")));
        assert!(jumped.iter().any(|line| line == "v Active 5"));
        assert!(jumped.iter().any(|line| line.contains("Faster search")));
    }

    /// A tap on a scrolled list must open the row drawn under the
    /// finger. If the tap ignored the scroll, the user would open a
    /// different card than the one they tapped.
    #[test]
    fn handle_input_scrolled_tap_opens_drawn_row() {
        let (mut board, lines) = draw_short_queue_view(make_many_questions_queue(), 8);
        board.handle_input(&tap_on(&lines, "1 run"));
        let scrolled = board.render_lines(40, 8);

        board.handle_input(&tap_on(&scrolled, "Faster search"));
        let details = board.render_lines(40, 8);

        assert_eq!(details[1], "Faster search");
    }

    /// The row cuts a card to one line, so its details page is where
    /// the user sees who works on it, what to do next and its pull
    /// request. The read already holds this card's note, so opening the
    /// page must not read the note again and make the user wait.
    #[test]
    fn handle_input_tap_shows_details_without_reading() {
        let mut board = make_queue_board();
        let lines = board.render_lines(40, 16);

        let step = board.handle_input(&tap_on(&lines, "Retry a failed"));
        let details = board.render_lines(40, 16);

        assert_eq!(step, Step::Stay);
        let expected = [
            "< back",
            "Retry a failed upload twice, then show",
            "the error",
            "",
            "DEMO-KB-010  in-review  Demo",
            "by docs-fix",
            "next: merge after the checks pass",
            "https://github.com/demo/app/pull/301",
            "",
            "- [ ] A failed upload retries.",
        ];
        assert_eq!(details[..10], expected);
    }

    /// The read gets the note only of cards a session, a parked
    /// record or a report names, to stay fast. Any other card's page
    /// must ask for its note and then show it, or cards not yet
    /// started never show their criteria.
    #[test]
    fn handle_input_unread_card_loads_its_note() {
        let mut board = make_queue_board();
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

    /// The board's dump and control modes show agents the elements, not
    /// the screen, and a tap finds its element by place. An element
    /// whose id, place or words differ from the drawn row sends an
    /// agent's tap to another row, or shows it words the user never
    /// sees. Glance pages 10, 6 and 4 rows high cut blocks down to their
    /// heading or less, which once put QUEUE's elements over NEXT.
    #[test]
    fn dump_lists_ids_and_places_as_drawn() {
        let mut board = make_queue_board();
        let list = board.dump(40, 16, None);
        board.handle_input(&tap_on(&list.lines, "Retry a failed"));
        let details = board.dump(40, 16, None);
        let short_glances = [10, 6, 4].map(|rows| make_sample_board().dump(40, rows, None));

        let ids: Vec<&str> = list
            .elements
            .iter()
            .filter(|element| !element.hidden)
            .map(|element| element.id.as_str())
            .collect();
        let expected = [
            "back",
            "count:run",
            "count:you",
            "count:idle",
            "group:needs-you",
            "question:Deploy tonight?",
            "session:blocked-fix",
            "group:active",
            "card:DEMO-KB-001",
            "card:DEMO-KB-002",
            "card:DEMO-KB-003",
            "card:DEMO-KB-004",
            "more:active",
            "group:in-review",
            "card:DEMO-KB-010",
            "group:todo",
            "group:draft",
            "hint",
        ];
        assert_eq!(ids, expected);
        assert_eq!(details.page, "details");
        for dump in [&list, &details].into_iter().chain(&short_glances) {
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
        let mut board = make_queue_board();

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
            ["more:active", "card:DEMO-KB-005", "more", "tap more:active"],
            ["group:todo", "card:DEMO-KB-020", "fold", "tap group:todo"],
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
        let (mut board, lines) = draw_short_queue_view(make_sample_queue(), 5);
        board.handle_input(&tap_on(&lines, "1 run"));

        let dump = board.dump(40, 5, None);

        let read_hiding = |id: &str| {
            let element = dump.elements.iter().find(|element| element.id == id);
            let hiding = element.and_then(|element| element.hiding.as_ref()).unwrap();
            (hiding.hidden_by.word(), hiding.show.as_str())
        };
        assert_eq!(
            read_hiding("question:Deploy tonight?"),
            ("scroll", "press pageup")
        );
        assert_eq!(read_hiding("card:DEMO-KB-005"), ("more", "press pagedown"));
        assert_eq!(read_hiding("group:draft"), ("scroll", "press pagedown"));
        assert_eq!(read_hiding("card:OPS-KB-001"), ("fold", "press pagedown"));
    }

    /// Raw mode turns Ctrl-C into a key, so a board that misses it
    /// leaves the user no way out but closing the pane. On a view, q
    /// and Esc go back to the glance page, and on a details page they
    /// and a tap go back to the view, as the bottom lines say. A user
    /// who only meant to close a page must not lose the board. A phone's
    /// Esc is easy to hit by mistake, so Esc on the glance page does
    /// nothing.
    #[test]
    fn handle_input_quits_or_goes_back() {
        struct Case {
            name: &'static str,
            /// The keys that reach the page the case starts on.
            reach: &'static [KeyCode],
            input: Event,
            expected: Step,
            top_line: &'static str,
        }
        let glance_top = "read 10:42 0.6s";
        let queue_top = "< queue  1 run · 2 you · 1 idle";
        let details: &[KeyCode] = &[KeyCode::Char('2'), KeyCode::Char('j'), KeyCode::Enter];
        let ctrl_c = Event::Key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
        let tap = Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 0,
            row: 5,
            modifiers: KeyModifiers::NONE,
        });
        let cases = [
            Case {
                name: "ctrl-c-on-view",
                reach: &[KeyCode::Char('2')],
                input: ctrl_c,
                expected: Step::Quit,
                top_line: queue_top,
            },
            Case {
                name: "q-on-glance",
                reach: &[],
                input: press(KeyCode::Char('q')),
                expected: Step::Quit,
                top_line: glance_top,
            },
            Case {
                name: "esc-on-glance",
                reach: &[],
                input: press(KeyCode::Esc),
                expected: Step::Stay,
                top_line: glance_top,
            },
            Case {
                name: "q-on-view",
                reach: &[KeyCode::Char('2')],
                input: press(KeyCode::Char('q')),
                expected: Step::Stay,
                top_line: glance_top,
            },
            Case {
                name: "esc-on-view",
                reach: &[KeyCode::Char('2')],
                input: press(KeyCode::Esc),
                expected: Step::Stay,
                top_line: glance_top,
            },
            Case {
                name: "q-on-details",
                reach: details,
                input: press(KeyCode::Char('q')),
                expected: Step::Stay,
                top_line: queue_top,
            },
            Case {
                name: "esc-on-details",
                reach: details,
                input: press(KeyCode::Esc),
                expected: Step::Stay,
                top_line: queue_top,
            },
            Case {
                name: "tap-on-details",
                reach: details,
                input: tap,
                expected: Step::Stay,
                top_line: queue_top,
            },
        ];

        for case in cases {
            let mut board = make_sample_board();
            for code in case.reach {
                board.handle_input(&press(*code));
            }

            let step = board.handle_input(&case.input);
            let lines = board.render_lines(40, 16);

            assert_eq!(step, case.expected, "{}", case.name);
            assert_eq!(lines[0], case.top_line, "{}", case.name);
        }
    }

    /// The board reads again every 10 seconds. A read that moved the
    /// choice would pull the row away while the user reads it, and a
    /// press of Enter would open another item.
    #[test]
    fn show_reading_keeps_the_chosen_row() {
        let mut board = make_queue_board();
        let lines = board.render_lines(40, 16);
        board.handle_input(&tap_on(&lines, "blocked-fix"));
        board.handle_input(&press(KeyCode::Esc));
        let answered = Queue {
            questions: Vec::new(),
            ..make_sample_queue()
        };

        board.show_reading(Ok(answered));
        board.handle_input(&press(KeyCode::Enter));
        let details = board.render_lines(40, 16);

        assert_eq!(details[1], "blocked-fix waits on you: needs you");
    }

    /// Keys 1 to 5 open each view full screen, under a breadcrumb that
    /// names it. A key that missed a view would hide the sessions or
    /// gauges from a phone user, who has no wide screen to see them on.
    #[test]
    fn handle_input_digits_open_each_view() {
        let mut board = make_sample_board();
        let mut tops = Vec::new();

        for digit in ['1', '2', '3', '4', '5'] {
            board.handle_input(&press(KeyCode::Char(digit)));
            tops.push(board.render_lines(40, 16)[0].clone());
        }

        let expected = [
            "< away  newest",
            "< queue  1 run · 2 you · 1 idle",
            "< next",
            "< sessions",
            "< machine",
        ];
        assert_eq!(tops, expected);
    }

    /// Each page has a help page on `?` that says what its words mean,
    /// and Esc goes back to the page it came from. Help that opened on
    /// the wrong page, or a way back that lost the view, would leave a
    /// new user lost.
    #[test]
    fn handle_input_question_mark_opens_view_help() {
        let mut board = make_queue_board();

        board.handle_input(&press(KeyCode::Char('?')));
        let help = board.render_lines(40, 16);
        board.handle_input(&press(KeyCode::Esc));
        let back = board.render_lines(40, 16);

        assert_eq!(help[..2], ["< back", "QUEUE"]);
        assert_eq!(back[0], "< queue  1 run · 2 you · 1 idle");
    }

    /// From 100 columns the three blocks sit side by side, with two
    /// blank columns and a `│` between them. A block drawn over another
    /// would hide what the user opened a wide screen to see.
    #[test]
    fn render_lines_blocks_side_by_side_when_wide() {
        let mut board = make_sample_board();

        let lines = board.render_lines(120, 16);

        let headings: Vec<&str> = lines[1].split(" │ ").map(str::trim_end).collect();
        assert_eq!(
            headings,
            [
                "AWAY  newest                       [1]",
                "QUEUE  1 run · 2 you · 1 idle      [2]",
                "NEXT                               [3]",
            ]
        );
        assert!(lines[2].starts_with(" 10:40 api-build reported "));
        assert!(lines[2].ends_with(" │  you: answer DEMO-KB-001 Deploy tonig~"));
    }

    /// A read can move rows under a finger, so a tap on a session row
    /// must only choose it, and only a tap on the bar that names it
    /// jumps. A row that jumped on a tap would move the user's screen
    /// to whichever session slid under the finger.
    #[test]
    fn handle_input_session_tap_chooses_bar_jumps() {
        let mut board = make_sample_board();
        board.handle_input(&press(KeyCode::Char('4')));
        let before = board.render_lines(40, 16);

        let row_tap = board.handle_input(&tap_on(&before, "docs-fix"));
        let chosen = board.render_lines(40, 16);
        let bar_tap = board.handle_input(&tap_on(&chosen, "-> jump"));

        assert_eq!(before[14], "tap a session to choose it");
        assert_eq!(row_tap, Step::Stay);
        assert_eq!(chosen[14], "-> jump to docs-fix");
        assert_eq!(bar_tap, Step::Jump("docs-fix".to_owned()));
    }

    /// A held key repeats faster than `REST`, so Enter right after
    /// other input must not jump, or a held j and Enter would move the
    /// user's screen to a session they never meant to open.
    #[test]
    fn handle_input_at_enter_waits_for_rest() {
        let mut board = make_sample_board();
        let start = Instant::now();
        let quick_at = start + Duration::from_millis(300);
        board.handle_input_at(&press(KeyCode::Char('4')), start);
        board.handle_input_at(&press(KeyCode::Char('j')), start);

        let quick = board.handle_input_at(&press(KeyCode::Enter), quick_at);
        let rested = board.handle_input_at(&press(KeyCode::Enter), quick_at + REST);

        assert_eq!(quick, Step::Stay);
        assert_eq!(rested, Step::Jump("blocked-fix".to_owned()));
    }

    /// The clean pass closes sessions, so the chosen one can be gone at
    /// the next read. The bar must stop naming it, or Enter would try
    /// to jump to a tab that no longer exists.
    #[test]
    fn show_reading_drops_chosen_session_once_gone() {
        let mut board = make_sample_board();
        board.handle_input(&press(KeyCode::Char('4')));
        let before = board.render_lines(40, 16);
        board.handle_input(&tap_on(&before, "docs-fix"));
        let mut later = make_sample_queue();
        later.sessions.retain(|row| row.name != "docs-fix");

        board.show_reading(Ok(later));
        let after = board.render_lines(40, 16);

        assert_eq!(after[14], "tap a session to choose it");
    }

    /// c must show what y would close and what it would park, so the
    /// user sees what y does before pressing it. A dialog that hid the
    /// parks would let y park sessions the user never saw.
    #[test]
    fn handle_input_c_lists_sessions_to_close() {
        let mut board = make_sample_board();

        let step = board.handle_input(&press(KeyCode::Char('c')));
        let lines = board.render_lines(40, 16);

        assert_eq!(step, Step::Stay);
        assert_eq!(
            lines[..3],
            [
                "close 1  park 1",
                "old-search          close",
                "ship-docs           park"
            ]
        );
        assert_eq!(lines[15], "y close and park  n cancel");
    }

    /// With nothing safe to close, c must change nothing and say why.
    /// An empty dialog would let y start a clean pass the user saw
    /// nothing of.
    #[test]
    fn handle_input_c_refuses_when_nothing_closes() {
        let mut queue = make_sample_queue();
        queue
            .sessions
            .retain(|row| !["old-search", "ship-docs"].contains(&row.name.as_str()));
        let mut board = Board::new(false);
        board.show_reading(Ok(queue));
        board.render_lines(40, 16);

        board.handle_input(&press(KeyCode::Char('c')));
        let dump = board.dump(40, 16, None);

        assert_eq!(dump.page, "list");
        assert_eq!(dump.lines[0], "nothing to close");
    }

    /// The dialog does not scroll, so when its rows pass the screen's
    /// end the last row must count the rest. Otherwise y would close
    /// sessions the user never saw listed.
    #[test]
    fn render_lines_clean_dialog_counts_hidden_rows() {
        let mut queue = make_sample_queue();
        queue.sessions.extend((1..=15).map(|number| SessionRow {
            name: format!("old-{number:02}"),
            state: Lifecycle::ToClose,
            why: "card done".to_owned(),
            stage: Some(Stage::Done),
            context_tokens: None,
        }));
        let mut board = Board::new(false);
        board.show_reading(Ok(queue));
        board.render_lines(40, 16);

        board.handle_input(&press(KeyCode::Char('c')));
        let lines = board.render_lines(40, 16);

        assert_eq!(lines[0], "close 16  park 1");
        assert_eq!(lines[13], "old-11              close");
        assert_eq!(lines[14], "+ 4 more");
    }

    /// Only a deliberate y closes sessions. A y typed ahead within
    /// `CONFIRM_WAIT` must do nothing, and so must a held y, whose
    /// repeats never leave the `REST` between them.
    #[test]
    fn handle_input_y_cleans_only_when_rested() {
        let listed = vec!["old-search".to_owned(), "ship-docs".to_owned()];
        let millis = Duration::from_millis;
        let cases = [
            ("y-too-soon", vec![millis(900)], Step::Stay, "confirm"),
            (
                "held-y",
                vec![millis(500), millis(1000)],
                Step::Stay,
                "confirm",
            ),
            (
                "y-after-wait",
                vec![CONFIRM_WAIT],
                Step::Clean {
                    run: PathBuf::from("/runs/sample"),
                    names: listed,
                },
                "clean",
            ),
        ];

        for (name, presses, expected, page) in cases {
            let mut board = make_sample_board();
            let start = Instant::now();
            board.handle_input_at(&press(KeyCode::Char('c')), start);

            let mut step = Step::Stay;
            for after in presses {
                step = board.handle_input_at(&press(KeyCode::Char('y')), start + after);
            }

            assert_eq!(step, expected, "{name}");
            assert_eq!(board.dump(40, 16, None).page, page, "{name}");
        }
    }

    /// Any key but y, and any tap, must close the dialog, so the user
    /// can always back out of a clean they did not mean.
    #[test]
    fn handle_input_dialog_closes_on_other_input() {
        let tap = Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 0,
            row: 1,
            modifiers: KeyModifiers::NONE,
        });
        let cases = [
            ("n", press(KeyCode::Char('n'))),
            ("r", press(KeyCode::Char('r'))),
            ("tap", tap),
        ];

        for (name, input) in cases {
            let mut board = make_sample_board();
            let start = Instant::now();
            board.handle_input_at(&press(KeyCode::Char('c')), start);

            let step = board.handle_input_at(&input, start + CONFIRM_WAIT);

            assert_eq!(step, Step::Stay, "{name}");
            assert_eq!(board.dump(40, 16, None).page, "list", "{name}");
        }
    }

    /// The board reads again every 10 seconds, and while the dialog is
    /// open a read can add a session that became safe to close, or find
    /// another hub's run first. y must act only on the sessions the
    /// dialog listed, in the run they came from, or it would close or
    /// park one the user never saw.
    #[test]
    fn handle_input_y_cleans_what_dialog_listed() {
        let mut board = make_sample_board();
        let start = Instant::now();
        board.handle_input_at(&press(KeyCode::Char('c')), start);
        let mut later = make_sample_queue();
        later.run = Some(PathBuf::from("/runs/other"));
        later.sessions.push(SessionRow {
            name: "late-close".to_owned(),
            state: Lifecycle::ToClose,
            why: "card done".to_owned(),
            stage: Some(Stage::Done),
            context_tokens: None,
        });

        board.show_reading(Ok(later));
        let dialog = board.render_lines(40, 16);
        let step = board.handle_input_at(&press(KeyCode::Char('y')), start + CONFIRM_WAIT);

        assert!(!dialog.iter().any(|line| line.contains("late-close")));
        assert_eq!(
            step,
            Step::Clean {
                run: PathBuf::from("/runs/sample"),
                names: vec!["old-search".to_owned(), "ship-docs".to_owned()],
            }
        );
    }
}
