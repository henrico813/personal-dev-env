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
use jiff::tz::TimeZone;
use jiff::{SignedDuration, Timestamp};
use ratatui::Frame;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{ListState, Paragraph};

use super::away;
use super::draw::{self, ListScreen};
use super::elements::{self, Action, Element, Kind, ScreenDump};
use super::glance::{self, Glance};
use super::groups::{self, Group, Item, Target};
use super::panels::{self, GLANCE_HELP, Panel, PanelText, WIDE_COLUMNS};
use super::queue::{NO_HUB, Queue};
use super::sessions::{self, SessionGroup};
use super::since;
use crate::external::cards::{CardKey, CardNote};

/// How many items an open group shows before its `+ N more` line.
/// WAITING ON YOU and IN REVIEW show `LONG_GROUP_LIMIT`, so a phone
/// shows six questions or reviews on one screen.
const GROUP_LIMIT: usize = 4;
const LONG_GROUP_LIMIT: usize = 6;

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
    /// The dim line under an item that has a `detail`, which acts as
    /// the item does.
    Detail {
        group: Group,
        index: usize,
    },
    /// The `+ N more` line under an open group's first items.
    More {
        group: Group,
        hidden: usize,
    },
    /// The line under WAITING ON YOU when no hub runs.
    NoHub,
    /// Page 1's last line, `then: review N cards`, which opens page 5.
    ThenReview,
    /// A block's heading on the first screen, which opens its page.
    Block(Panel),
    /// A block's fold row on the first screen, such as `+ 2 more`,
    /// which opens its page with every row the block holds.
    Fold(Panel),
    /// The first screen's fault row at this place in `Queue::problems`,
    /// which opens page 4.
    Fault(usize),
    /// The first screen's SINCE YOU LEFT row at this place in
    /// `since::group_events_by_card`, which opens page 2.
    Event(usize),
}

