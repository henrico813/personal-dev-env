//! Draws the board's screens from its state. The list screen shows
//! the sections, and full-screen pages show a row's details, the menu
//! of sections and the help. Nothing here changes the board. Read
//! `draw_list_screen` first.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Position, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Paragraph, Wrap};

use super::list::{ITEM_INDENT, Row, SectionList};
use super::sections::Counts;

/// Screens at least this tall show the chosen row's first lines under
/// the bottom line.
const PREVIEW_MIN_HEIGHT: u16 = 18;

/// The rows of that preview.
const PREVIEW_HEIGHT: u16 = 2;

/// The bottom line of a page that any tap closes.
const BACK_HINT: &str = "tap or Esc to go back";

/// The help's bottom line. The help is longer than a phone screen.
const HELP_HINT: &str = "swipe for more, tap to go back";

/// The menu's last row, which opens the help.
const HELP_ROW: &str = "? keys and taps";

/// The help page. Each line fits 40 columns, so a phone shows it
/// without wrapping.
const HELP_TEXT: &str = "\
On a phone
 Tap a row to choose it. Tap it
 again to read it in full.
 Tap a heading to fold or open it.
 Bottom line: left pages up, middle
 opens the menu of sections, right
 pages down.
 On details or help, a tap goes back.
Keys
 j k, arrows   move
 PgUp PgDn     page
 g G           first, last row
 n N           next, previous section
 Enter, i      open or fold
 s             menu of sections
 ?             this help
 Esc           back
 r             read now
 q, Ctrl-C     quit
Marks
 V card in review  * card worked on
 - card with no worker
 ! card worked on in draft or ready
 A approval  B blocked  ? question
 # PR  . PR note  X problem
 H waits on hub  R unread report
 P parked  C safe to close  K kept";

/// Where the parts of the list screen go. Taps use it to find what
/// they hit.
#[derive(Clone, Copy)]
pub(super) struct ListLayout {
    pub top: Rect,
    pub list: Rect,
    pub bottom: Rect,
    pub preview: Rect,
}

impl ListLayout {
    /// Splits `area` into the top line, the list, the bottom line and,
    /// on a tall screen, a preview.
    pub(super) fn split(area: Rect) -> ListLayout {
        let preview_height = if area.height >= PREVIEW_MIN_HEIGHT {
            PREVIEW_HEIGHT
        } else {
            0
        };
        let [top, list, bottom, preview] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
            Constraint::Length(preview_height),
        ])
        .areas(area);
        ListLayout {
            top,
            list,
            bottom,
            preview,
        }
    }

    /// Returns what a tap at `at` asks for by the third of the bottom
    /// line it hits, or `None` when the tap is off that line.
    pub(super) fn bottom_tap(&self, at: Position) -> Option<BottomTap> {
        if !self.bottom.contains(at) {
            return None;
        }
        let side = side_third_width(self.bottom.width);
        let x = at.x - self.bottom.x;
        Some(if x < side {
            BottomTap::PageUp
        } else if x >= self.bottom.width - side {
            BottomTap::PageDown
        } else {
            BottomTap::Menu
        })
    }
}

/// What a tap on each third of the bottom line does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum BottomTap {
    PageUp,
    /// Opens the menu of sections and help.
    Menu,
    PageDown,
}

/// Draws the top line, the rows `list` placed, the bottom line and the
/// preview.
pub(super) fn draw_list_screen(
    frame: &mut Frame,
    layout: &ListLayout,
    top_line: Line<'_>,
    list: &SectionList,
    color: bool,
) {
    frame.render_widget(Paragraph::new(top_line).style(bar_style()), layout.top);
    for (y, (row, is_chosen)) in (layout.list.y..).zip(list.drawn_rows()) {
        let line_area = Rect::new(layout.list.x, y, layout.list.width, 1);
        draw_cut_line(
            frame,
            row_line(row, is_chosen, layout.list.width, color),
            line_area,
        );
    }
    frame.render_widget(
        Paragraph::new(bottom_line(list, layout.bottom.width)).style(bar_style()),
        layout.bottom,
    );
    frame.render_widget(
        Paragraph::new(detail_text(list.chosen_row())).wrap(Wrap { trim: true }),
        layout.preview,
    );
}

