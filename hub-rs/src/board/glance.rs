//! Draws the glance page, the board's first page. It answers three
//! questions at once. What happened while the user was away? What is
//! in the queue, and does it run now? What comes next? Each block
//! is cut to the rows that fit, and a tap on it opens its view. Below
//! 100 columns the blocks are stacked; from 100 on they sit side by
//! side. Read `draw_glance` and `build_glance_elements` first.

use jiff::{SignedDuration, Timestamp};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::Line as TextLine;
use ratatui::widgets::Paragraph;

use super::away;
use super::draw;
use super::elements::{Action, Element, HiddenBy, Kind, Place};
use super::groups::{self, Group, Item};
use super::panels::{self, Panel, WIDE_COLUMNS};
use super::queue::Queue;
use super::sessions::fit;
use crate::shared::format_span;

/// The blocks, top to bottom or left to right.
const BLOCKS: [Panel; 3] = [Panel::Away, Panel::Queue, Panel::Next];

/// The most rows the AWAY and NEXT blocks take below 100 columns,
/// heading included. QUEUE takes the rest.
const SHORT_BLOCK_ROWS: u16 = 4;

/// What sits between two blocks side by side.
const GAP: &str = " │ ";

/// The space between the bottom line's parts.
const HINT_GAP: &str = "  ";

/// The bottom line's parts a tap acts on, each as its element id, words
/// and action.
const HINT: [(&str, &str, Action); 3] = [
    ("help", "? help", Action::Help),
    ("open:sessions", "4 sessions", Action::Open(Panel::Sessions)),
    ("open:machine", "5 machine", Action::Open(Panel::Machine)),
];

/// The bottom line's last part, which a tap does nothing on.
const QUIT_HINT: &str = "q quit";

/// What the glance page shows.
pub(super) struct Glance<'a> {
    pub groups: &'a [(Group, Vec<Item>)],
    /// The last read, or `None` before the first.
    pub queue: Option<&'a Queue>,
    /// The top line's end, such as `read 10:42 0.6s`.
    pub status: &'a str,
    /// The start of the user's visit, or `None` with no saved time.
    pub since: Option<Timestamp>,
    /// How long the user was away before this visit.
    pub away_for: Option<SignedDuration>,
    pub color: bool,
}

/// The page's areas. `gaps` are empty below 100 columns.
struct Parts {
    top: Rect,
    blocks: [Rect; 3],
    gaps: [Rect; 2],
    hint: Rect,
}

/// One block as drawn, with its heading and the rows that fit. When some
/// do not fit, it also holds the count on its `+ N more` line and the
/// rows left out.
struct Block {
    heading: String,
    shown: Vec<TextLine<'static>>,
    more: Option<usize>,
    hidden: Vec<TextLine<'static>>,
}

/// Draws the glance page over `area`.
pub(super) fn draw_glance(frame: &mut Frame, area: Rect, glance: &Glance<'_>) {
    let parts = split_glance(area);
    frame.render_widget(Paragraph::new(format_top_line(glance)), parts.top);
    let bold = Style::new().add_modifier(Modifier::BOLD);
    for (panel, block_area) in BLOCKS.into_iter().zip(parts.blocks) {
        let block = cut_block(panel, glance, block_area);
        let mut lines = vec![TextLine::styled(block.heading, bold)];
        lines.extend(block.shown);
        if let Some(hidden) = block.more {
            lines.push(TextLine::styled(
                format_more_words(hidden),
                draw::pick_dim_style(),
            ));
        }
        frame.render_widget(Paragraph::new(lines), block_area);
    }
    for gap in parts.gaps {
        let bars = vec![TextLine::from(GAP); usize::from(gap.height)];
        frame.render_widget(Paragraph::new(bars).style(draw::pick_dim_style()), gap);
    }
    frame.render_widget(
        Paragraph::new(format_hint_text()).style(draw::pick_dim_style()),
        parts.hint,
    );
}

