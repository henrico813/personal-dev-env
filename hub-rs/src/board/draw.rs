//! Draws the board's list page and details page, and lists the
//! elements each drew. Read `draw_list` and `list_elements` first.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line as TextLine, Span};
use ratatui::widgets::{List, ListItem, ListState, Paragraph};

use super::elements::{Action, Element, Kind};
use super::groups::{Group, Item};
use super::view::{Details, Line};

/// The space between the top line's counts.
const COUNT_GAP: &str = "  ";

/// The details page's bottom line.
const BACK_HINT: &str = "tap or q to go back";

/// The columns of an item's tag, such as `DEMO`, so tags line up.
const TAG_WIDTH: usize = 6;

/// What the list page shows.
pub(super) struct ListScreen<'a> {
    pub groups: &'a [(Group, Vec<Item>)],
    pub lines: &'a [Line],
    /// The bottom line's end, such as `read 10:42 0.6s`.
    pub status: &'a str,
    pub color: bool,
}

/// Draws the counts on top, the list of groups, and the bottom line.
/// The list scrolls so the chosen line in `list` shows.
pub(super) fn draw_list(frame: &mut Frame, screen: &ListScreen<'_>, list: &mut ListState) {
    let [top, middle, bottom] = split_screen(frame.area());
    frame.render_widget(count_line(screen.groups, screen.color), top);
    let width = usize::from(middle.width);
    let items: Vec<ListItem> = screen
        .lines
        .iter()
        .map(|line| ListItem::new(list_line(screen, *line, width)))
        .collect();
    let list_widget =
        List::new(items).highlight_style(Style::new().add_modifier(Modifier::REVERSED));
    frame.render_stateful_widget(list_widget, middle, list);
    let hint = format!("q quit  tap open  {}", screen.status);
    frame.render_widget(Paragraph::new(hint).style(dim()), bottom);
}

/// Draws an item's details page, wrapped to the width and scrolled
/// down `details.scroll` lines. The scroll stops where the last line
/// shows at the bottom, so a flick never leaves an empty page.
pub(super) fn draw_details(frame: &mut Frame, details: &mut Details) {
    let [top, middle, bottom] = split_screen(frame.area());
    let bold = Style::new().add_modifier(Modifier::BOLD);
    frame.render_widget(TextLine::styled("< back", bold), top);
    let wrapped = wrap_details(details, middle);
    details.scroll = details
        .scroll
        .min(wrapped.len().saturating_sub(usize::from(middle.height)));
    let shown: Vec<TextLine> = shown_details(details, middle)
        .into_iter()
        .map(TextLine::from)
        .collect();
    frame.render_widget(Paragraph::new(shown), middle);
    frame.render_widget(Paragraph::new(BACK_HINT).style(dim()), bottom);
}

/// Returns the list page's elements as `draw_list` drew them in
/// `area`, with `list` scrolled and chosen as the last draw left it.
pub(super) fn list_elements(screen: &ListScreen<'_>, area: Rect, list: &ListState) -> Vec<Element> {
    let [top, middle, bottom] = split_screen(area);
    let mut elements = Vec::new();
    let parts = count_parts(screen.groups);
    if parts.is_empty() {
        elements.push(text_element("title", "hub board", top));
    }
    // The gap holds two ASCII spaces.
    const GAP_WIDTH: u16 = COUNT_GAP.len() as u16;
    let mut column = top.x;
    for (group, text) in parts {
        let width = u16::try_from(text.len()).unwrap_or(u16::MAX);
        elements.push(Element {
            id: format!("count:{}", group.slug()),
            kind: Kind::Count,
            text,
            column,
            row: top.y,
            width,
            tap: Some("jump"),
            chosen: false,
            action: Action::Jump(group),
        });
        column = column.saturating_add(width).saturating_add(GAP_WIDTH);
    }
    let width = usize::from(middle.width);
    let shown = screen.lines.iter().enumerate().skip(list.offset());
    for ((index, line), row) in shown.zip(middle.y..middle.bottom()) {
        let (id, kind, tap) = describe_line(screen.groups, *line);
        elements.push(Element {
            id,
            kind,
            text: line_text(&list_line(screen, *line, width)),
            column: middle.x,
            row,
            width: middle.width,
            tap: Some(tap),
            chosen: list.selected() == Some(index),
            action: Action::Line(index),
        });
    }
    let hint = format!("q quit  tap open  {}", screen.status);
    elements.push(text_element("hint", &hint, bottom));
    elements
}