/// Draws the chosen row's full text over the whole screen, scrolled
/// down `scroll` lines, under its section's heading.
pub(super) fn draw_details(frame: &mut Frame, list: &SectionList, scroll: u16) {
    let title = match list.chosen_row() {
        Some(Row::Heading { section, .. } | Row::Item { section, .. }) => section.heading(),
        None => String::new(),
    };
    let text = detail_text(list.chosen_row());
    draw_page(frame, &title, text, scroll, BACK_HINT);
}

/// Draws every key and tap over the whole screen, scrolled down
/// `scroll` lines.
pub(super) fn draw_help(frame: &mut Frame, scroll: u16) {
    draw_page(frame, "KEYS AND TAPS", HELP_TEXT, scroll, HELP_HINT);
}

/// Returns how far the help scrolls on a screen of `area`, so its last
/// line stays at the bottom and a flick never leaves an empty page.
pub(super) fn help_scroll_limit(area: Rect) -> u16 {
    let [_, body, _] = page_areas(area);
    let lines = u16::try_from(HELP_TEXT.lines().count()).unwrap_or(u16::MAX);
    lines.saturating_sub(body.height)
}

/// Draws the menu, a row per section and then the help row, with row
/// `marked` marked. `menu_row_at` finds the row a tap hits.
pub(super) fn draw_menu(frame: &mut Frame, list: &SectionList, marked: usize, color: bool) {
    let entries = list
        .sections()
        .iter()
        .map(|section| (section.heading(), section.id.color().filter(|_| color)))
        .chain([(HELP_ROW.to_owned(), None)]);
    let rows: Vec<Line> = entries
        .enumerate()
        .map(|(index, (text, text_color))| {
            let is_marked = index == marked;
            let pointer = if is_marked { '>' } else { ' ' };
            let mut style = Style::new().add_modifier(Modifier::BOLD);
            if let Some(text_color) = text_color {
                style = style.fg(text_color);
            }
            if is_marked {
                style = style.add_modifier(Modifier::REVERSED);
            }
            Line::styled(format!("{pointer} {text}"), style)
        })
        .collect();
    draw_page(
        frame,
        "GO TO A SECTION",
        rows,
        0,
        "tap one, or tap here to go back",
    );
}

/// Returns the menu row drawn at `at` on a screen of `area`, or `None`
/// when `at` is on the title or bottom line. The row may be past the
/// last entry.
pub(super) fn menu_row_at(area: Rect, at: Position) -> Option<usize> {
    let [_, body, _] = page_areas(area);
    body.contains(at).then(|| usize::from(at.y - body.y))
}

/// Returns the top line, such as `YOU 3+2  PR 1  HUB 2  busy 9/8!
/// 10:42`, with `when` last. The busy count turns red over the card
/// limit.
pub(super) fn top_line(counts: &Counts, when: &str, color: bool) -> Line<'static> {
    let mut you = format!("YOU {}", counts.you);
    if counts.later > 0 {
        you.push_str(&format!("+{}", counts.later));
    }
    let mut spans = vec![Span::raw(format!(
        "{you}  PR {}  HUB {}  ",
        counts.pull_requests, counts.hub
    ))];
    spans.push(match counts.cards {
        Some((busy, limit)) if busy > limit => {
            let style = if color {
                Style::new().fg(Color::Red)
            } else {
                Style::new()
            };
            Span::styled(format!("busy {busy}/{limit}!"), style)
        }
        Some((busy, limit)) => Span::raw(format!("busy {busy}/{limit}")),
        None => Span::raw("busy ?"),
    });
    spans.push(Span::raw(format!("  {when}")));
    Line::from(spans)
}