/// Returns the glance page's elements as `draw_glance` drew them in
/// `area`. Every line of a block opens its view on a tap. The rows a
/// block cuts come after its last line, hidden, with the tap that opens
/// the block as the step that shows them.
pub(super) fn build_glance_elements(area: Rect, glance: &Glance<'_>) -> Vec<Element> {
    let parts = split_glance(area);
    let mut elements = vec![draw::build_text_element(
        "status",
        &format_top_line(glance),
        parts.top,
    )];
    for (panel, block_area) in BLOCKS.into_iter().zip(parts.blocks) {
        // A screen too short for the block draws none of it.
        if block_area.height == 0 {
            continue;
        }
        let block = cut_block(panel, glance, block_area);
        let slug = panel.slug();
        let make_open_element = |id: &str, kind, text: &str, row: usize| {
            let offset = u16::try_from(row).unwrap_or(u16::MAX);
            let place = Rect {
                y: block_area.y.saturating_add(offset),
                ..block_area
            };
            Element {
                kind,
                tap: Some("open"),
                action: Action::Open(panel),
                ..draw::build_text_element(id, text, place)
            }
        };
        elements.push(make_open_element(
            &format!("block:{slug}"),
            Kind::Heading,
            &block.heading,
            0,
        ));
        for (number, line) in block.shown.iter().enumerate() {
            let id = format!("{slug}:{number}");
            elements.push(make_open_element(
                &id,
                Kind::Text,
                &draw::join_line_text(line),
                number + 1,
            ));
        }
        if let Some(hidden) = block.more {
            let more_row = block.shown.len() + 1;
            let more_id = format!("more:{slug}");
            elements.push(make_open_element(
                &more_id,
                Kind::More,
                &format_more_words(hidden),
                more_row,
            ));
        }
        let show = format!("tap block:{slug}");
        for (number, line) in block.hidden.iter().enumerate() {
            let id = format!("{slug}:{}", block.shown.len() + number);
            let element = make_open_element(&id, Kind::Text, &draw::join_line_text(line), 0);
            elements.push(element.hide(HiddenBy::More, show.clone()));
        }
    }
    let mut column = parts.hint.x;
    for (id, words, action) in HINT {
        let element = draw::build_text_element(id, words, parts.hint);
        elements.push(Element {
            tap: Some("open"),
            action,
            place: element.place.map(|place| move_place(place, column, words)),
            ..element
        });
        column += draw::measure_width(words) + draw::measure_width(HINT_GAP);
    }
    let quit = draw::build_text_element("quit", QUIT_HINT, parts.hint);
    elements.push(Element {
        place: quit.place.map(|place| move_place(place, column, QUIT_HINT)),
        ..quit
    });
    elements
}

/// Returns `place` moved to `column` and as wide as `words`.
fn move_place(place: Place, column: u16, words: &str) -> Place {
    Place {
        column,
        width: draw::measure_width(words),
        ..place
    }
}

/// Returns the top line, such as `away 1h29m · read 10:42 0.6s`, with
/// the board's status last.
fn format_top_line(glance: &Glance<'_>) -> String {
    match glance.away_for {
        Some(away_for) => format!("away {} · {}", format_span(away_for), glance.status),
        None => glance.status.to_owned(),
    }
}

/// Returns the bottom line, `? help  4 sessions  5 machine  q quit`.
fn format_hint_text() -> String {
    let mut parts: Vec<&str> = HINT.iter().map(|(_, words, _)| *words).collect();
    parts.push(QUIT_HINT);
    parts.join(HINT_GAP)
}

