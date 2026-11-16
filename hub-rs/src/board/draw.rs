//! Draws the board's queue view and details page, and lists the
//! elements each holds, drawn or hidden. Read `draw_list` and
//! `list_elements` first.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line as TextLine, Span};
use ratatui::widgets::{List, ListItem, ListState, Paragraph};

use super::elements::{Action, Element, HiddenBy, Kind, Place};
use super::groups::{self, Group, Item, Status};
use super::queue::NO_HUB;
use super::view::{Details, Line};

/// The queue view's breadcrumb, which a tap on goes back.
const QUEUE_BACK: &str = "< queue";

/// The space after the breadcrumb.
const BACK_GAP: &str = "  ";

/// The space between the top line's counts.
const COUNT_GAP: &str = " · ";

/// The details page's bottom line.
const BACK_HINT: &str = "tap or q to go back";

/// The most columns an item's tag takes, so tags line up. A longer tag
/// is cut without a `~`, as the demo cuts `Homelab` to `Homela`.
const TAG_WIDTH: usize = 6;

/// What the queue view shows.
pub(super) struct ListScreen<'a> {
    pub groups: &'a [(Group, Vec<Item>)],
    pub lines: &'a [Line],
    /// The bottom line's end, such as `read 10:42 0.6s`.
    pub status: &'a str,
    pub color: bool,
}