/// Returns one list line. A heading is a full-width rule, such as
/// `── NEEDS YOU 3 ─────`, starting `+─` while folded. An item's first
/// line is `>` when chosen, its mark and its text, and its other lines
/// line up under that text.
fn row_line(row: Row<'_>, is_chosen: bool, width: u16, color: bool) -> Line<'static> {
    let chosen_style = if is_chosen {
        Style::new().add_modifier(Modifier::REVERSED)
    } else {
        Style::new()
    };
    match row {
        Row::Heading { section, folded } => {
            let opener = if folded { '+' } else { '─' };
            let mut text = format!("{opener}─ {} ", section.heading());
            let fill = usize::from(width).saturating_sub(text.chars().count());
            text.extend(std::iter::repeat_n('─', fill));
            let mut style = chosen_style.add_modifier(Modifier::BOLD);
            if color && let Some(color) = section.id.color() {
                style = style.fg(color);
            }
            Line::styled(text, style)
        }
        Row::Item {
            item, line, first, ..
        } => {
            let text = if first {
                let pointer = if is_chosen { '>' } else { ' ' };
                format!("{pointer}{} {line}", item.mark)
            } else {
                format!("{:ITEM_INDENT$}{line}", "")
            };
            Line::styled(text, chosen_style)
        }
    }
}

/// Returns the bottom line. Its thirds are tap targets, page up, the
/// menu and page down, and the middle one shows the chosen row's
/// place, such as `3/12 menu`.
fn bottom_line(list: &SectionList, width: u16) -> String {
    let third = usize::from(side_third_width(width));
    let width = usize::from(width);
    let middle_width = width - 2 * third;
    let place = list.chosen().map_or("0/0".to_owned(), |index| {
        format!("{}/{}", index + 1, list.len())
    });
    let middle = format!("{place} menu");
    format!(
        "{:<third$}{middle:^middle_width$}{:>third$}",
        "^ up", "down v"
    )
}

/// Returns the width of the bottom line's left and right thirds. The
/// middle third takes the rest, so what a tap hits matches what is
/// drawn there.
fn side_third_width(width: u16) -> u16 {
    width / 3
}

/// Returns a row's full text, or for a heading how to fold or open it.
fn detail_text(row: Option<Row<'_>>) -> String {
    match row {
        Some(Row::Item { item, .. }) => item.detail.clone(),
        Some(Row::Heading { folded: true, .. }) => {
            "Folded. Tap it or press Enter to open it.".to_owned()
        }
        Some(Row::Heading { folded: false, .. }) => "Tap it or press Enter to fold it.".to_owned(),
        None => "Nothing to show.".to_owned(),
    }
}

/// Draws `line` in `area`, a single row, ending it with `~` when it is
/// cut, so the user knows more text is in the details.
fn draw_cut_line(frame: &mut Frame, line: Line<'_>, area: Rect) {
    let is_cut = line.width() > usize::from(area.width);
    frame.render_widget(line, area);
    if is_cut && area.width > 0 {
        frame.buffer_mut()[(area.right() - 1, area.y)].set_symbol("~");
    }
}

/// Draws a full-screen page with `title` on top, `body` scrolled down
/// `scroll` lines, and `hint` on the bottom line.
fn draw_page<'a>(
    frame: &mut Frame,
    title: &str,
    body: impl Into<Text<'a>>,
    scroll: u16,
    hint: &str,
) {
    let [top, middle, bottom] = page_areas(frame.area());
    frame.render_widget(Paragraph::new(title).style(bar_style()), top);
    frame.render_widget(
        Paragraph::new(body)
            .wrap(Wrap { trim: false })
            .scroll((scroll, 0)),
        middle,
    );
    frame.render_widget(Paragraph::new(hint).style(bar_style()), bottom);
}

/// Returns a full-screen page's title line, body and bottom line.
fn page_areas(area: Rect) -> [Rect; 3] {
    Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(1),
        Constraint::Length(1),
    ])
    .areas(area)
}

/// Returns the style of the top and bottom lines.
fn bar_style() -> Style {
    Style::new().add_modifier(Modifier::REVERSED)
}
