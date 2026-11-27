//! Draws the list pages 1, 3 and 5 and the details page, and lists
//! the elements each holds, drawn or hidden. Read `draw_list` and
//! `build_list_elements` first.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line as TextLine, Span};
use ratatui::widgets::{List, ListItem, ListState, Paragraph};

use super::elements::{Action, Element, HiddenBy, Kind, Place};
use super::groups::{Group, Item, Status, Target};
use super::panels::Panel;
use super::pull_requests::CheckResult;
use super::queue::{NO_HUB, Queue};
use super::view::{Details, Line};
use crate::external::cards::BoardCounts;

/// The space after the breadcrumb.
const BACK_GAP: &str = "  ";

/// The space between the top line's counts.
const COUNT_GAP: &str = " · ";

/// The details page's bottom line.
const BACK_HINT: &str = "tap or q to go back";

/// The most columns an item's tag takes, so tags line up. A longer tag
/// is cut without a `~`; 7 keeps `see tab` and labels such as `Demo`
/// whole.
const TAG_WIDTH: usize = 7;

/// What a list page shows.
pub(super) struct ListScreen<'a> {
    /// Page 1, 3 or 5, which names the breadcrumb.
    pub panel: Panel,
    pub groups: &'a [(Group, Vec<Item>)],
    pub lines: &'a [Line],
    /// The read the counts come from, or `None` before the first read.
    pub queue: Option<&'a Queue>,
    /// The bottom line's end, such as `read 10:42 0.6s`.
    pub status: &'a str,
    pub color: bool,
}