/// Draws the breadcrumb and counts on top, the list of groups, and the
/// bottom line in `area`. The list scrolls so the chosen line in `list`
/// shows.
pub(super) fn draw_list(
    frame: &mut Frame,
    area: Rect,
    screen: &ListScreen<'_>,
    list: &mut ListState,
) {
    let [top, middle, bottom] = split_screen(area);
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
    frame.render_widget(
        Paragraph::new(view_hint(screen.status)).style(dim()),
        bottom,
    );
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

/// Returns the queue view's elements as `draw_list` drew them in
/// `area`, with `list` scrolled and chosen as the last draw left it.
/// Rows scrolled off come too, hidden, and so do the cards of a folded
/// group or behind `+ N more`, which come right after that line.
pub(super) fn list_elements(screen: &ListScreen<'_>, area: Rect, list: &ListState) -> Vec<Element> {
    let [top, middle, bottom] = split_screen(area);
    let mut elements = vec![back_element(QUEUE_BACK, top)];
    let mut column = top.x + width_of(QUEUE_BACK) + width_of(BACK_GAP);
    for (group, status, text) in count_parts(screen.groups) {
        let width = width_of(&text);
        elements.push(Element {
            id: format!("count:{}", status.word()),
            kind: Kind::Count,
            text,
            place: Some(Place {
                column,
                row: top.y,
                width,
            }),
            tap: Some("jump"),
            chosen: false,
            hidden: false,
            hiding: None,
            action: Action::Jump(group),
        });
        column = column
            .saturating_add(width)
            .saturating_add(width_of(COUNT_GAP));
    }
    let width = usize::from(middle.width);
    for (index, line) in screen.lines.iter().enumerate() {
        let (id, kind, tap) = describe_line(screen.groups, *line);
        let element = Element {
            id,
            kind,
            text: line_text(&list_line(screen, *line, width)),
            place: None,
            tap,
            chosen: list.selected() == Some(index),
            hidden: false,
            hiding: None,
            action: Action::Line(index),
        };
        let element = match row_or_scroll(index, list.offset(), middle) {
            Ok(row) => Element {
                place: Some(Place {
                    column: middle.x,
                    row,
                    width: middle.width,
                }),
                ..element
            },
            Err(step) => element.hide(HiddenBy::Scroll, step.to_owned()),
        };
        // A card under a line scrolled off is first reached by the
        // scroll that shows the line.
        let reach = |own: String| {
            element
                .hiding
                .as_ref()
                .map_or(own, |hiding| hiding.show.clone())
        };
        let hidden_cards = match *line {
            Line::Heading {
                group,
                is_open: false,
            } => {
                let show = reach(format!("tap group:{}", group.slug()));
                hidden_items(screen, group, 0, HiddenBy::Fold, &show, width)
            }
            Line::More { group, hidden } => {
                let show = reach(format!("tap more:{}", group.slug()));
                let first = group_items(screen.groups, group)
                    .len()
                    .saturating_sub(hidden);
                hidden_items(screen, group, first, HiddenBy::More, &show, width)
            }
            _ => Vec::new(),
        };
        elements.push(element);
        elements.extend(hidden_cards);
    }
    elements.push(text_element("hint", &view_hint(screen.status), bottom));
    elements
}

/// Returns a view's bottom line, such as
/// `esc back  ? help  read 10:42 0.6s`, with the board's `status` last.
pub(super) fn view_hint(status: &str) -> String {
    format!("esc back  ? help  {status}")
}

/// Returns the element of a view's breadcrumb, such as `< queue`, on
/// `area`'s first row. A tap on it goes back to the glance page.
pub(super) fn back_element(text: &str, area: Rect) -> Element {
    Element {
        kind: Kind::Back,
        tap: Some("back"),
        action: Action::Back,
        place: Some(Place {
            column: area.x,
            row: area.y,
            width: width_of(text),
        }),
        ..text_element("back", text, area)
    }
}

/// Returns the columns `text` takes, where a mark such as `·` takes
/// more than one byte but one column.
pub(super) fn width_of(text: &str) -> u16 {
    u16::try_from(text.chars().count()).unwrap_or(u16::MAX)
}

/// Returns the details page's elements as `draw_details` drew them in
/// `area`, with the lines scrolled off hidden. A tap anywhere on the
/// page goes back, so each says so.
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
    for (number, text) in wrap_details(details, middle).iter().enumerate() {
        let id = format!("line:{number}");
        let element = match row_or_scroll(number, details.scroll, middle) {
            Ok(row) => text_element(&id, text, Rect { y: row, ..middle }),
            Err(step) => text_element(&id, text, middle).hide(HiddenBy::Scroll, step.to_owned()),
        };
        elements.push(back(element));
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

/// Returns the row that line `index` is drawn on in `middle` with line
/// `first` on top, or for a line off screen the step that scrolls
/// toward it. On the page or view keys act on, repeating that step
/// reaches the line, since PageDown moves it by the rows it shows.
pub(super) fn row_or_scroll(index: usize, first: usize, middle: Rect) -> Result<u16, &'static str> {
    if index < first {
        return Err("press pageup");
    }
    u16::try_from(index - first)
        .ok()
        .filter(|&row| row < middle.height)
        .map(|row| middle.y + row)
        .ok_or("press pagedown")
}

/// Returns `group`'s items from `first` on as hidden elements, each
/// with `hidden_by` and `show`.
fn hidden_items(
    screen: &ListScreen<'_>,
    group: Group,
    first: usize,
    hidden_by: HiddenBy,
    show: &str,
    width: usize,
) -> Vec<Element> {
    group_items(screen.groups, group)
        .iter()
        .skip(first)
        .map(|item| {
            let element = Element {
                id: item_id(item),
                kind: Kind::Item,
                text: line_text(&item_line(item, width, screen.color)),
                place: None,
                tap: Some("open"),
                chosen: false,
                hidden: false,
                hiding: None,
                action: Action::None,
            };
            element.hide(hidden_by, show.to_owned())
        })
        .collect()
}

/// Returns `group`'s items, or none when the read has no such group.
fn group_items(groups: &[(Group, Vec<Item>)], group: Group) -> &[Item] {
    groups
        .iter()
        .find(|(each, _)| *each == group)
        .map_or(&[], |(_, items)| items)
}

/// Returns an item's element id, such as `card:DEMO-KB-007`.
fn item_id(item: &Item) -> String {
    format!("{}:{}", item.target.kind_word(), item.id)
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

/// Returns a list line's element id, kind and what a tap on it does,
/// if anything.
fn describe_line(
    groups: &[(Group, Vec<Item>)],
    line: Line,
) -> (String, Kind, Option<&'static str>) {
    match line {
        Line::Heading { group, is_open } => {
            let tap = if is_open { "fold" } else { "unfold" };
            (format!("group:{}", group.slug()), Kind::Heading, Some(tap))
        }
        Line::More { group, .. } => (
            format!("more:{}", group.slug()),
            Kind::More,
            Some("show all"),
        ),
        Line::Item { group, index } => {
            let id = group_items(groups, group)
                .get(index)
                .map_or_else(String::new, item_id);
            (id, Kind::Item, Some("open"))
        }
        Line::NoHub => ("no-hub".to_owned(), Kind::Text, None),
    }
}

/// Returns an element that shows `text` on `area`'s first row and does
/// nothing on a tap.
pub(super) fn text_element(id: &str, text: &str, area: Rect) -> Element {
    Element {
        id: id.to_owned(),
        kind: Kind::Text,
        text: text.trim_end().to_owned(),
        place: Some(Place {
            column: area.x,
            row: area.y,
            width: area.width,
        }),
        tap: None,
        chosen: false,
        hidden: false,
        hiding: None,
        action: Action::None,
    }
}

/// Returns a drawn line's words, with the end trimmed.
pub(super) fn line_text(line: &TextLine<'_>) -> String {
    let text: String = line
        .spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect();
    text.trim_end().to_owned()
}

/// Returns the QUEUE counts, such as `2 run`, each with its status
/// and the group a tap on it jumps to. None before the first read.
fn count_parts(groups: &[(Group, Vec<Item>)]) -> Vec<(Group, Status, String)> {
    groups::count_statuses(groups)
        .into_iter()
        .map(|(status, count, group)| (group, status, format!("{count} {}", status.word())))
        .collect()
}

/// Returns the QUEUE counts as one text, such as `2 run · 1 you`, or
/// none before the first read.
pub(super) fn count_words(groups: &[(Group, Vec<Item>)]) -> String {
    let parts: Vec<String> = count_parts(groups)
        .into_iter()
        .map(|(_, _, text)| text)
        .collect();
    parts.join(COUNT_GAP)
}

/// Returns the queue view's top line, such as
/// `< queue  2 run · 1 you · 1 idle`.
fn count_line(groups: &[(Group, Vec<Item>)], color: bool) -> TextLine<'static> {
    let bold = Style::new().add_modifier(Modifier::BOLD);
    let mut spans = vec![Span::styled(QUEUE_BACK, bold), Span::raw(BACK_GAP)];
    for (index, (_, status, text)) in count_parts(groups).into_iter().enumerate() {
        if index > 0 {
            spans.push(Span::raw(COUNT_GAP));
        }
        let style = if color { bold.fg(status.color()) } else { bold };
        spans.push(Span::styled(text, style));
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
        Line::NoHub => TextLine::styled(format!("   {NO_HUB}"), dim()),
        Line::Item { group, index } => {
            let item = screen
                .groups
                .iter()
                .find(|(each, _)| *each == group)
                .and_then(|(_, items)| items.get(index));
            match item {
                Some(item) => item_line(item, width, screen.color),
                None => TextLine::default(),
            }
        }
    }
}

/// Returns an item's line, such as ` rev  Retry failed u~ DevEnv`. The
/// line holds its status word, its words cut to fit with `~`, and its
/// tag in the last `TAG_WIDTH` columns.
pub(super) fn item_line(item: &Item, width: usize, color: bool) -> TextLine<'static> {
    let room = width.saturating_sub(Status::WIDTH + 3 + TAG_WIDTH);
    let tag: String = item.tag.chars().take(TAG_WIDTH).collect();
    let status_style = match item.status {
        Status::None => dim(),
        status if color => Style::new().fg(status.color()),
        _ => Style::new(),
    };
    TextLine::from(vec![
        Span::raw(" "),
        Span::styled(
            format!("{:<width$}", item.status.word(), width = Status::WIDTH),
            status_style,
        ),
        Span::raw(" "),
        Span::raw(format!("{:room$}", fit_text(&item.text, room))),
        Span::styled(format!(" {tag}"), dim()),
    ])
}

/// Cuts `text` to `room` columns, ending a cut text with `~`.
fn fit_text(text: &str, room: usize) -> String {
    if text.chars().count() > room {
        let kept: String = text.chars().take(room.saturating_sub(1)).collect();
        return format!("{kept}~");
    }
    text.to_owned()
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
pub(super) fn dim() -> Style {
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