/// Returns the details page's elements as `draw_details` drew them in
/// `area`. A tap anywhere on the page goes back, so each says so.
pub(super) fn details_elements(details: &Details, area: Rect) -> Vec<Element> {
    let [top, middle, bottom] = split_screen(area);
    let back = |element: Element| Element {
        tap: Some("back"),
        action: Action::Back,
        ..element
    };
    let mut elements = vec![back(Element {
        kind: Kind::Back,
        ..text_element("back", "< back", top)
    })];
    let shown = shown_details(details, middle);
    for (number, (text, row)) in shown.iter().zip(middle.y..).enumerate() {
        let row_area = Rect { y: row, ..middle };
        elements.push(back(text_element(
            &format!("line:{number}"),
            text,
            row_area,
        )));
    }
    elements.push(back(text_element("hint", BACK_HINT, bottom)));
    elements
}

/// Returns the top line's area, the middle and the bottom line's area.
pub(super) fn split_screen(area: Rect) -> [Rect; 3] {
    Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(1),
        Constraint::Length(1),
    ])
    .areas(area)
}

/// Returns the details page's lines wrapped to `middle`'s width.
fn wrap_details(details: &Details, middle: Rect) -> Vec<String> {
    details
        .lines
        .iter()
        .flat_map(|line| wrap_text(line, usize::from(middle.width)))
        .collect()
}

/// Returns the wrapped details lines that fit in `middle`, from
/// `details.scroll` on.
fn shown_details(details: &Details, middle: Rect) -> Vec<String> {
    wrap_details(details, middle)
        .into_iter()
        .skip(details.scroll)
        .take(usize::from(middle.height))
        .collect()
}

/// Returns a list line's element id, kind and what a tap on it does.
fn describe_line(groups: &[(Group, Vec<Item>)], line: Line) -> (String, Kind, &'static str) {
    match line {
        Line::Heading { group, is_open } => {
            let tap = if is_open { "fold" } else { "open" };
            (format!("group:{}", group.slug()), Kind::Heading, tap)
        }
        Line::More { group, .. } => (format!("more:{}", group.slug()), Kind::More, "show all"),
        Line::Item { group, index } => {
            let item = groups
                .iter()
                .find(|(each, _)| *each == group)
                .and_then(|(_, items)| items.get(index));
            let id = item.map_or_else(String::new, |item| {
                format!("{}:{}", item.target.kind_word(), item.id)
            });
            (id, Kind::Item, "open")
        }
    }
}

/// Returns an element that shows `text` on `area`'s first row and does
/// nothing on a tap.
fn text_element(id: &str, text: &str, area: Rect) -> Element {
    Element {
        id: id.to_owned(),
        kind: Kind::Text,
        text: text.trim_end().to_owned(),
        column: area.x,
        row: area.y,
        width: area.width,
        tap: None,
        chosen: false,
        action: Action::None,
    }
}

/// Returns a drawn line's words, with the end trimmed.
fn line_text(line: &TextLine<'_>) -> String {
    let text: String = line
        .spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect();
    text.trim_end().to_owned()
}

/// Returns the top line's counts, such as `*5 progress`, or none
/// before the first read.
fn count_parts(groups: &[(Group, Vec<Item>)]) -> Vec<(Group, String)> {
    Group::COUNTED
        .iter()
        .filter_map(|&(group, word)| {
            let (_, items) = groups.iter().find(|(each, _)| *each == group)?;
            Some((group, format!("{}{} {word}", group.mark(), items.len())))
        })
        .collect()
}