/// What names a line across reads, since a read can move an item to
/// another place in its group.
#[derive(PartialEq, Eq)]
enum LineId {
    Heading(Group),
    More(Group),
    /// The item's `Item::id`, such as a card key.
    Item(Group, String),
    Detail(Group, String),
    NoHub,
    ThenReview,
    Block(Panel),
    Fold(Panel),
    Fault(usize),
    /// The row's `CardEvents::format_row_id`.
    Event(String),
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
    /// The glance page's chosen line while a view is open, so going
    /// back chooses it again.
    glance_choice: Option<LineId>,
    /// How many rows each glance page column is scrolled down, as the
    /// last draw left them, so the chosen row shows.
    glance_scroll: [usize; 2],
    /// How many lines of page 2 are scrolled off the top.
    scroll: usize,
    /// The start of the user's visit, which SINCE YOU LEFT shows events
    /// after, or `None` with no saved time.
    since: Option<Timestamp>,
    /// `since` in the board's time zone, such as `19:07`, for page 2's
    /// breadcrumb.
    since_time: Option<String>,
    /// The page 2 rows shown open, by `CardEvents::format_row_id`.
    open_cards: BTreeSet<String>,
    /// True when page 4 shows the kept, safe to close and parked
    /// sessions under its fold row.
    show_done: bool,
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
            // Idle and Draft hold most cards, and Blocked holds nothing
            // for the user to do, so they start folded. The first draw
            // at full width opens Idle, as `fold_idle_to_width` says.
            open: BTreeSet::from([
                Group::NeedsYou,
                Group::Working,
                Group::NoWorker,
                Group::Todo,
                Group::InReview,
                Group::OtherPrs,
            ]),
            shown_in_full: BTreeSet::new(),
            lines: Vec::new(),
            list: ListState::default(),
            page: Page::List,
            view: None,
            glance_choice: None,
            glance_scroll: [0; 2],
            scroll: 0,
            since: None,
            since_time: None,
            open_cards: BTreeSet::new(),
            show_done: false,
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
        self.status = format!("read {} {:.1}s", queue.read_at, queue.took.as_secs_f32());
        self.rebuild_keeping_choice(|board| {
            board.groups = groups::build_groups(&queue);
            // A session gone since the last read can no longer be jumped to.
            board.chosen_session = board
                .chosen_session
                .take()
                .filter(|name| queue.sessions.iter().any(|row| row.name == *name));
            board.queue = Some(queue);
        });
    }

    /// Shows a read a board saved before, with its time on top, until
    /// this board's first read replaces it.
    pub(crate) fn show_saved_reading(&mut self, queue: Queue) {
        let saved_at = queue.read_at.clone();
        self.show_reading(Ok(queue));
        self.status = format!("saved {saved_at} · reading");
    }

    /// Shows the events after `since` in SINCE YOU LEFT, with `since` in
    /// `time_zone` on page 2's breadcrumb, and `away_for`, how long the
    /// user was away, on the glance page's top line. With `since` `None`,
    /// SINCE YOU LEFT shows the newest events.
    pub(crate) fn show_visit(
        &mut self,
        since: Option<Timestamp>,
        away_for: Option<SignedDuration>,
        time_zone: &TimeZone,
    ) {
        self.since_time =
            since.map(|at| at.to_zoned(time_zone.clone()).strftime("%H:%M").to_string());
        self.away_for = away_for;
        // SINCE YOU LEFT's rows on the glance page are lines.
        self.rebuild_keeping_choice(|board| board.since = since);
    }

    /// Makes `change` to what the lines come from and builds them again,
    /// keeping the chosen line when it is still there, else its place.
    /// Every change to the lines' sources goes through here, so a tap or
    /// Enter never acts on a line the screen no longer draws.
    fn rebuild_keeping_choice(&mut self, change: impl FnOnce(&mut Board)) {
        let chosen = self.list.selected();
        let chosen_id = chosen.and_then(|index| self.find_line_id(index));
        change(self);
        self.rebuild_lines();
        let same_line = chosen_id.and_then(|id| self.find_line_by_id(&id));
        let last = self.lines.len().saturating_sub(1);
        let place = same_line.unwrap_or_else(|| chosen.unwrap_or(0).min(last));
        self.list.select((!self.lines.is_empty()).then_some(place));
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
        let found = self.queue.iter().find_map(|queue| {
            let card = queue.cards.iter().find(|card| card.key == *key)?;
            Some((card, &queue.sessions))
        });
        if let Some((card, sessions)) = found {
            details.lines = groups::describe_card(card, sessions, note.as_ref().ok());
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
        let was_wide = self.area.width >= WIDE_COLUMNS;
        self.area = frame.area();
        self.fold_idle_to_width(was_wide);
        if let Page::Details(details) | Page::Cleaned(details) = &mut self.page {
            draw::draw_details(frame, details);
            return;
        }
        if let Page::Confirm(_) = self.page {
            self.draw_confirm(frame);
            return;
        }
        match self.view {
            None => {
                let scroll = glance::draw_glance(frame, self.area, &self.build_glance());
                self.glance_scroll = scroll;
            }
            Some(panel) => self.draw_view(frame, panel),
        }
    }

    /// Opens IDLE when the screen grows to `WIDE_COLUMNS` columns and
    /// folds it when it shrinks below, since only the full screen has
    /// room for its rows. A tap on its heading still folds or opens it
    /// until the width crosses again.
    fn fold_idle_to_width(&mut self, was_wide: bool) {
        let is_wide = self.area.width >= WIDE_COLUMNS;
        if is_wide == was_wide {
            return;
        }
        // The glance page's lines also change with the width, since it
        // shows more rows from `WIDE_COLUMNS` on.
        self.rebuild_keeping_choice(|board| {
            if is_wide {
                board.open.insert(Group::Idle);
            } else {
                board.open.remove(&Group::Idle);
            }
        });
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
            Some(panel @ (Panel::Waiting | Panel::Active | Panel::Review)) => {
                draw::build_list_elements(&self.build_list_screen(panel), self.area, &self.list)
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
            since_time: self.since_time.as_deref(),
            away_for: self.away_for,
            chosen: self.list.selected(),
            scroll: self.glance_scroll,
            color: self.color,
        }
    }

    /// Returns page 2's or page 4's breadcrumb, such as
    /// `< 2 SINCE YOU LEFT AT 19:07  7 cards`. Page 2 says `newest` in
    /// place of the time and count when no visit time was saved.
    fn format_view_title(&self, panel: Panel) -> String {
        let title = draw::format_page_title(panel);
        if panel != Panel::Since {
            return title;
        }
        match (&self.since_time, &self.queue) {
            (Some(time), Some(queue)) => {
                let count = since::group_events_by_card(self.select_away_events(queue)).len();
                let cards = if count == 1 { "card" } else { "cards" };
                format!("{title} AT {time}  {count} {cards}")
            }
            _ => format!("{title}  newest"),
        }
    }

    /// Returns the events SINCE YOU LEFT shows.
    fn select_away_events<'a>(&self, queue: &'a Queue) -> &'a [away::AwayEvent] {
        away::find_events_since(&queue.events, self.since)
    }

    /// Returns page 2's lines, `width` columns wide, each with the row
    /// id of the card it names. A card row, such as
    /// `> DEMO-KB-001 Faster search  report`, ends in its newest event's
    /// word, and an open one in its event count above a line per event,
    /// such as `   10:40 report`.
    fn format_since_lines(&self, width: usize) -> Vec<(PanelText, Option<String>)> {
        let Some(queue) = &self.queue else {
            return Vec::new();
        };
        let make_text = |is_heading, text: String, time: Option<&str>, word: &str| {
            let text = sessions::fit(&text, width);
            PanelText {
                is_heading,
                is_high: false,
                is_ok: false,
                colored: Some(draw::build_event_line(&text, time, word, self.color)),
                text,
            }
        };
        let mut lines = Vec::new();
        for row in since::group_events_by_card(self.select_away_events(queue)) {
            let id = row.format_row_id();
            let is_open = self.open_cards.contains(&id);
            let (mark, end) = if is_open {
                let count = row.events.len();
                let events = if count == 1 { "event" } else { "events" };
                ("v", format!("{count} {events}"))
            } else {
                (">", since::describe_event(row.events[0]).to_owned())
            };
            let name = format!("{mark} {}", row.describe_card(&queue.cards));
            let text = glance::join_row_end(&name, &end, width);
            lines.push((make_text(true, text, None, &end), Some(id)));
            if is_open {
                lines.extend(row.events.iter().map(|event| {
                    let word = since::describe_event(event);
                    let text = format!("   {} {word}", event.time);
                    (make_text(false, text, Some(&event.time), word), None)
                }));
            }
        }
        lines
    }

    /// Returns page 4's fault and gauge rows, `width` columns wide.
    fn format_machine_rows(&self, width: usize) -> Vec<PanelText> {
        self.queue.as_ref().map_or_else(Vec::new, |queue| {
            panels::format_machine_lines(&queue.problems, &queue.gauges, queue.no_hub, width)
        })
    }

    /// Draws `panel`'s page over the whole frame. Pages 1, 3 and 5 are
    /// lists. Pages 2 and 4 have the breadcrumb on top and the bottom
    /// hint, with page 4's fault and gauge rows above its sessions.
    fn draw_view(&mut self, frame: &mut Frame, panel: Panel) {
        if let Panel::Waiting | Panel::Active | Panel::Review = panel {
            // Built field by field, since `self.list` is borrowed
            // mutably beside it.
            let screen = ListScreen {
                panel,
                groups: &self.groups,
                lines: &self.lines,
                queue: self.queue.as_ref(),
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
        if panel == Panel::Machine {
            let machine = self.format_machine_rows(usize::from(body.width));
            let [top, below] = split_rows_off_top(body, machine.len());
            panels::draw_panel_text(frame, top, &machine, 0, self.color);
            let rows = self.format_session_rows(usize::from(below.width));
            self.session_list.select(self.find_chosen_line(&rows));
            sessions::draw_sessions(
                frame,
                below,
                &rows,
                self.chosen_session.as_deref(),
                &mut self.session_list,
            );
            return;
        }
        let (lines, _): (Vec<PanelText>, Vec<_>) = self
            .format_since_lines(usize::from(body.width))
            .into_iter()
            .unzip();
        // Stop the scroll where the last line shows at the bottom.
        let last_top = lines.len().saturating_sub(usize::from(body.height));
        self.scroll = self.scroll.min(last_top);
        panels::draw_panel_text(frame, body, &lines, self.scroll, self.color);
    }

    /// Returns the elements of page 2 or page 4, as `draw_view` drew
    /// them. A page 2 card row taps `unfold` or `fold`.
    fn build_view_elements(&self, panel: Panel) -> Vec<Element> {
        let [title, body, hint] = draw::split_screen(self.area);
        let mut elements = vec![draw::build_back_element(
            &self.format_view_title(panel),
            title,
        )];
        if panel == Panel::Machine {
            let machine = self.format_machine_rows(usize::from(body.width));
            let [top, below] = split_rows_off_top(body, machine.len());
            elements.extend(panels::build_text_elements(panel, &machine, top, 0));
            let rows = self.format_session_rows(usize::from(below.width));
            let sessions = self.queue.as_ref().map_or(&[][..], |queue| &queue.sessions);
            elements.extend(sessions::build_session_elements(
                &rows,
                sessions,
                below,
                &self.session_list,
                self.chosen_session.as_deref(),
                self.show_done,
            ));
        } else {
            let (lines, ids): (Vec<PanelText>, Vec<Option<String>>) = self
                .format_since_lines(usize::from(body.width))
                .into_iter()
                .unzip();
            let texts = panels::build_text_elements(panel, &lines, body, self.scroll);
            elements.extend(texts.into_iter().zip(ids).enumerate().map(
                |(number, (element, id))| match id {
                    Some(id) => Element {
                        kind: Kind::Heading,
                        tap: Some(if self.open_cards.contains(&id) {
                            "fold"
                        } else {
                            "unfold"
                        }),
                        action: Action::Toggle(number),
                        ..element
                    },
                    None => element,
                },
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
            // The fault and gauge rows sit on top, and the jump bar takes
            // the body's last row.
            (Page::List, Some(Panel::Machine)) => {
                let machine = u16::try_from(self.format_machine_rows(0).len()).unwrap_or(rows);
                rows.saturating_sub(machine + 1)
            }
            _ => rows,
        };
        isize::try_from(rows).unwrap_or(1).max(1)
    }

    /// Opens `panel`'s page over the glance page, at its top, with its
    /// first line chosen.
    fn open_view(&mut self, panel: Panel) {
        if self.view.is_none() {
            self.glance_choice = self
                .list
                .selected()
                .and_then(|index| self.find_line_id(index));
        }
        self.view = Some(panel);
        self.scroll = 0;
        self.rebuild_lines();
        self.list = ListState::default();
        self.list.select((!self.lines.is_empty()).then_some(0));
    }

    /// Goes back to the glance page, with the line chosen there before
    /// the view opened chosen again, else its first line.
    fn close_view(&mut self) {
        self.view = None;
        self.rebuild_lines();
        let place = self
            .glance_choice
            .take()
            .and_then(|id| self.find_line_by_id(&id));
        self.list = ListState::default();
        self.list
            .select((!self.lines.is_empty()).then_some(place.unwrap_or(0)));
    }

    /// Opens `panel`'s page with every row its glance block holds shown,
    /// its groups open and past `GROUP_LIMIT`, so the rows the block's
    /// fold row sums up show on it. On page 3 that opens IDLE. They stay
    /// shown when the page opens again, as after `+ N more` on a page.
    fn open_view_with_block_rows(&mut self, panel: Panel) {
        let groups = glance::list_block_groups(panel);
        self.open.extend(groups);
        self.shown_in_full.extend(groups);
        self.open_view(panel);
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
                match self
                    .view
                    .map_or(Some(Panel::Waiting), Panel::find_next_panel)
                {
                    Some(panel) => self.open_view(panel),
                    None => self.close_view(),
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
                self.close_view();
                return Step::Stay;
            }
            KeyCode::Char('q') => return Step::Quit,
            _ => {}
        }
        match self.view {
            // The glance page picks its lines as the list pages do. Esc
            // on it does nothing, since a phone's Esc is easy to hit by
            // mistake.
            None | Some(Panel::Waiting | Panel::Active | Panel::Review) => {}
            Some(Panel::Since) => {
                match key.code {
                    KeyCode::Char('j') | KeyCode::Down => self.scroll(1),
                    KeyCode::Char('k') | KeyCode::Up => self.scroll(-1),
                    KeyCode::PageDown => self.scroll(page_size),
                    KeyCode::PageUp => self.scroll(-page_size),
                    _ => Step::Stay,
                };
                return Step::Stay;
            }
            Some(Panel::Machine) => {
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

    /// Acts on the element under a tap. A breadcrumb goes back, a list
    /// or glance line acts as Enter does, a page 2 card row or page 4's
    /// fold row folds or unfolds, a session row only chooses, and the
    /// bar jumps. Any tap on another page goes back to the list.
    fn tap(&mut self, column: u16, row: u16) -> Step {
        if let Page::Details(_) | Page::Cleaned(_) | Page::Confirm(_) = self.page {
            self.page = Page::List;
            return Step::Stay;
        }
        let elements = self.collect_elements();
        let action =
            elements::find_element_at(&elements, column, row).map(|element| element.action);
        match action {
            Some(Action::Line(index)) => self.activate(index),
            Some(Action::Toggle(line)) => {
                self.toggle_row(line);
                Step::Stay
            }
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
                self.close_view();
                Step::Stay
            }
            Some(Action::None) | None => Step::Stay,
        }
    }

    /// Chooses the line at `index` and acts on it. A heading folds or
    /// opens, `+ N more` shows the rest, an item or its detail line
    /// opens its details, and `then: review` opens page 5. On the
    /// glance page a block's heading, a fault row or a SINCE YOU LEFT
    /// row opens its page, and a fold row opens its page with the rows
    /// it sums up shown.
    fn activate(&mut self, index: usize) -> Step {
        let Some(&line) = self.lines.get(index) else {
            return Step::Stay;
        };
        self.list.select(Some(index));
        match line {
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
            Line::Item { group, index } | Line::Detail { group, index } => {
                self.open_details(group, index)
            }
            Line::NoHub => Step::Stay,
            Line::ThenReview => {
                self.open_view(Panel::Review);
                Step::Stay
            }
            Line::Block(panel) => {
                self.open_view(panel);
                Step::Stay
            }
            Line::Fault(_) => {
                self.open_view(Panel::Machine);
                Step::Stay
            }
            Line::Event(_) => {
                self.open_view(Panel::Since);
                Step::Stay
            }
            Line::Fold(panel) => {
                self.open_view_with_block_rows(panel);
                Step::Stay
            }
        }
    }

    /// Folds or unfolds the row at `line` of the open page, a page 2
    /// card row or page 4's fold row.
    fn toggle_row(&mut self, line: usize) {
        match self.view {
            Some(Panel::Since) => {
                let row_id = self.format_since_lines(0).into_iter().nth(line);
                let Some((_, Some(id))) = row_id else {
                    return;
                };
                if !self.open_cards.remove(&id) {
                    self.open_cards.insert(id);
                }
            }
            Some(Panel::Machine) => self.show_done = !self.show_done,
            _ => {}
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

    /// Moves the choice on the glance page, a list page or page 4's
    /// sessions, scrolls page 2, or scrolls a details or clean output
    /// page.
    fn scroll(&mut self, steps: isize) -> Step {
        match &mut self.page {
            Page::List => match self.view {
                None | Some(Panel::Waiting | Panel::Active | Panel::Review) => {
                    self.move_choice(steps);
                }
                Some(Panel::Machine) => {
                    self.move_session(steps);
                }
                // `draw_view` stops the scroll at the page's end.
                Some(Panel::Since) => {
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
        let order = sessions::order_shown_sessions(&queue.sessions, self.show_done);
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
            sessions::format_session_lines(&queue.sessions, width, self.show_done)
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

    /// Lists the open page's groups in `Panel::list_groups` order, each
    /// as its heading, then its first `GROUP_LIMIT` items, each with its
    /// detail line, and a `+ N more` line when it is open. Page 1 has
    /// no heading, since its breadcrumb names its one group, and it is
    /// always open. WAITING ON YOU says that no hub runs when none did
    /// at the read. Page 1 ends in `then: review N cards` when IN
    /// REVIEW holds a card. The glance page's lines are the rows it
    /// draws at the last drawn width, as `glance::list_glance_lines`
    /// lists them.
    fn rebuild_lines(&mut self) {
        if self.view.is_none() {
            self.lines = glance::list_glance_lines(self.area, &self.build_glance());
            return;
        }
        let no_hub = self.queue.as_ref().is_some_and(|queue| queue.no_hub);
        let page_groups = self.view.map_or(&[][..], Panel::list_groups);
        let mut lines = Vec::new();
        for group in page_groups {
            let Some((_, items)) = self.groups.iter().find(|(each, _)| each == group) else {
                continue;
            };
            let on_page_one = self.view == Some(Panel::Waiting);
            if !on_page_one {
                let is_open = self.open.contains(group);
                lines.push(Line::Heading {
                    group: *group,
                    is_open,
                });
                if !is_open {
                    continue;
                }
            }
            if *group == Group::NeedsYou && no_hub {
                lines.push(Line::NoHub);
            }
            let limit = match group {
                Group::NeedsYou | Group::InReview => LONG_GROUP_LIMIT,
                _ => GROUP_LIMIT,
            };
            let shown = if self.shown_in_full.contains(group) {
                items.len()
            } else {
                items.len().min(limit)
            };
            for (index, item) in items.iter().enumerate().take(shown) {
                lines.push(Line::Item {
                    group: *group,
                    index,
                });
                if item.detail.is_some() {
                    lines.push(Line::Detail {
                        group: *group,
                        index,
                    });
                }
            }
            if shown < items.len() {
                lines.push(Line::More {
                    group: *group,
                    hidden: items.len() - shown,
                });
            }
        }
        let has_review = self
            .groups
            .iter()
            .any(|(group, items)| *group == Group::InReview && !items.is_empty());
        if self.view == Some(Panel::Waiting) && has_review {
            lines.push(Line::ThenReview);
        }
        self.lines = lines;
    }

    /// Returns what list page `panel` draws from.
    fn build_list_screen(&self, panel: Panel) -> ListScreen<'_> {
        ListScreen {
            panel,
            groups: &self.groups,
            lines: &self.lines,
            queue: self.queue.as_ref(),
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
            Line::Detail { group, index } => Some(LineId::Detail(
                group,
                self.find_item(group, index)?.id.clone(),
            )),
            Line::NoHub => Some(LineId::NoHub),
            Line::ThenReview => Some(LineId::ThenReview),
            Line::Block(panel) => Some(LineId::Block(panel)),
            Line::Fold(panel) => Some(LineId::Fold(panel)),
            Line::Fault(number) => Some(LineId::Fault(number)),
            Line::Event(number) => {
                let queue = self.queue.as_ref()?;
                let rows = since::group_events_by_card(self.select_away_events(queue));
                Some(LineId::Event(rows.get(number)?.format_row_id()))
            }
        }
    }

    /// Returns the place of the line that `id` names, or `None` when no
    /// line does.
    fn find_line_by_id(&self, id: &LineId) -> Option<usize> {
        (0..self.lines.len()).find(|&index| self.find_line_id(index).as_ref() == Some(id))
    }
}

/// Returns `area` split into its top `rows` rows and the rest, which
/// keeps at least one row.
fn split_rows_off_top(area: Rect, rows: usize) -> [Rect; 2] {
    let rows = u16::try_from(rows).unwrap_or(u16::MAX);
    Layout::vertical([Constraint::Length(rows), Constraint::Min(1)]).areas(area)
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;

    use std::time::Duration;

    use crossterm::event::{MouseButton, MouseEvent};
    use jiff::civil::date;
    use ratatui::buffer::Buffer;
    use ratatui::style::Color;

    use crate::board::away::AwayEvent;
    use crate::board::machine::{Gauges, Usage};
    use crate::board::pull_requests::{CheckResult, PullRequestLine};
    use crate::board::queue::{CardLine, SessionLine, SessionRow};
    use crate::external::cards::Stage;
    use crate::shared::lifecycle::{KeptReason, Lifecycle};
    use crate::shared::questions::OpenQuestion;

    /// A queue with five cards in progress, one needing the user's
    /// review, a question, a session that waits on the user as the
    /// worker of DEMO-KB-003, one card ready to start and one the user
    /// has not refined. Its sessions fill every session group, one
    /// waiting on the hub and one to park. Its watch gauge is
    /// past 90%, and its events are two reports and a `notes.md` line,
    /// the newest at `10:40`. It has no pull request list and no board
    /// limits.
    pub(crate) fn make_sample_queue() -> Queue {
        let make_session = |name: &str, state, why: &str, stage, tokens| {
            SessionRow::new(name.to_owned(), state, why.to_owned(), stage, tokens)
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
            read_on: date(2026, 11, 15),
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
                number: 1,
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
            events: vec![
                make_event("10:40", "api-build reported"),
                make_event("10:31", "launched card-sync on DEMO-KB-002"),
                make_event("10:12", "docs-fix reported"),
            ],
            no_hub: false,
            run: Some(PathBuf::from("/runs/sample")),
            notes: [("DEMO-KB-010".parse().unwrap(), reviewed_note)].into(),
            problems: Vec::new(),
            pull_requests: None,
            board: None,
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

    /// Returns `make_sample_board` with page 3, ACTIVE, open, drawn once
    /// at 40x16.
    pub(crate) fn make_active_board() -> Board {
        let mut board = make_sample_board();
        board.handle_input(&press(KeyCode::Char('3')));
        board.render_lines(40, 16);
        board
    }

    /// Returns `make_sample_board` with page `digit` open, drawn once at
    /// 40x16.
    fn open_sample_page(digit: char) -> (Board, Vec<String>) {
        let mut board = make_sample_board();
        board.handle_input(&press(KeyCode::Char(digit)));
        let lines = board.render_lines(40, 16);
        (board, lines)
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

    /// The phone's first screen, 40 by 16, must show with no key
    /// pressed what waits on the user, what happened since the last
    /// visit, what is at work and what is in review, in that order,
    /// with each block's key. A fault comes first, so the user sees a
    /// stopped watcher before trusting the rows below it. A block cut
    /// off, or a fold row that miscounts, hides work from the user.
    #[test]
    fn render_lines_glance_shows_faults_then_blocks() {
        let mut queue = make_sample_queue();
        queue.problems = vec!["hub watcher stopped".to_owned()];
        let mut board = Board::new(false);
        board.show_reading(Ok(queue));

        let lines = board.render_lines(40, 16);

        let expected = [
            "read 10:42 0.6s",
            "! hub watcher stopped",
            "WAITING ON YOU  2                    [1]",
            " blocked-fix                     see tab",
            " DEMO-KB-001 Faster search        answer",
            "",
            "SINCE YOU LEFT  newest               [2]",
            " 10:40 hub                        report",
            "   + 1 other",
            "ACTIVE  5 · 1 working                [3]",
            " DEMO-KB-001 Faster search",
            " DEMO-KB-004 Lint every Dockerfile in t~",
            "   + 3 more, none at work",
            "IN REVIEW  1                         [5]",
            "",
            "4 sessions and machine            ? help",
        ];
        assert_eq!(lines, expected);
    }

    /// SINCE YOU LEFT must show only what came after the user's last
    /// visit, with the visit's time. Older events listed as new would
    /// bury the news under what the user already read.
    #[test]
    fn render_lines_glance_shows_events_since_visit() {
        let mut board = Board::new(false);
        board.show_reading(Ok(make_sample_queue()));
        let since = "2026-11-15T10:31:00Z".parse().ok();

        board.show_visit(since, Some(SignedDuration::from_mins(89)), &TimeZone::UTC);
        let lines = board.render_lines(40, 16);

        assert_eq!(
            lines[5..7],
            [
                "SINCE YOU LEFT AT 10:31  1 card      [2]",
                " 10:40 hub                        report",
            ]
        );
    }

    /// A phone in landscape, or a pane split in two, can leave the board
    /// 10 rows. WAITING ON YOU comes first, so it keeps its rows and the
    /// blocks below it are cut. A layout that cut WAITING ON YOU would
    /// hide the questions the user opened the board for.
    #[test]
    fn render_lines_short_glance_keeps_waiting_rows() {
        let mut board = make_sample_board();

        let lines = board.render_lines(40, 10);

        let expected = [
            "read 10:42 0.6s",
            "WAITING ON YOU  2                    [1]",
            " blocked-fix                     see tab",
            " DEMO-KB-001 Faster search        answer",
            "",
            "SINCE YOU LEFT  newest               [2]",
            " 10:40 hub                        report",
            "   + 1 other",
            "ACTIVE  5 · 1 working                [3]",
            "4 sessions and machine            ? help",
        ];
        assert_eq!(lines, expected);
    }

    /// Returns `make_sample_queue` with a fold row in two blocks.
    /// WAITING ON YOU hides an auto run's hub and one question not asked
    /// today. SINCE YOU LEFT hides a report, a new worker and a finished
    /// card.
    fn make_fold_queue() -> Queue {
        let mut queue = make_sample_queue();
        queue.questions[0].asked_on = Some(queue.read_on);
        let make_question = |key: &str, asked_on| OpenQuestion {
            key: key.parse().unwrap(),
            project: "Demo".to_owned(),
            number: 1,
            title: "Ship it?".to_owned(),
            lines: Vec::new(),
            sessions: Vec::new(),
            asked_on,
        };
        let today = Some(queue.read_on);
        let added = [
            make_question("DEMO-KB-004", today),
            make_question("DEMO-KB-005", today),
            make_question("DEMO-KB-020", None),
        ];
        queue.questions.extend(added);
        queue.waiting = vec![SessionLine {
            name: "auto-hub-24".to_owned(),
            why: "needs you".to_owned(),
            card: None,
        }];
        let make_event = |time: &str, text: &str| AwayEvent {
            at: format!("2026-11-15T{time}:00Z").parse().unwrap(),
            time: time.to_owned(),
            text: text.to_owned(),
        };
        queue.events = vec![
            make_event("10:40", "demo-kb-001-build reported"),
            make_event("10:35", "demo-kb-002-build reported"),
            make_event(
                "10:30",
                "started demo-kb-004-build (p_1) from /h/handoff.md",
            ),
            make_event("10:20", "parked demo-kb-005-build (DEMO-KB-005 in-review)"),
        ];
        queue
    }

    /// WAITING ON YOU's fold row must name what it hides, an auto
    /// run's hub by its run and a question not asked today as old.
    /// A bare count tells the user neither which run waits nor whether
    /// the hidden questions are new.
    #[test]
    fn render_lines_glance_fold_names_hidden_rows() {
        let mut board = Board::new(false);
        board.show_reading(Ok(make_fold_queue()));

        let lines = board.render_lines(40, 16);

        assert_eq!(lines[5], "   + run 24 hub and 1 old question");
    }

    /// Draws `board` at 40x16 and returns its cells, styles included.
    fn render_cells(board: &mut Board) -> Buffer {
        let mut terminal = Terminal::new(TestBackend::new(40, 16)).unwrap();
        terminal.draw(|frame| board.render(frame)).unwrap();
        terminal.backend().buffer().clone()
    }

    /// Returns the color and dimness of the first cell of `words`, on
    /// the first row of `cells` that holds them.
    fn find_words_style(cells: &Buffer, words: &str) -> (Option<Color>, bool) {
        let style = find_words_cell_style(cells, words);
        (style.fg, style.add_modifier.contains(Modifier::DIM))
    }

    /// Returns the style of the first cell of `words`, on the first row
    /// of `cells` that holds them.
    fn find_words_cell_style(cells: &Buffer, words: &str) -> Style {
        for row in 0..cells.area.height {
            let line: String = (0..cells.area.width)
                .map(|column| cells[(column, row)].symbol())
                .collect();
            if let Some(start) = line.find(words) {
                let column = u16::try_from(line[..start].chars().count()).unwrap();
                return cells[(column, row)].style();
            }
        }
        panic!("no row holds {words:?}");
    }

    /// With color on, the fold rows' kind words must take the colors
    /// their rows use, yellow for old questions, cyan for new workers
    /// and green for finished cards. The `+`, the run's name and
    /// `report` stay dim. A fold row all in dim hides a waiting old
    /// question or a new worker among the counts the user skims past.
    #[test]
    fn render_fold_rows_color_kinds_dim_rest() {
        let mut board = Board::new(true);
        board.show_reading(Ok(make_fold_queue()));

        let cells = render_cells(&mut board);

        // The first two find the `+` of WAITING ON YOU's fold row, then
        // the `r` of the plain words after it.
        let styles = [
            "+ run 24 hub",
            "run 24 hub",
            "1 old question",
            "1 report",
            "1 new worker",
            "1 finished",
        ]
        .map(|words| find_words_style(&cells, words));
        assert_eq!(
            styles,
            [
                (Color::Reset, true),
                (Color::Reset, true),
                (Color::Yellow, false),
                (Color::Reset, true),
                (Color::Cyan, false),
                (Color::Green, false),
            ]
            .map(|(color, is_dim)| (Some(color), is_dim))
        );
    }

    /// With color off, the kind words that color would mark must stay
    /// dim like the rest of their fold row. Drawn plain, they would make
    /// a fold row look like a full row the user must read.
    #[test]
    fn render_fold_rows_dim_when_color_off() {
        let mut board = Board::new(false);
        board.show_reading(Ok(make_fold_queue()));

        let cells = render_cells(&mut board);

        let styles = ["1 old question", "1 new worker", "1 finished"]
            .map(|words| find_words_style(&cells, words));
        assert_eq!(styles, [(Some(Color::Reset), true); 3]);
    }

    /// A block shows only a few rows, so the rest must be one tap away.
    /// A tap on the block's heading opens its page under a breadcrumb,
    /// and a tap on the breadcrumb comes back to the same first screen.
    /// Without the way back, a phone user is stuck on the page.
    #[test]
    fn handle_input_block_tap_opens_breadcrumb_closes() {
        let mut board = make_sample_board();
        let glance = board.render_lines(40, 16);

        board.handle_input(&tap_on(&glance, "ACTIVE"));
        let opened = board.render_lines(40, 16);
        board.handle_input(&tap_on(&opened, "< 3 ACTIVE"));
        let back = board.render_lines(40, 16);

        assert_eq!(opened[0], "< 3 ACTIVE  5 · 1 working");
        assert_eq!(back, glance);
    }

    /// Returns the id of the element the 40x16 dump marks `chosen`, or
    /// an empty string when none is.
    fn read_chosen_id(board: &mut Board) -> String {
        let dump = board.dump(40, 16, None);
        dump.elements
            .into_iter()
            .find(|element| element.chosen)
            .map_or_else(String::new, |element| element.id)
    }

    /// j, k and the arrow keys move the first screen's choice one row
    /// at a time, as on the pages, past the blank line between blocks.
    /// A key that stuck, or stopped on the blank line, would leave a
    /// user with no mouse unable to reach the next block's rows.
    #[test]
    fn handle_input_j_k_move_glance_choice() {
        let mut board = make_sample_board();
        let (j, k) = (KeyCode::Char('j'), KeyCode::Char('k'));

        let mut chosen = Vec::new();
        for code in [j, KeyCode::Down, j, k, KeyCode::Up] {
            board.handle_input(&press(code));
            chosen.push(read_chosen_id(&mut board));
        }

        let expected = [
            "waiting:0",
            "waiting:1",
            "block:since",
            "waiting:1",
            "waiting:0",
        ];
        assert_eq!(chosen, expected);
    }

    /// When fault rows push the last block below a 40x16 screen, the
    /// first screen scrolls so the chosen row shows, drawn reversed as
    /// on the pages. Without it the user moves the choice onto a row
    /// they cannot see, and Enter acts on it blind.
    #[test]
    fn render_glance_scrolls_to_chosen_row_reversed() {
        let mut queue = make_sample_queue();
        queue.problems = ["one", "two", "three"]
            .map(|words| format!("fault {words}"))
            .to_vec();
        let mut board = Board::new(false);
        board.show_reading(Ok(queue));
        board.render_lines(40, 16);

        board.handle_input(&press(KeyCode::PageDown));
        let cells = render_cells(&mut board);

        let style = find_words_cell_style(&cells, "IN REVIEW");
        assert!(style.add_modifier.contains(Modifier::REVERSED));
    }

    /// Esc from a page comes back to the first screen with the row the
    /// user picked there still chosen. Without it every visit to a page
    /// sends the choice back to the top.
    #[test]
    fn handle_input_esc_restores_glance_choice() {
        let mut board = make_sample_board();
        board.handle_input(&press(KeyCode::Char('j')));
        board.handle_input(&press(KeyCode::Char('j')));

        board.handle_input(&press(KeyCode::Char('3')));
        board.handle_input(&press(KeyCode::Esc));

        assert_eq!(read_chosen_id(&mut board), "waiting:1");
    }

    /// From 100 columns a tap on a row in either column opens that
    /// row's details. A tap finds its line by its place in the left
    /// column then the right, so a wrong order would open another card.
    #[test]
    fn handle_input_wide_tap_opens_tapped_row() {
        let mut board = make_sample_board();
        let glance = board.render_lines(100, 24);

        board.handle_input(&tap_on(&glance, "blocked-fix"));
        let left = board.render_lines(100, 24);
        board.handle_input(&press(KeyCode::Esc));
        board.handle_input(&tap_on(&glance, "DEMO-KB-004"));
        let right = board.render_lines(100, 24);

        assert_eq!(left[1], "blocked-fix waits on you: needs you");
        assert_eq!(right[1], "Lint every Dockerfile in the repository");
    }

    /// A new visit time can drop SINCE YOU LEFT's rows from above the
    /// chosen row. The choice must stay on the row the user picked, or
    /// Enter would act on whichever row slid into its place.
    #[test]
    fn show_visit_keeps_choice_when_rows_drop() {
        let mut board = make_sample_board();
        let visit = |time: &str| format!("2026-11-15T{time}:00Z").parse().ok();
        board.show_visit(visit("10:31"), None, &TimeZone::UTC);
        for _ in 0..20 {
            if read_chosen_id(&mut board) == "block:active" {
                break;
            }
            board.handle_input(&press(KeyCode::Char('j')));
        }
        assert_eq!(read_chosen_id(&mut board), "block:active");

        board.show_visit(visit("11:00"), None, &TimeZone::UTC);

        assert_eq!(read_chosen_id(&mut board), "block:active");
    }

    /// Enter on ACTIVE's `+ N more` row opens page 3 with the rows it
    /// sums up shown, so IDLE opens. Without it the idle cards the
    /// first screen folds stay out of a keyboard user's reach.
    #[test]
    fn handle_input_enter_on_fold_opens_idle() {
        let mut board = make_sample_board();
        for _ in 0..20 {
            if read_chosen_id(&mut board) == "more:active" {
                break;
            }
            board.handle_input(&press(KeyCode::Char('j')));
        }
        assert_eq!(read_chosen_id(&mut board), "more:active");

        board.handle_input(&press(KeyCode::Enter));
        let lines = board.render_lines(40, 16);

        assert_eq!(lines[0], "< 3 ACTIVE  5 · 1 working");
        assert_eq!(
            lines[6..9],
            [
                "v IDLE 2",
                " idle DEMO-KB-002 Board for the~ Demo",
                " you  DEMO-KB-003 Trim the buil~ Demo",
            ]
        );
    }

    /// An agent learns what to open from the elements, not the screen.
    /// The rows a fold row sums up must be listed, hidden, with the key
    /// of the page that shows them, or the agent never reads them.
    #[test]
    fn dump_glance_lists_folded_rows_with_key() {
        let mut board = Board::new(false);
        board.show_reading(Ok(make_sample_queue()));

        let dump = board.dump(40, 16, None);

        let hidden: Vec<(&str, &str)> = dump
            .elements
            .iter()
            .filter_map(|element| {
                let hiding = element.hiding.as_ref()?;
                Some((element.id.as_str(), hiding.show.as_str()))
            })
            .collect();
        let expected = [
            ("since:1", "press 2"),
            ("active:2", "press 3"),
            ("active:3", "press 3"),
            ("active:4", "press 3"),
            ("review:0", "press 5"),
        ];
        assert_eq!(hidden, expected);
    }

    /// The user reads page 3 on a phone, 40 by 16. The cards at work
    /// come first, then those with no worker, while idle cards and
    /// drafts stay folded. A card whose worker waits on the user is
    /// idle, since no worker is at work on it. A row cut without
    /// `~`, a wrong word, or a group out of order hides what the user
    /// looks for.
    #[test]
    fn render_lines_groups_folds_and_cuts_rows() {
        let mut board = make_active_board();

        let lines = board.render_lines(40, 16);

        let expected = [
            "< 3 ACTIVE  5 · 1 working",
            "v WORKING 1",
            " run  DEMO-KB-001 Faster search  Demo",
            "v NO WORKER 2",
            " none DEMO-KB-004 Lint every Do~ Demo",
            " none DEMO-KB-005 Rotate the ba~ Demo",
            "> IDLE 2",
            "v UP NEXT 1",
            " none DEMO-KB-020 Sort ready ca~ Demo",
            "> DRAFT 1",
            "",
            "",
            "",
            "",
            "",
            "esc back  ? help  read 10:42 0.6s",
        ];
        assert_eq!(lines, expected);
    }

    /// The full screen has room for idle cards, so IDLE must open
    /// when the board grows to 100 columns, and a tap on its heading
    /// must still fold it there. Without this the user on a wide
    /// screen opens IDLE by hand each time, or loses the fold.
    #[test]
    fn render_lines_wide_opens_idle_keeps_taps() {
        let mut board = make_active_board();

        let wide = board.render_lines(100, 24);
        board.handle_input(&tap_on(&wide, "v IDLE"));
        let tapped = board.render_lines(100, 24);

        assert!(wide.iter().any(|line| line == "v IDLE 2"));
        assert!(tapped.iter().any(|line| line == "> IDLE 2"));
    }

    /// A tap is the user's main input on a phone. A tap on a heading
    /// must fold or open its group, and a tap on `+ N more` must show
    /// the rest, or the cards past the first four cannot be reached.
    #[test]
    fn handle_input_taps_fold_and_show_more() {
        let (mut board, lines) = draw_active_page(make_many_ready_queue(), 16);

        board.handle_input(&tap_on(&lines, "v WORKING"));
        let folded = board.render_lines(40, 16);
        board.handle_input(&tap_on(&folded, "> WORKING"));
        let opened = board.render_lines(40, 16);
        board.handle_input(&tap_on(&opened, "+ 1 more"));
        let in_full = board.render_lines(40, 16);

        assert_eq!(folded[1..3], ["> WORKING 1", "v NO WORKER 2"]);
        assert_eq!(opened, lines);
        assert_eq!(in_full[12], " none DEMO-KB-024 Ready card 24  Demo");
    }

    /// `make_sample_queue` with four more cards ready to start, so UP
    /// NEXT holds five and page 3 at 40x16 shows four with `+ 1 more`.
    fn make_many_ready_queue() -> Queue {
        let mut queue = make_sample_queue();
        queue.cards.extend((21..=24).map(|number| CardLine {
            key: format!("DEMO-KB-0{number}").parse().unwrap(),
            stage: Stage::Todo,
            project: "Demo".to_owned(),
            title: format!("Ready card {number}"),
            workers: Vec::new(),
        }));
        queue
    }

    /// Returns a board that shows `queue` with page 3 open, drawn once
    /// at 40 by `rows`.
    fn draw_active_page(queue: Queue, rows: u16) -> (Board, Vec<String>) {
        let mut board = Board::new(false);
        board.show_reading(Ok(queue));
        board.render_lines(40, rows);
        board.handle_input(&press(KeyCode::Char('3')));
        let lines = board.render_lines(40, rows);
        (board, lines)
    }

    /// A tap on a scrolled list must open the row drawn under the
    /// finger. If the tap ignored the scroll, the user would open a
    /// different card than the one they tapped.
    #[test]
    fn handle_input_scrolled_tap_opens_drawn_row() {
        let (mut board, _) = draw_active_page(make_sample_queue(), 8);
        board.handle_input(&press(KeyCode::PageDown));
        let scrolled = board.render_lines(40, 8);

        board.handle_input(&tap_on(&scrolled, "DEMO-KB-005"));
        let details = board.render_lines(40, 8);

        assert_eq!(scrolled[1], " run  DEMO-KB-001 Faster search  Demo");
        assert_eq!(details[1], "Rotate the backup keys");
    }

    /// The row cuts a card to one line, so its details page is where
    /// the user sees who works on it, what to do next and its pull
    /// request. The read already holds this card's note, so opening the
    /// page must not read the note again and make the user wait.
    #[test]
    fn handle_input_tap_shows_details_without_reading() {
        let (mut board, lines) = open_sample_page('5');

        let step = board.handle_input(&tap_on(&lines, "DEMO-KB-010"));
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
        let (mut board, lines) = open_sample_page('3');
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
    /// sees. First screens 10, 6 and 4 rows high cut blocks down to
    /// their heading or less, which once put one block's elements over
    /// the next block.
    #[test]
    fn dump_lists_ids_and_places_as_drawn() {
        let mut board = make_active_board();
        let list = board.dump(40, 16, None);
        board.handle_input(&tap_on(&list.lines, "Faster search"));
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
            "group:working",
            "card:DEMO-KB-001",
            "group:no-worker",
            "card:DEMO-KB-004",
            "card:DEMO-KB-005",
            "group:idle",
            "group:todo",
            "card:DEMO-KB-020",
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
        let (mut board, _) = draw_active_page(make_many_ready_queue(), 16);

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
            ["group:idle", "card:DEMO-KB-002", "fold", "tap group:idle"],
            [
                "card:DEMO-KB-002",
                "card:DEMO-KB-003",
                "fold",
                "tap group:idle",
            ],
            ["more:todo", "card:DEMO-KB-024", "more", "tap more:todo"],
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
        let (mut board, _) = draw_active_page(make_sample_queue(), 5);
        board.handle_input(&press(KeyCode::PageDown));

        let dump = board.dump(40, 5, None);

        let read_hiding = |id: &str| {
            let element = dump.elements.iter().find(|element| element.id == id);
            let hiding = element.and_then(|element| element.hiding.as_ref()).unwrap();
            (hiding.hidden_by.word(), hiding.show.as_str())
        };
        assert_eq!(read_hiding("group:working"), ("scroll", "press pageup"));
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
        let active_top = "< 3 ACTIVE  5 · 1 working";
        let details: &[KeyCode] = &[KeyCode::Char('3'), KeyCode::Char('j'), KeyCode::Enter];
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
                reach: &[KeyCode::Char('3')],
                input: ctrl_c,
                expected: Step::Quit,
                top_line: active_top,
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
                reach: &[KeyCode::Char('3')],
                input: press(KeyCode::Char('q')),
                expected: Step::Stay,
                top_line: glance_top,
            },
            Case {
                name: "esc-on-view",
                reach: &[KeyCode::Char('3')],
                input: press(KeyCode::Esc),
                expected: Step::Stay,
                top_line: glance_top,
            },
            Case {
                name: "q-on-details",
                reach: details,
                input: press(KeyCode::Char('q')),
                expected: Step::Stay,
                top_line: active_top,
            },
            Case {
                name: "esc-on-details",
                reach: details,
                input: press(KeyCode::Esc),
                expected: Step::Stay,
                top_line: active_top,
            },
            Case {
                name: "tap-on-details",
                reach: details,
                input: tap,
                expected: Step::Stay,
                top_line: active_top,
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
        let (mut board, lines) = open_sample_page('1');
        board.handle_input(&tap_on(&lines, "DEMO-KB-001"));
        board.handle_input(&press(KeyCode::Esc));
        let tab_closed = Queue {
            waiting: Vec::new(),
            ..make_sample_queue()
        };

        board.show_reading(Ok(tab_closed));
        board.handle_input(&press(KeyCode::Enter));
        let details = board.render_lines(40, 16);

        assert_eq!(details[1], "Deploy tonight?");
    }

    /// Page 1 shows each waiting row's question, or why its tab waits,
    /// on the line below the row. A lost line hides what the user must
    /// answer.
    #[test]
    fn render_lines_puts_details_under_waiting_rows() {
        let (_, lines) = open_sample_page('1');

        assert!(lines[1].contains("blocked-fix"));
        assert_eq!(lines[2], "   needs you");
        assert!(lines[3].contains("DEMO-KB-001"));
        assert_eq!(lines[4], "   undated: Deploy tonight?");
    }

    /// Page 1 ends in a `then: review` line, and a tap on it opens
    /// page 5. A dead last line strands a phone user on page 1.
    #[test]
    fn handle_input_then_line_opens_review_page() {
        let (mut board, lines) = open_sample_page('1');

        board.handle_input(&tap_on(&lines, "then: review"));
        let review = board.render_lines(40, 16);

        assert_eq!(review[0], "< 5 IN REVIEW AND PULL REQUESTS");
    }

    /// A tap on page 4's fold row shows the kept, safe-to-close and
    /// parked sessions. A dead tap leaves them out of reach on a phone.
    #[test]
    fn handle_input_fold_tap_shows_done_sessions() {
        let (mut board, lines) = open_sample_page('4');

        board.handle_input(&tap_on(&lines, "> KEPT"));
        let opened = board.render_lines(40, 16);

        assert!(opened.iter().any(|line| line == "KEPT 1"));
        assert!(opened.iter().any(|line| line.starts_with("docs-fix ")));
    }

    /// Keys 1 to 5 open each page full screen, under a breadcrumb that
    /// names it. A key that missed a page would hide the sessions or
    /// gauges from a phone user, who has no wide screen to see them on.
    #[test]
    fn handle_input_digits_open_each_page() {
        let mut board = make_sample_board();
        let mut tops = Vec::new();

        for digit in ['1', '2', '3', '4', '5'] {
            board.handle_input(&press(KeyCode::Char(digit)));
            tops.push(board.render_lines(40, 16)[0].clone());
        }

        let expected = [
            "< 1 WAITING ON YOU  2",
            "< 2 SINCE YOU LEFT  newest",
            "< 3 ACTIVE  5 · 1 working",
            "< 4 SESSIONS AND MACHINE",
            "< 5 IN REVIEW AND PULL REQUESTS",
        ];
        assert_eq!(tops, expected);
    }

    /// Each page has a help page on `?` that says what its words mean,
    /// and Esc goes back to the page it came from. Help that opened on
    /// the wrong page, or a way back that lost the view, would leave a
    /// new user lost.
    #[test]
    fn handle_input_question_mark_opens_view_help() {
        let mut board = make_active_board();

        board.handle_input(&press(KeyCode::Char('?')));
        let help = board.render_lines(40, 16);
        board.handle_input(&press(KeyCode::Esc));
        let back = board.render_lines(40, 16);

        assert_eq!(help[..2], ["< back", "ACTIVE"]);
        assert_eq!(back[0], "< 3 ACTIVE  5 · 1 working");
    }

    /// Page 2 shows one row per card, named by its newest event, so a
    /// card's other events are one tap away. A row that did not open
    /// would hide what happened while the user was away, and one that
    /// did not fold again would push the other cards off a phone.
    #[test]
    fn handle_input_since_tap_toggles_events() {
        let (mut board, folded) = open_sample_page('2');

        board.handle_input(&tap_on(&folded, "> hub"));
        let opened = board.render_lines(40, 16);
        board.handle_input(&tap_on(&opened, "v hub"));
        let refolded = board.render_lines(40, 16);

        assert_eq!(folded[1], "> hub                             report");
        assert_eq!(
            opened[1..4],
            [
                "v hub                           2 events",
                "   10:40 report",
                "   10:12 report",
            ]
        );
        assert_eq!(refolded, folded);
    }

    /// Page 5 draws a card's pull request on the line under the card,
    /// with its check result, when the card's `prs:` link names it. The
    /// user reads there whether a card in review can merge.
    #[test]
    fn render_lines_review_page_shows_card_pr() {
        let mut queue = make_sample_queue();
        queue.pull_requests = Some(vec![PullRequestLine {
            name_with_owner: "demo/app".to_owned(),
            number: 301,
            title: "Retry uploads".to_owned(),
            check: CheckResult::Pass,
        }]);
        let mut board = Board::new(false);
        board.show_reading(Ok(queue));
        board.render_lines(40, 16);

        board.handle_input(&press(KeyCode::Char('5')));
        let lines = board.render_lines(40, 16);

        assert_eq!(
            lines[1..4],
            [
                "v IN REVIEW 1",
                " rev  DEMO-KB-010 Retry a faile~ Demo",
                "   Demo #301 pass",
            ]
        );
    }

    /// From 100 columns WAITING ON YOU sits left of ACTIVE, with a `│`
    /// between them. A block drawn over another would hide what the
    /// user opened a wide screen to see.
    #[test]
    fn render_lines_wide_glance_shows_two_columns() {
        let mut board = make_sample_board();

        let lines = board.render_lines(120, 16);

        // Each column's words, with the padding between them dropped.
        let read_columns = |line: &str| -> Vec<String> {
            line.split(" │ ")
                .map(|column| column.split_whitespace().collect::<Vec<_>>().join(" "))
                .collect()
        };
        assert_eq!(
            read_columns(&lines[1]),
            ["WAITING ON YOU 2 [1]", "ACTIVE 5 · 1 working [3]"]
        );
        assert_eq!(
            read_columns(&lines[2]),
            ["blocked-fix see tab", "DEMO-KB-001 Faster search"]
        );
    }

    /// From 100 columns the bottom line lists every page's key. Without
    /// it a user on a wide screen must open help to learn the keys.
    #[test]
    fn render_lines_wide_hint_lists_every_key() {
        let mut board = make_sample_board();

        let lines = board.render_lines(120, 16);

        assert_eq!(
            lines[15],
            "1 waiting  2 since  3 active  4 sessions and machine  5 review  ? help  q quit"
        );
    }

    /// From 100 columns each WAITING ON YOU and ACTIVE row has a dim
    /// line below it, the question or why the tab waits, and the card's
    /// newest event. Without it the user opens each page to see why a
    /// row is there.
    #[test]
    fn render_lines_wide_glance_shows_detail_lines() {
        let mut queue = make_sample_queue();
        queue.events[1].text = "started demo-kb-001-build (p_1) from /h/handoff.md".to_owned();
        let mut board = Board::new(false);
        board.show_reading(Ok(queue));

        let lines = board.render_lines(120, 16);

        let columns: Vec<String> = lines[3]
            .split(" │ ")
            .map(|column| column.split_whitespace().collect::<Vec<_>>().join(" "))
            .collect();
        assert!(lines[3].starts_with("   "));
        assert_eq!(columns, ["needs you", "10:31: new worker"]);
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

        let row_tap = board.handle_input(&tap_on(&before, "card-sync"));
        let chosen = board.render_lines(40, 16);
        let bar_tap = board.handle_input(&tap_on(&chosen, "-> jump"));

        assert_eq!(before[14], "tap a session to choose it");
        assert_eq!(row_tap, Step::Stay);
        assert_eq!(chosen[14], "-> jump to card-sync");
        assert_eq!(bar_tap, Step::Jump("card-sync".to_owned()));
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
        board.handle_input(&tap_on(&before, "card-sync"));
        let mut later = make_sample_queue();
        later.sessions.retain(|row| row.name != "card-sync");

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
            working: false,
            status: None,
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
            working: false,
            status: None,
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