/// Draws the breadcrumb and counts on top, such as
/// `< 3 ACTIVE  23, max 8 · 2 working`, the page's groups, and the
/// bottom line in `area`. The list scrolls so the chosen line in `list`
/// shows.
pub(super) fn draw_list(
    frame: &mut Frame,
    area: Rect,
    screen: &ListScreen<'_>,
    list: &mut ListState,
) {
    let [top, middle, bottom] = split_screen(area);
    frame.render_widget(format_breadcrumb_line(screen), top);
    let width = usize::from(middle.width);
    let items: Vec<ListItem> = screen
        .lines
        .iter()
        .map(|line| ListItem::new(format_list_line(screen, *line, width)))
        .collect();
    let list_widget =
        List::new(items).highlight_style(Style::new().add_modifier(Modifier::REVERSED));
    frame.render_stateful_widget(list_widget, middle, list);
    frame.render_widget(
        Paragraph::new(format_view_hint(screen.status)).style(pick_dim_style()),
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
    let shown: Vec<TextLine> = select_shown_detail_lines(details, middle)
        .into_iter()
        .map(TextLine::from)
        .collect();
    frame.render_widget(Paragraph::new(shown), middle);
    frame.render_widget(Paragraph::new(BACK_HINT).style(pick_dim_style()), bottom);
}

/// Returns a list page's elements as `draw_list` drew them in
/// `area`, with `list` scrolled and chosen as the last draw left it.
/// Rows scrolled off come too, hidden, and so do the cards of a folded
/// group or behind `+ N more`, which come right after that line.
pub(super) fn build_list_elements(
    screen: &ListScreen<'_>,
    area: Rect,
    list: &ListState,
) -> Vec<Element> {
    let [top, middle, bottom] = split_screen(area);
    let mut elements = vec![build_back_element(&format_page_title(screen.panel), top)];
    let width = usize::from(middle.width);
    for (index, line) in screen.lines.iter().enumerate() {
        let (id, kind, tap) = describe_line(screen.groups, *line);
        let element = Element {
            id,
            kind,
            text: join_line_text(&format_list_line(screen, *line, width)),
            place: None,
            tap,
            chosen: list.selected() == Some(index),
            hidden: false,
            hiding: None,
            action: Action::Line(index),
        };
        let element = match find_row_or_scroll(index, list.offset(), middle) {
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
        let pick_reach_step = |own: String| {
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
                let show = pick_reach_step(format!("tap group:{}", group.slug()));
                build_hidden_items(screen, group, 0, HiddenBy::Fold, &show, width)
            }
            Line::More { group, hidden } => {
                let show = pick_reach_step(format!("tap more:{}", group.slug()));
                let first = find_group_items(screen.groups, group)
                    .len()
                    .saturating_sub(hidden);
                build_hidden_items(screen, group, first, HiddenBy::More, &show, width)
            }
            _ => Vec::new(),
        };
        elements.push(element);
        elements.extend(hidden_cards);
    }
    elements.push(build_text_element(
        "hint",
        &format_view_hint(screen.status),
        bottom,
    ));
    elements
}

/// Returns a view's bottom line, such as
/// `esc back  ? help  read 10:42 0.6s`, with the board's `status` last.
pub(super) fn format_view_hint(status: &str) -> String {
    format!("esc back  ? help  {status}")
}

/// Returns the element of a view's breadcrumb, such as `< 3 ACTIVE`,
/// on `area`'s first row. A tap on it goes back to the glance page.
pub(super) fn build_back_element(text: &str, area: Rect) -> Element {
    Element {
        kind: Kind::Back,
        tap: Some("back"),
        action: Action::Back,
        place: Some(Place {
            column: area.x,
            row: area.y,
            width: measure_width(text),
        }),
        ..build_text_element("back", text, area)
    }
}

/// Returns the columns `text` takes, where a mark such as `·` takes
/// more than one byte but one column.
pub(super) fn measure_width(text: &str) -> u16 {
    u16::try_from(text.chars().count()).unwrap_or(u16::MAX)
}

/// Returns the details page's elements as `draw_details` drew them in
/// `area`, with the lines scrolled off hidden. A tap anywhere on the
/// page goes back, so each says so.
pub(super) fn build_details_elements(details: &Details, area: Rect) -> Vec<Element> {
    let [top, middle, bottom] = split_screen(area);
    let add_back_tap = |element: Element| Element {
        tap: Some("back"),
        action: Action::Back,
        ..element
    };
    let mut elements = vec![add_back_tap(Element {
        kind: Kind::Back,
        ..build_text_element("back", "< back", top)
    })];
    for (number, text) in wrap_details(details, middle).iter().enumerate() {
        let id = format!("line:{number}");
        let element = match find_row_or_scroll(number, details.scroll, middle) {
            Ok(row) => build_text_element(&id, text, Rect { y: row, ..middle }),
            Err(step) => {
                build_text_element(&id, text, middle).hide(HiddenBy::Scroll, step.to_owned())
            }
        };
        elements.push(add_back_tap(element));
    }
    elements.push(add_back_tap(build_text_element("hint", BACK_HINT, bottom)));
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
pub(super) fn find_row_or_scroll(
    index: usize,
    first: usize,
    middle: Rect,
) -> Result<u16, &'static str> {
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
fn build_hidden_items(
    screen: &ListScreen<'_>,
    group: Group,
    first: usize,
    hidden_by: HiddenBy,
    show: &str,
    width: usize,
) -> Vec<Element> {
    find_group_items(screen.groups, group)
        .iter()
        .skip(first)
        .map(|item| {
            let element = Element {
                id: format_item_id(item),
                kind: Kind::Item,
                text: join_line_text(&format_group_row(group, item, width, screen.color)),
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
fn find_group_items(groups: &[(Group, Vec<Item>)], group: Group) -> &[Item] {
    groups
        .iter()
        .find(|(each, _)| *each == group)
        .map_or(&[], |(_, items)| items)
}

/// Returns an item's element id, such as `card:DEMO-KB-007`.
fn format_item_id(item: &Item) -> String {
    format!("{}:{}", item.target.pick_kind_word(), item.id)
}

/// Returns the wrapped details lines that fit in `middle`, from
/// `details.scroll` on.
fn select_shown_detail_lines(details: &Details, middle: Rect) -> Vec<String> {
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
            let id = find_group_items(groups, group)
                .get(index)
                .map_or_else(String::new, format_item_id);
            (id, Kind::Item, Some("open"))
        }
        Line::Detail { group, index } => {
            let id = find_group_items(groups, group)
                .get(index)
                .map_or_else(String::new, |item| {
                    format!("detail:{}", format_item_id(item))
                });
            (id, Kind::Item, Some("open"))
        }
        Line::NoHub => ("no-hub".to_owned(), Kind::Text, None),
        Line::ThenReview => ("then:review".to_owned(), Kind::Text, Some("open")),
    }
}

/// Returns an element that shows `text` on `area`'s first row and does
/// nothing on a tap.
pub(super) fn build_text_element(id: &str, text: &str, area: Rect) -> Element {
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
pub(super) fn join_line_text(line: &TextLine<'_>) -> String {
    let text: String = line
        .spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect();
    text.trim_end().to_owned()
}

/// Returns a page's breadcrumb, such as `< 3 ACTIVE`.
pub(super) fn format_page_title(panel: Panel) -> String {
    format!("< {} {}", panel.number(), panel.as_title())
}

/// Returns the ACTIVE counts, such as `23, max 8 · 2 working`, which
/// the glance block and page 3 show. The count turns yellow over the
/// limit, and `max` is left out when `card board` gave no limits.
pub(super) fn format_active_spans(
    groups: &[(Group, Vec<Item>)],
    board: Option<&BoardCounts>,
    color: bool,
) -> Vec<Span<'static>> {
    let active: usize = [Group::Working, Group::NoWorker, Group::Idle]
        .into_iter()
        .map(|group| find_group_items(groups, group).len())
        .sum();
    let working = find_group_items(groups, Group::Working).len();
    let mut spans = format_limit_spans(active, board.map(|board| board.active_limit), color);
    spans.push(Span::raw(COUNT_GAP));
    spans.push(Span::styled(
        format!("{working} working"),
        pick_color_style(Color::Cyan, color),
    ));
    spans
}

/// Returns the IN REVIEW counts, such as `7, max 6 · 1 PR fails`,
/// which the glance block shows. The PR part needs the `prs` tool's
/// list and at least one failing check.
pub(super) fn format_review_spans(
    groups: &[(Group, Vec<Item>)],
    queue: Option<&Queue>,
    color: bool,
) -> Vec<Span<'static>> {
    let count = find_group_items(groups, Group::InReview).len();
    let limit = queue
        .and_then(|queue| queue.board.as_ref())
        .map(|board| board.in_review_limit);
    let mut spans = format_limit_spans(count, limit, color);
    let failing = count_failing_prs(queue);
    if failing > 0 {
        spans.push(Span::raw(COUNT_GAP));
        spans.push(Span::styled(
            match failing {
                1 => "1 PR fails".to_owned(),
                _ => format!("{failing} PRs fail"),
            },
            pick_status_style(Status::Problem, color),
        ));
    }
    spans
}

/// Returns `count` with its limit, such as `23, max 8`, the count
/// yellow when over, or the count alone without a limit.
fn format_limit_spans(count: usize, limit: Option<u32>, color: bool) -> Vec<Span<'static>> {
    let Some(limit) = limit else {
        return vec![Span::raw(count.to_string())];
    };
    let is_over = u32::try_from(count).map_or(true, |count| count > limit);
    let style = if is_over {
        pick_color_style(Color::Yellow, color)
    } else {
        Style::new()
    };
    vec![
        Span::styled(count.to_string(), style),
        Span::raw(format!(", max {limit}")),
    ]
}

/// Returns how many listed pull requests fail a check, 0 without the
/// `prs` tool's list.
fn count_failing_prs(queue: Option<&Queue>) -> usize {
    queue
        .and_then(|queue| queue.pull_requests.as_deref())
        .unwrap_or_default()
        .iter()
        .filter(|pr| pr.check == CheckResult::Fails)
        .count()
}

/// Returns a list page's top line, its breadcrumb and then page 1's
/// waiting count or page 3's ACTIVE counts.
fn format_breadcrumb_line(screen: &ListScreen<'_>) -> TextLine<'static> {
    let bold = Style::new().add_modifier(Modifier::BOLD);
    let mut spans = vec![
        Span::styled(format_page_title(screen.panel), bold),
        Span::raw(BACK_GAP),
    ];
    match screen.panel {
        Panel::Waiting => spans.push(Span::styled(
            find_group_items(screen.groups, Group::NeedsYou)
                .len()
                .to_string(),
            pick_count_style(Group::NeedsYou, screen.color),
        )),
        Panel::Active => spans.extend(format_active_spans(
            screen.groups,
            screen.queue.and_then(|queue| queue.board.as_ref()),
            screen.color,
        )),
        Panel::Since | Panel::Machine | Panel::Review => {}
    }
    TextLine::from(spans)
}

/// Returns one list line, `width` columns wide.
fn format_list_line(screen: &ListScreen<'_>, line: Line, width: usize) -> TextLine<'static> {
    match line {
        Line::Heading { group, is_open } => {
            let count = screen
                .groups
                .iter()
                .find(|(each, _)| *each == group)
                .map_or(0, |(_, items)| {
                    // The `+ N pass, no card` row stands for N pull requests.
                    items
                        .iter()
                        .map(|item| match item.target {
                            Target::PassingPrs(folded) => folded,
                            _ => 1,
                        })
                        .sum()
                });
            let arrow = if is_open { "v" } else { ">" };
            let bold = Style::new().add_modifier(Modifier::BOLD);
            let mut spans = vec![
                Span::styled(arrow, pick_dim_style()),
                Span::styled(format!(" {} ", group.title()), bold),
                Span::styled(count.to_string(), pick_count_style(group, screen.color)),
            ];
            match group {
                Group::InReview => spans.extend(format_review_limit_spans(screen)),
                Group::OtherPrs => spans.extend(format_failing_spans(screen)),
                _ => {}
            }
            TextLine::from(spans)
        }
        Line::More { hidden, .. } => {
            TextLine::styled(format!("   + {hidden} more"), pick_dim_style())
        }
        Line::NoHub => TextLine::styled(format!("   {NO_HUB}"), pick_dim_style()),
        Line::Item { group, index } => {
            let item = screen
                .groups
                .iter()
                .find(|(each, _)| *each == group)
                .and_then(|(_, items)| items.get(index));
            match item {
                Some(item) => format_group_row(group, item, width, screen.color),
                None => TextLine::default(),
            }
        }
        Line::Detail { group, index } => {
            let item = find_group_items(screen.groups, group).get(index);
            let detail = item
                .and_then(|item| item.detail.as_deref())
                .unwrap_or_default();
            let text = fit_text(&format!("   {detail}"), width);
            // A question's detail starts with its age, such as
            // `3 days old, +1 question: Deploy tonight?`.
            let age_end = detail.find([',', ':']).map(|end| end + 3);
            match (item.map(|item| item.tag.as_str()), age_end) {
                (Some("answer"), Some(end)) if end <= text.len() => {
                    let (age, rest) = text.split_at(end);
                    TextLine::from(vec![
                        Span::styled(
                            age.to_owned(),
                            pick_color_style(Color::Yellow, screen.color),
                        ),
                        Span::raw(rest.to_owned()),
                    ])
                }
                _ => TextLine::raw(text),
            }
        }
        Line::ThenReview => {
            let count = find_group_items(screen.groups, Group::InReview).len();
            let noun = if count == 1 { "card" } else { "cards" };
            let text = format!(" then: review {count} {noun}");
            let key = "[5]";
            let room = width.saturating_sub(key.len());
            TextLine::from(vec![
                Span::raw(format!("{:room$}", fit_text(&text, room))),
                Span::styled(key, pick_dim_style()),
            ])
        }
    }
}

/// Returns the IN REVIEW heading's limit, such as `, max 6`, the count
/// before it being drawn already.
fn format_review_limit_spans(screen: &ListScreen<'_>) -> Vec<Span<'static>> {
    match screen.queue.and_then(|queue| queue.board.as_ref()) {
        Some(board) => vec![Span::raw(format!(", max {}", board.in_review_limit))],
        None => Vec::new(),
    }
}

/// Returns the OTHER PULL REQUESTS heading's end, such as
/// ` · 1 failing`, or nothing when none of its rows fails. A failing PR
/// shown under its card is not counted here.
fn format_failing_spans(screen: &ListScreen<'_>) -> Vec<Span<'static>> {
    let failing = find_group_items(screen.groups, Group::OtherPrs)
        .iter()
        .filter(|item| item.status == Status::Problem)
        .count();
    if failing == 0 {
        return Vec::new();
    }
    vec![
        Span::raw(COUNT_GAP),
        Span::styled(
            format!("{failing} failing"),
            pick_status_style(Status::Problem, screen.color),
        ),
    ]
}

/// Returns an item's line, such as ` rev  Retry failed u~ DevEnv`. The
/// line holds its status word, its words cut to fit with `~`, and its
/// tag in the last `TAG_WIDTH` columns.
pub(super) fn format_item_line(item: &Item, width: usize, color: bool) -> TextLine<'static> {
    let room = width.saturating_sub(Status::WIDTH + 3 + TAG_WIDTH);
    let tag: String = item.tag.chars().take(TAG_WIDTH).collect();
    TextLine::from(vec![
        Span::raw(" "),
        Span::styled(
            format!("{:<width$}", item.status.word(), width = Status::WIDTH),
            pick_status_style(item.status, color),
        ),
        Span::raw(" "),
        Span::raw(format!("{:room$}", fit_text(&item.text, room))),
        Span::styled(format!(" {tag}"), pick_tag_style(&item.tag, color)),
    ])
}

/// Returns `item`'s row in `group`. An OTHER PULL REQUESTS row has no
/// status word, since `!`, `run` and `rev` name faults, workers and
/// cards, and ends in its check word. Its `+ N pass, no card` row is
/// dim, like a fold row.
fn format_group_row(group: Group, item: &Item, width: usize, color: bool) -> TextLine<'static> {
    match (group, &item.target) {
        (Group::OtherPrs, Target::PassingPrs(_)) => TextLine::styled(
            fit_text(&format!("   {}", item.text), width),
            pick_dim_style(),
        ),
        (Group::OtherPrs, _) => format_glance_row(&item.text, &item.tag, width, color),
        _ => format_item_line(item, width, color),
    }
}

/// Returns a first-screen row, such as
/// ` DEMO-KB-001 Faster search        answer`, with no status word. The
/// words are cut to fit with `~`, and a `tag` that is not empty ends
/// the row.
pub(super) fn format_glance_row(
    item_text: &str,
    tag: &str,
    width: usize,
    color: bool,
) -> TextLine<'static> {
    let text = format!(" {item_text}");
    if tag.is_empty() {
        return TextLine::raw(fit_text(&text, width));
    }
    let room = width.saturating_sub(tag.chars().count() + 1);
    TextLine::from(vec![
        Span::raw(format!("{:room$}", fit_text(&text, room))),
        Span::styled(format!(" {tag}"), pick_tag_style(tag, color)),
    ])
}

/// Returns a tag's style. `answer` and `see tab` are magenta, a pull
/// request's `fails` red and `pass` green. Other tags, such as a
/// project label, keep the default color.
fn pick_tag_style(tag: &str, color: bool) -> Style {
    match tag {
        "answer" | "see tab" => pick_color_style(Color::Magenta, color),
        "fails" => pick_color_style(Color::Red, color),
        "pass" => pick_color_style(Color::Green, color),
        _ => Style::new(),
    }
}

/// Returns `text` with its first `time` dim and its `word` at the end
/// in the word's color, such as
/// ` 10:40 DEMO-KB-001 Faster search  finished`. A part that `text`
/// lacks, such as a word cut off, stays plain.
pub(super) fn build_event_line(
    text: &str,
    time: Option<&str>,
    word: &str,
    color: bool,
) -> TextLine<'static> {
    let (rest, end) = text
        .strip_suffix(word)
        .map_or((text, ""), |rest| (rest, word));
    let mut spans = Vec::new();
    match time.and_then(|time| Some((rest.find(time)?, time))) {
        Some((start, time)) => {
            spans.push(Span::raw(rest[..start].to_owned()));
            spans.push(Span::styled(time.to_owned(), pick_dim_style()));
            spans.push(Span::raw(rest[start + time.len()..].to_owned()));
        }
        None => spans.push(Span::raw(rest.to_owned())),
    }
    spans.push(Span::styled(end.to_owned(), pick_event_style(word, color)));
    TextLine::from(spans)
}

/// Returns an event word's style, green for `finished`, cyan for
/// `new worker` and plain for others.
pub(super) fn pick_event_style(word: &str, color: bool) -> Style {
    match word {
        "finished" => pick_color_style(Color::Green, color),
        "new worker" => pick_color_style(Color::Cyan, color),
        _ => Style::new(),
    }
}

/// Cuts `text` to `room` columns, ending a cut text with `~`.
fn fit_text(text: &str, room: usize) -> String {
    if text.chars().count() > room {
        let kept: String = text.chars().take(room.saturating_sub(1)).collect();
        return format!("{kept}~");
    }
    text.to_owned()
}

/// Returns a status word's style, dim for `idle` and otherwise in the
/// word's color unless `color` is false.
pub(super) fn pick_status_style(status: Status, color: bool) -> Style {
    match status {
        Status::Idle => pick_dim_style(),
        status => pick_color_style(status.color(), color),
    }
}

/// Returns the style of a group heading's count, dim for IDLE and
/// DRAFT and otherwise in the group's color unless `color` is false.
pub(super) fn pick_count_style(group: Group, color: bool) -> Style {
    match group {
        Group::Idle | Group::Draft => pick_dim_style(),
        group => pick_color_style(group.color(), color),
    }
}

/// Returns the style that draws in `fg`, or the terminal's own colors
/// when `color` is false.
pub(super) fn pick_color_style(fg: Color, color: bool) -> Style {
    if color {
        Style::new().fg(fg)
    } else {
        Style::new()
    }
}

/// Returns the style of hints and tags.
pub(super) fn pick_dim_style() -> Style {
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