/// Returns the top line's, the blocks' and the bottom line's areas.
fn split_glance(area: Rect) -> Parts {
    let [top, body, hint] = draw::split_screen(area);
    if area.width >= WIDE_COLUMNS {
        let gap = Constraint::Length(draw::measure_width(GAP));
        let fill = Constraint::Fill(1);
        let [away, gap_1, queue, gap_2, next] =
            Layout::horizontal([fill, gap, fill, gap, fill]).areas(body);
        return Parts {
            top,
            blocks: [away, queue, next],
            gaps: [gap_1, gap_2],
            hint,
        };
    }
    // On a short screen AWAY and NEXT give up rows first, and each block
    // keeps a third of the body, so QUEUE never shrinks to nothing.
    let short = Constraint::Length(SHORT_BLOCK_ROWS.min(body.height / 3));
    let [away, queue, next] = Layout::vertical([short, Constraint::Fill(1), short]).areas(body);
    Parts {
        top,
        blocks: [away, queue, next],
        gaps: [Rect::default(); 2],
        hint,
    }
}

/// Returns `panel`'s block cut to `area`, with its heading on the first
/// row and then the rows that fit. When some do not fit, the last row is
/// `+ N more`. A block with room for its heading alone shows only that.
fn cut_block(panel: Panel, glance: &Glance<'_>, area: Rect) -> Block {
    let width = usize::from(area.width);
    let mut shown = list_block_rows(panel, glance, width);
    let room = usize::from(area.height).saturating_sub(1);
    let mut more = None;
    let mut hidden = Vec::new();
    if shown.len() > room {
        hidden = shown.split_off(room.saturating_sub(1));
        more = (room > 0).then_some(hidden.len());
    }
    Block {
        heading: format_heading_line(panel, glance, width),
        shown,
        more,
        hidden,
    }
}

/// Returns a block's heading, such as `QUEUE  2 run · 1 you`, with its
/// key, such as `[2]`, at the right end of `width`.
fn format_heading_line(panel: Panel, glance: &Glance<'_>, width: usize) -> String {
    let words = match (panel, glance.queue) {
        (Panel::Away, Some(queue)) => {
            let count = away::find_events_since(&queue.events, glance.since).len();
            format!("AWAY  {}", away::describe_new_count(glance.since, count))
        }
        (Panel::Queue, _) => format!("QUEUE  {}", draw::count_words(glance.groups)),
        _ => panel.slug().to_uppercase(),
    };
    let key = format!("[{}]", panel.number());
    let room = width.saturating_sub(key.len() + 1);
    format!("{:<room$} {key}", fit(words.trim_end(), room))
}

/// Returns every row of `panel`'s block, before any cut. A block with
/// nothing to show has one dim row that says why.
fn list_block_rows(panel: Panel, glance: &Glance<'_>, width: usize) -> Vec<TextLine<'static>> {
    let dim_row = |words: &str| {
        vec![TextLine::styled(
            format!(" {words}"),
            draw::pick_dim_style(),
        )]
    };
    let Some(queue) = glance.queue else {
        return dim_row("reading");
    };
    let rows: Vec<TextLine<'static>> = match panel {
        Panel::Away => {
            panels::format_away_lines(away::find_events_since(&queue.events, glance.since), width)
                .into_iter()
                .map(|line| TextLine::raw(line.text))
                .collect()
        }
        Panel::Queue => groups::pick_glance_items(glance.groups)
            .into_iter()
            .map(|(_, item)| draw::format_item_line(item, width, glance.color))
            .collect(),
        Panel::Next => panels::format_next_lines(glance.groups, queue.no_hub, 1, width)
            .into_iter()
            .map(|line| TextLine::raw(line.text))
            .collect(),
        Panel::Sessions | Panel::Machine => Vec::new(),
    };
    if !rows.is_empty() {
        return rows;
    }
    match panel {
        Panel::Away if glance.since.is_some() => dim_row("nothing new"),
        Panel::Away => dim_row("no reports or notes yet"),
        _ => dim_row("nothing waits or runs"),
    }
}

/// Returns a block's `+ N more` line.
fn format_more_words(hidden: usize) -> String {
    format!("   + {hidden} more")
}