/// Returns the top line, such as `*5 progress  @23 review  ?19 you`.
fn count_line(groups: &[(Group, Vec<Item>)], color: bool) -> TextLine<'static> {
    let parts = count_parts(groups);
    if parts.is_empty() {
        return TextLine::from("hub board");
    }
    let mut spans = Vec::new();
    for (group, text) in parts {
        if !spans.is_empty() {
            spans.push(Span::raw(COUNT_GAP));
        }
        spans.push(Span::styled(text, heading_style(group, color)));
    }
    TextLine::from(spans)
}

/// Returns one list line, `width` columns wide.
fn list_line(screen: &ListScreen<'_>, line: Line, width: usize) -> TextLine<'static> {
    match line {
        Line::Heading { group, is_open } => {
            let count = screen
                .groups
                .iter()
                .find(|(each, _)| *each == group)
                .map_or(0, |(_, items)| items.len());
            let arrow = if is_open { "v" } else { ">" };
            let text = format!("{arrow} {} {count}", group.title());
            TextLine::styled(text, heading_style(group, screen.color))
        }
        Line::More { hidden, .. } => TextLine::styled(format!("   + {hidden} more"), dim()),
        Line::Item { group, index } => {
            let item = screen
                .groups
                .iter()
                .find(|(each, _)| *each == group)
                .and_then(|(_, items)| items.get(index));
            match item {
                Some(item) => item_line(group, item, width, screen.color),
                None => TextLine::default(),
            }
        }
    }
}

/// Returns an item's line, such as ` @ Retry failed upl~ DEMO`. The
/// line holds its group's mark, its words cut to fit with `~`, and its
/// tag in the last `TAG_WIDTH` columns.
fn item_line(group: Group, item: &Item, width: usize, color: bool) -> TextLine<'static> {
    let room = width.saturating_sub(4 + TAG_WIDTH);
    let mark_style = if color {
        Style::new().fg(group.color())
    } else {
        Style::new()
    };
    TextLine::from(vec![
        Span::raw(" "),
        Span::styled(group.mark(), mark_style),
        Span::raw(" "),
        Span::raw(format!("{:room$}", fit_text(&item.text, room))),
        Span::styled(format!(" {}", fit_text(&item.tag, TAG_WIDTH)), dim()),
    ])
}

/// Cuts `text` to `room` columns, ending a cut text with `~`. A title
/// that `card list` already cut with `…` also ends with `~`.
fn fit_text(text: &str, room: usize) -> String {
    if text.chars().count() > room {
        let kept: String = text.chars().take(room.saturating_sub(1)).collect();
        return format!("{kept}~");
    }
    match text.strip_suffix('…') {
        Some(kept) => format!("{kept}~"),
        None => text.to_owned(),
    }
}

/// Returns the bold style of a heading or count, in its group's color
/// unless `color` is false.
fn heading_style(group: Group, color: bool) -> Style {
    let style = Style::new().add_modifier(Modifier::BOLD);
    if color {
        style.fg(group.color())
    } else {
        style
    }
}

/// Returns the style of hints and tags.
fn dim() -> Style {
    Style::new().add_modifier(Modifier::DIM)
}

/// Splits `text` into lines of at most `width` characters, at spaces
/// where it can and inside a word longer than `width`.
fn wrap_text(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    // Splitting at single spaces keeps double spaces inside a line.
    for word in text.split(' ') {
        let fits = width == 0 || line.chars().count() + 1 + word.chars().count() <= width;
        if !line.is_empty() && !fits {
            lines.push(std::mem::take(&mut line));
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(word);
        while width > 0 && line.chars().count() > width {
            let cut = line
                .char_indices()
                .nth(width)
                .map_or(line.len(), |(at, _)| at);
            let rest = line.split_off(cut);
            lines.push(std::mem::replace(&mut line, rest));
        }
    }
    lines.push(line);
    lines
}
