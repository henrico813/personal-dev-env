//! Draws the first screen, which the board opens on. Its four blocks
//! say what waits on the user, what happened since the last visit,
//! what is at work and what is in review. Fault rows come first. Each
//! block shows a fixed number of rows, then a fold row that sums up
//! the rest. Below 100 columns the blocks are stacked; from 100 on,
//! WAITING ON YOU and SINCE YOU LEFT sit left of ACTIVE and IN REVIEW.
//! The drawn rows are the board's list lines, so keys, taps and the
//! chosen row work as on pages 1, 3 and 5. Read `draw_glance`,
//! `list_glance_lines` and `build_glance_elements` first.

use jiff::{SignedDuration, Timestamp};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line as TextLine, Span};
use ratatui::widgets::{List, ListItem, ListState, Paragraph};

use super::away;
use super::draw;
use super::elements::{Action, Element, HiddenBy, Kind, Place};
use super::groups::{Group, Item, Status, Target};
use super::panels::{Panel, WIDE_COLUMNS};
use super::queue::{CardLine, NO_HUB, Queue};
use super::sessions::fit;
use super::since::{self, CardEvents};
use super::view::Line;
use crate::shared::format_span;
use crate::shared::lifecycle;

/// What sits between the two columns from 100 columns on.
const GAP: &str = " │ ";

/// The space between the bottom line's parts from 100 columns on.
const HINT_GAP: &str = "  ";

/// The bottom line below 100 columns. `HELP_HINT` sits at its right end.
const PHONE_HINT: &str = "4 sessions and machine";

const HELP_HINT: &str = "? help";

/// The bottom line's parts from 100 columns on, each with its tap.
const WIDE_HINT: [(&str, Action); 7] = [
    ("1 waiting", Action::Open(Panel::Waiting)),
    ("2 since", Action::Open(Panel::Since)),
    ("3 active", Action::Open(Panel::Active)),
    ("4 sessions and machine", Action::Open(Panel::Machine)),
    ("5 review", Action::Open(Panel::Review)),
    (HELP_HINT, Action::Help),
    ("q quit", Action::None),
];

/// What the first screen shows.
pub(super) struct Glance<'a> {
    pub groups: &'a [(Group, Vec<Item>)],
    /// The last read, or `None` before the first.
    pub queue: Option<&'a Queue>,
    /// The top line's end, such as `read 10:42 0.6s`.
    pub status: &'a str,
    /// The start of the user's visit, or `None` with no saved time.
    pub since: Option<Timestamp>,
    /// `since` as `HH:MM` in the board's time zone.
    pub since_time: Option<&'a str>,
    /// How long the user was away before this visit.
    pub away_for: Option<SignedDuration>,
    /// The place of the board's chosen line, drawn reversed.
    pub chosen: Option<usize>,
    /// How many rows each column was scrolled down at the last draw.
    pub scroll: [usize; 2],
    pub color: bool,
}

/// The screen's areas. There are two columns from 100 columns on, and
/// `gap` is empty below that.
struct Parts {
    top: Rect,
    columns: Vec<Rect>,
    gap: Rect,
    hint: Rect,
}

/// One line of a column, and the page a tap on it opens.
struct GlanceLine {
    id: String,
    kind: Kind,
    text: TextLine<'static>,
    /// `None` on the blank line between two blocks.
    opens: Option<Panel>,
    /// The list line Enter and a tap act on, or `None` on a line that
    /// cannot be chosen, such as a blank line or `nothing new`.
    line: Option<Line>,
    /// The place of `line` in the board's lines, which a tap passes to
    /// `Board::activate`, or `None` on a line that cannot be chosen or
    /// is folded.
    place: Option<usize>,
    /// True on a row that its block's fold row sums up, which is not
    /// drawn.
    is_folded: bool,
}

/// Draws the first screen over `area` and returns how far each column
/// is scrolled. Each column is drawn as the list pages draw theirs, so
/// it scrolls to keep the chosen line in view and draws it reversed.
pub(super) fn draw_glance(frame: &mut Frame, area: Rect, glance: &Glance<'_>) -> [usize; 2] {
    let parts = split_glance(area);
    frame.render_widget(Paragraph::new(format_top_line(glance)), parts.top);
    let mut scroll = glance.scroll;
    let columns = list_column_lines(glance, &parts.columns);
    for ((lines, column), first) in columns.into_iter().zip(&parts.columns).zip(&mut scroll) {
        let drawn: Vec<GlanceLine> = lines.into_iter().filter(|line| !line.is_folded).collect();
        let chosen_row = drawn.iter().position(|line| is_chosen(line, glance));
        let items: Vec<ListItem> = drawn
            .into_iter()
            .map(|line| ListItem::new(line.text))
            .collect();
        let mut state = ListState::default()
            .with_offset(*first)
            .with_selected(chosen_row);
        let list = List::new(items).highlight_style(draw::pick_chosen_style());
        frame.render_stateful_widget(list, *column, &mut state);
        *first = state.offset();
    }
    let bars = vec![TextLine::from(GAP); usize::from(parts.gap.height)];
    frame.render_widget(
        Paragraph::new(bars).style(draw::pick_dim_style()),
        parts.gap,
    );
    frame.render_widget(
        Paragraph::new(format_hint_text(parts.hint.width)).style(draw::pick_dim_style()),
        parts.hint,
    );
    scroll
}

/// Returns the first screen's elements as `draw_glance` drew them in
/// `area`. A tap on a line that can be chosen acts on it as Enter does,
/// and a tap on another line of a block opens its page. A row the fold
/// row sums up comes hidden, with its page's key as the step that shows
/// it, such as `press 3`. A row scrolled off comes hidden with the page
/// key that scrolls toward it, as on the list pages.
pub(super) fn build_glance_elements(area: Rect, glance: &Glance<'_>) -> Vec<Element> {
    let parts = split_glance(area);
    let mut elements = vec![draw::build_text_element(
        "status",
        &format_top_line(glance),
        parts.top,
    )];
    let columns = list_column_lines(glance, &parts.columns);
    for ((lines, column), first) in columns.into_iter().zip(&parts.columns).zip(glance.scroll) {
        let mut row = 0;
        for line in lines {
            let drawn_row = (!line.is_folded).then_some(row);
            if !line.is_folded {
                row += 1;
            }
            let Some(panel) = line.opens else {
                continue;
            };
            let tap = line.line.map_or(Some("open"), |each| {
                draw::describe_line(glance.groups, each).2
            });
            let element = Element {
                kind: line.kind,
                tap,
                chosen: is_chosen(&line, glance),
                action: line.place.map_or(Action::Open(panel), Action::Line),
                ..draw::build_text_element(&line.id, &draw::join_line_text(&line.text), *column)
            };
            let Some(drawn_row) = drawn_row else {
                let show = format!("press {}", panel.number());
                elements.push(element.hide(HiddenBy::More, show));
                continue;
            };
            elements.push(match draw::find_row_or_scroll(drawn_row, first, *column) {
                Ok(y) => Element {
                    place: Some(Place {
                        column: column.x,
                        row: y,
                        width: column.width,
                    }),
                    ..element
                },
                Err(step) => element.hide(HiddenBy::Scroll, step.to_owned()),
            });
        }
    }
    for (column, words, action) in list_hint_parts(parts.hint.width) {
        let id = match action {
            Action::Open(panel) => format!("open:{}", panel.slug()),
            Action::Help => "help".to_owned(),
            _ => "quit".to_owned(),
        };
        let place = Rect {
            x: parts.hint.x + column,
            width: draw::measure_width(words),
            ..parts.hint
        };
        elements.push(Element {
            tap: (action != Action::None).then_some("open"),
            action,
            ..draw::build_text_element(&id, words, place)
        });
    }
    elements
}

/// Returns the lines that can be chosen, in their `GlanceLine::place`
/// order: the fault rows, then each block's heading, its drawn rows
/// and their detail lines, and its fold row. From 100 columns the left
/// column comes first, then the right. j and k move in this order.
pub(super) fn list_glance_lines(area: Rect, glance: &Glance<'_>) -> Vec<Line> {
    let parts = split_glance(area);
    list_column_lines(glance, &parts.columns)
        .into_iter()
        .flatten()
        .filter(|line| line.place.is_some())
        .filter_map(|line| line.line)
        .collect()
}

/// Returns true when `line` is the board's chosen line.
fn is_chosen(line: &GlanceLine, glance: &Glance<'_>) -> bool {
    line.place.is_some() && line.place == glance.chosen
}

/// Returns `name` with `end` at the right end of `width` columns, such
/// as `hub watcher  ok`. `name` is cut with `~` to leave room for `end`.
pub(super) fn join_row_end(name: &str, end: &str, width: usize) -> String {
    let room = width.saturating_sub(end.chars().count() + 1);
    format!("{:room$} {end}", fit(name, room))
}

/// Returns the top line, such as `away 1h29m · read 10:42 0.6s`, with
/// the board's status last.
fn format_top_line(glance: &Glance<'_>) -> String {
    match glance.away_for {
        Some(away_for) => format!("away {} · {}", format_span(away_for), glance.status),
        None => glance.status.to_owned(),
    }
}

/// Returns the bottom line's parts for `width` columns, each with the
/// column it starts at. Below 100 columns that is `4 sessions and
/// machine` with `? help` at the right end; from 100 on, every key.
fn list_hint_parts(width: u16) -> Vec<(u16, &'static str, Action)> {
    if width < WIDE_COLUMNS {
        let phone_width = draw::measure_width(PHONE_HINT);
        let help_column = width
            .saturating_sub(draw::measure_width(HELP_HINT))
            .max(phone_width + 2);
        return vec![
            (0, PHONE_HINT, Action::Open(Panel::Machine)),
            (help_column, HELP_HINT, Action::Help),
        ];
    }
    let mut column = 0;
    let mut parts = Vec::new();
    for (words, action) in WIDE_HINT {
        parts.push((column, words, action));
        column += draw::measure_width(words) + draw::measure_width(HINT_GAP);
    }
    parts
}

/// Returns the bottom line's words for `width` columns, as
/// `list_hint_parts` places them.
fn format_hint_text(width: u16) -> String {
    let mut text = String::new();
    for (column, words, _) in list_hint_parts(width) {
        let pad = usize::from(column).saturating_sub(text.chars().count());
        text.push_str(&" ".repeat(pad));
        text.push_str(words);
    }
    text
}

/// Returns the top line's, the columns' and the bottom line's areas.
fn split_glance(area: Rect) -> Parts {
    let [top, body, hint] = draw::split_screen(area);
    if area.width < WIDE_COLUMNS {
        return Parts {
            top,
            columns: vec![body],
            gap: Rect::default(),
            hint,
        };
    }
    let gap = Constraint::Length(draw::measure_width(GAP));
    let fill = Constraint::Fill(1);
    let [left, gap, right] = Layout::horizontal([fill, gap, fill]).areas(body);
    Parts {
        top,
        columns: vec![left, right],
        gap,
        hint,
    }
}

/// Returns the lines of each of `columns`, top to bottom, the rows that
/// fold rows sum up included. One column holds the fault rows and the
/// four blocks; two hold the fault rows, WAITING ON YOU and SINCE YOU
/// LEFT on the left, ACTIVE and IN REVIEW on the right. Each drawn
/// line that can be chosen gets its `place`, left column first.
fn list_column_lines(glance: &Glance<'_>, columns: &[Rect]) -> Vec<Vec<GlanceLine>> {
    let mut lines = build_column_lines(glance, columns);
    let choosable = lines
        .iter_mut()
        .flatten()
        .filter(|line| line.line.is_some() && !line.is_folded);
    for (place, line) in choosable.enumerate() {
        line.place = Some(place);
    }
    lines
}

/// Returns the lines of each of `columns` for `list_column_lines`, not
/// yet numbered.
fn build_column_lines(glance: &Glance<'_>, columns: &[Rect]) -> Vec<Vec<GlanceLine>> {
    let is_wide = columns.len() > 1;
    let width = |column: usize| usize::from(columns[column].width);
    let blank = || GlanceLine {
        id: String::new(),
        kind: Kind::Text,
        text: TextLine::default(),
        opens: None,
        line: None,
        place: None,
        is_folded: false,
    };
    let mut left = list_fault_lines(glance, width(0));
    left.extend(build_block(Panel::Waiting, glance, is_wide, width(0)));
    left.push(blank());
    left.extend(build_block(Panel::Since, glance, is_wide, width(0)));
    if !is_wide {
        left.extend(build_block(Panel::Active, glance, false, width(0)));
        left.extend(build_block(Panel::Review, glance, false, width(0)));
        return vec![left];
    }
    let mut right = build_block(Panel::Active, glance, true, width(1));
    right.push(blank());
    right.extend(build_block(Panel::Review, glance, true, width(1)));
    vec![left, right]
}

/// Returns a red `!` row for each fault the read found, such as
/// `! hub watcher stopped`. A tap on one opens page 4.
fn list_fault_lines(glance: &Glance<'_>, width: usize) -> Vec<GlanceLine> {
    let problems = glance
        .queue
        .map_or(&[][..], |queue| queue.problems.as_slice());
    let style = draw::pick_status_style(Status::Problem, glance.color);
    problems
        .iter()
        .enumerate()
        .map(|(number, problem)| GlanceLine {
            id: format!("fault:{number}"),
            kind: Kind::Text,
            text: TextLine::styled(fit(&format!("! {problem}"), width), style),
            opens: Some(Panel::Machine),
            line: Some(Line::Fault(number)),
            place: None,
            is_folded: false,
        })
        .collect()
}

/// Returns how many rows `panel`'s block shows before its fold row.
/// Below 100 columns the counts fit the four blocks in 40 by 16, and
/// IN REVIEW shows only its heading.
fn count_shown_rows(panel: Panel, is_wide: bool) -> usize {
    match (panel, is_wide) {
        (Panel::Waiting, false) => 3,
        (Panel::Since, false) => 1,
        (Panel::Active, _) => 2,
        (Panel::Review, false) | (Panel::Machine, _) => 0,
        (Panel::Waiting | Panel::Review, true) => 6,
        (Panel::Since, true) => 7,
    }
}

/// Returns `panel`'s block, `width` columns wide, as its heading, every
/// row with those past `count_shown_rows` folded, then the fold row.
/// From 100 columns a row may have a detail line below it.
/// A block that shows no rows has no fold row either.
fn build_block(panel: Panel, glance: &Glance<'_>, is_wide: bool, width: usize) -> Vec<GlanceLine> {
    let slug = panel.slug();
    let make_line = |id: String, kind, text, line| GlanceLine {
        id,
        kind,
        text,
        opens: Some(panel),
        line,
        place: None,
        is_folded: false,
    };
    let shown = count_shown_rows(panel, is_wide);
    let (rows, fold_parts) = list_block_rows(panel, glance, shown, width);
    let has_fold = shown > 0 && rows.len() > shown;
    // A block with nothing to list has one dim row, such as
    // `nothing new`, which has no list line.
    let row_lines = list_row_lines(panel, glance);
    let mut lines = vec![make_line(
        format!("block:{slug}"),
        Kind::Heading,
        format_heading_line(panel, glance, width),
        Some(Line::Block(panel)),
    )];
    let mut details = if is_wide {
        list_detail_lines(panel, glance, width)
    } else {
        Vec::new()
    }
    .into_iter();
    for (number, row) in rows.into_iter().enumerate() {
        let is_folded = number >= shown;
        let row_line = row_lines.get(number).copied();
        lines.push(GlanceLine {
            is_folded,
            ..make_line(format!("{slug}:{number}"), Kind::Text, row, row_line)
        });
        if let Some(Some(detail)) = details.next() {
            let detail_line = match row_line {
                Some(Line::Item { group, index }) => Some(Line::Detail { group, index }),
                _ => None,
            };
            lines.push(GlanceLine {
                is_folded,
                ..make_line(
                    format!("{slug}:{number}:detail"),
                    Kind::Text,
                    detail,
                    detail_line,
                )
            });
        }
    }
    if has_fold {
        let mut spans = vec![Span::styled("   + ", draw::pick_dim_style())];
        spans.extend(fold_parts);
        let text = fit_spans(spans, width);
        let fold = Some(Line::Fold(panel));
        lines.push(make_line(format!("more:{slug}"), Kind::More, text, fold));
    }
    lines
}

/// Returns a block's heading with its bold name, its counts and its key,
/// such as `[3]`, at the right end of `width`. The counts wait for the
/// first read.
fn format_heading_line(panel: Panel, glance: &Glance<'_>, width: usize) -> TextLine<'static> {
    // Page 5's title also names pull requests, which the block leaves
    // to its page.
    let mut name = match panel {
        Panel::Review => Group::InReview.title().to_owned(),
        _ => panel.as_title().to_owned(),
    };
    let mut counts = Vec::new();
    if let Some(queue) = glance.queue {
        counts = match panel {
            Panel::Waiting => {
                let count = find_items(glance.groups, &[Group::NeedsYou]).len();
                let style = draw::pick_count_style(Group::NeedsYou, glance.color);
                vec![Span::styled(count.to_string(), style)]
            }
            Panel::Since => {
                let events = away::find_events_since(&queue.events, glance.since);
                let count = since::group_events_by_card(events).len();
                match glance.since_time {
                    Some(time) => {
                        name = format!("{name} AT {time}");
                        vec![Span::raw(format_count(count, "card", "cards"))]
                    }
                    None => vec![Span::raw("newest")],
                }
            }
            Panel::Active => {
                draw::format_active_spans(glance.groups, queue.board.as_ref(), glance.color)
            }
            Panel::Review => draw::format_review_spans(glance.groups, Some(queue), glance.color),
            Panel::Machine => Vec::new(),
        };
    }
    let bold = Style::new().add_modifier(Modifier::BOLD);
    let mut spans = vec![Span::styled(name, bold), Span::raw("  ")];
    spans.extend(counts);
    let key = format!("[{}]", panel.number());
    let used: usize = spans.iter().map(Span::width).sum();
    let gap = width.saturating_sub(used + key.len()).max(1);
    spans.push(Span::raw(" ".repeat(gap)));
    spans.push(Span::styled(key, draw::pick_dim_style()));
    TextLine::from(spans)
}

/// Returns every row of `panel`'s block, and the words of its fold row
/// for the rows past the first `shown`, each part in its style. A
/// block with no rows has one dim row that says why, such as
/// `no hub running`.
fn list_block_rows(
    panel: Panel,
    glance: &Glance<'_>,
    shown: usize,
    width: usize,
) -> (Vec<TextLine<'static>>, Vec<Span<'static>>) {
    let dim = draw::pick_dim_style();
    let dim_row = |words: &str| TextLine::styled(format!(" {words}"), dim);
    let Some(queue) = glance.queue else {
        return (vec![dim_row("reading")], Vec::new());
    };
    // The first screen's WAITING ON YOU rows keep only their tag, and
    // its ACTIVE rows neither status word nor project label.
    let format_rows = |items: &[&Item], keep_tag: bool| -> Vec<TextLine<'static>> {
        items
            .iter()
            .map(|item| {
                let tag = if keep_tag { item.tag.as_str() } else { "" };
                draw::format_glance_row(&item.text, tag, width, glance.color)
            })
            .collect()
    };
    let (rows, fold_parts, empty) = match panel {
        Panel::Waiting => {
            let items = find_items(glance.groups, list_block_groups(panel));
            let hidden = items.get(shown..).unwrap_or_default();
            let fold_parts = describe_waiting_fold(hidden, queue, glance.color);
            // With no hub, no session can wait on the user yet, so the
            // empty block must not read as if nothing does.
            let empty = if queue.no_hub {
                NO_HUB
            } else {
                "nothing waits on you"
            };
            (format_rows(&items, true), fold_parts, empty)
        }
        Panel::Since => {
            let events = away::find_events_since(&queue.events, glance.since);
            let cards = since::group_events_by_card(events);
            let rows = cards
                .iter()
                .map(|row| format_since_row(row, &queue.cards, width, glance.color))
                .collect();
            let hidden = cards.get(shown..).unwrap_or_default();
            (rows, count_event_words(hidden, glance.color), "nothing new")
        }
        Panel::Active => {
            let items = find_items(glance.groups, list_block_groups(panel));
            let fold_words = describe_active_fold(items.get(shown..).unwrap_or_default());
            let fold_parts = vec![Span::styled(fold_words, dim)];
            (format_rows(&items, false), fold_parts, "no active card")
        }
        Panel::Review => {
            let items = find_items(glance.groups, list_block_groups(panel));
            let hidden = items.len().saturating_sub(shown);
            let rows = items
                .iter()
                .map(|item| draw::format_item_line(item, width, glance.color))
                .collect();
            (rows, vec![Span::styled(format!("{hidden} more"), dim)], "")
        }
        Panel::Machine => (Vec::new(), Vec::new(), ""),
    };
    if rows.is_empty() && shown > 0 {
        return (vec![dim_row(empty)], fold_parts);
    }
    (rows, fold_parts)
}

/// Returns the dim line under each WAITING ON YOU or ACTIVE row of the
/// full screen, in row order, or `None` for a row with nothing to say.
/// A WAITING ON YOU row shows its item's detail, such as
/// `16 days old, +3 questions: Who updates the docs?`. An ACTIVE row
/// shows its card's newest event since the visit, such as
/// `19:43: new worker`, from the events SINCE YOU LEFT reads.
fn list_detail_lines(
    panel: Panel,
    glance: &Glance<'_>,
    width: usize,
) -> Vec<Option<TextLine<'static>>> {
    let Some(queue) = glance.queue else {
        return Vec::new();
    };
    let dim_line =
        |text: &str| TextLine::styled(fit(&format!("   {text}"), width), draw::pick_dim_style());
    match panel {
        Panel::Waiting => find_items(glance.groups, list_block_groups(panel))
            .iter()
            .map(|item| item.detail.as_deref().map(&dim_line))
            .collect(),
        Panel::Active => {
            let events = away::find_events_since(&queue.events, glance.since);
            let cards = since::group_events_by_card(events);
            find_items(glance.groups, list_block_groups(panel))
                .iter()
                .map(|item| {
                    let row = cards.iter().find(|row| {
                        row.card
                            .as_ref()
                            .is_some_and(|card| card.as_str() == item.id)
                    })?;
                    let newest = row.events[0];
                    Some(dim_line(&format!(
                        "{}: {}",
                        newest.time,
                        since::describe_event(newest)
                    )))
                })
                .collect()
        }
        Panel::Since | Panel::Review | Panel::Machine => Vec::new(),
    }
}

/// Returns the groups whose items `panel`'s block lists, in row order.
/// SINCE YOU LEFT lists events, not items, so it has none.
pub(super) fn list_block_groups(panel: Panel) -> &'static [Group] {
    match panel {
        Panel::Waiting => &[Group::NeedsYou],
        Panel::Active => &[Group::Working, Group::NoWorker, Group::Idle],
        Panel::Review => &[Group::InReview],
        Panel::Since | Panel::Machine => &[],
    }
}

/// Returns the list line of each row `panel`'s block can show, in row
/// order: a SINCE YOU LEFT row by its place, any other row as its item.
fn list_row_lines(panel: Panel, glance: &Glance<'_>) -> Vec<Line> {
    if panel == Panel::Since {
        let count = glance.queue.map_or(0, |queue| {
            let events = away::find_events_since(&queue.events, glance.since);
            since::group_events_by_card(events).len()
        });
        return (0..count).map(Line::Event).collect();
    }
    list_block_groups(panel)
        .iter()
        .filter_map(|group| glance.groups.iter().find(|(each, _)| each == group))
        .flat_map(|(group, items)| {
            (0..items.len()).map(|index| Line::Item {
                group: *group,
                index,
            })
        })
        .collect()
}

/// Returns the items of `wanted` groups, in that order.
fn find_items<'a>(groups: &'a [(Group, Vec<Item>)], wanted: &[Group]) -> Vec<&'a Item> {
    wanted
        .iter()
        .filter_map(|group| groups.iter().find(|(each, _)| each == group))
        .flat_map(|(_, items)| items)
        .collect()
}

/// Returns a SINCE YOU LEFT row, such as
/// ` 10:40 DEMO-KB-001 Faster search  report`, with its card's newest
/// event's time and word.
fn format_since_row(
    row: &CardEvents<'_>,
    cards: &[CardLine],
    width: usize,
    color: bool,
) -> TextLine<'static> {
    let newest = row.events[0];
    let word = since::describe_event(newest);
    let name = format!(" {} {}", newest.time, row.describe_card(cards));
    let text = join_row_end(&name, word, width);
    draw::build_event_line(&text, Some(&newest.time), word, color)
}

/// Returns WAITING ON YOU's fold words for its `hidden` rows, such as
/// `run 24 hub and 2 old questions`. An auto run's hub is named by its
/// run, other sessions count as tabs, and questions count as old when
/// none of them was asked today. Old questions are yellow, like a
/// question's age on page 1, and the other words dim.
fn describe_waiting_fold(hidden: &[&Item], queue: &Queue, color: bool) -> Vec<Span<'static>> {
    let dim = draw::pick_dim_style();
    let mut hubs = Vec::new();
    let mut tabs = 0;
    let mut questions = Vec::new();
    for item in hidden {
        match item.target {
            Target::Session(_) => match lifecycle::parse_auto_hub_number(&item.id) {
                Some(run) => hubs.push(Span::styled(format!("run {run} hub"), dim)),
                None => tabs += 1,
            },
            Target::Question(index) => questions.push(&queue.questions[index]),
            _ => {}
        }
    }
    let is_old = questions
        .iter()
        .all(|question| question.asked_on != Some(queue.read_on));
    let (one, many, question_style) = if is_old {
        let yellow = draw::pick_color_style(Color::Yellow, color);
        (
            "old question",
            "old questions",
            pick_fold_part_style(yellow),
        )
    } else {
        ("question", "questions", dim)
    };
    let mut parts = hubs;
    if tabs > 0 {
        parts.push(Span::styled(format_count(tabs, "tab", "tabs"), dim));
    }
    if !questions.is_empty() {
        let words = format_count(questions.len(), one, many);
        parts.push(Span::styled(words, question_style));
    }
    join_with_and(parts)
}

/// Joins `parts` as `a, b and c`, with dim words between them.
fn join_with_and(parts: Vec<Span<'static>>) -> Vec<Span<'static>> {
    let last = parts.len().saturating_sub(1);
    let mut joined = Vec::new();
    for (number, part) in parts.into_iter().enumerate() {
        if number > 0 {
            let between = if number == last { " and " } else { ", " };
            joined.push(Span::styled(between, draw::pick_dim_style()));
        }
        joined.push(part);
    }
    joined
}

/// Returns SINCE YOU LEFT's fold words for its `hidden` cards, each
/// counted by its newest event's word, such as `3 reports, 2 new
/// workers`. An event with no known word counts as `other`. Each
/// count takes its word's color from the event rows, such as cyan for
/// `new worker`, and a word with no color stays dim.
fn count_event_words(hidden: &[CardEvents<'_>], color: bool) -> Vec<Span<'static>> {
    const WORDS: [(&str, &str); 4] = [
        ("report", "reports"),
        ("new worker", "new workers"),
        ("finished", "finished"),
        ("other", "other"),
    ];
    let mut counts = [0; WORDS.len()];
    for row in hidden {
        let word = since::describe_event(row.events[0]);
        let index = WORDS
            .iter()
            .position(|(one, _)| *one == word)
            .unwrap_or(WORDS.len() - 1);
        counts[index] += 1;
    }
    let mut parts = Vec::new();
    for ((one, many), count) in WORDS.iter().zip(counts) {
        if count == 0 {
            continue;
        }
        if !parts.is_empty() {
            parts.push(Span::styled(", ", draw::pick_dim_style()));
        }
        let style = pick_fold_part_style(draw::pick_event_style(one, color));
        parts.push(Span::styled(format_count(count, one, many), style));
    }
    parts
}

/// Returns `style` for a fold row part that has a color, or dim for
/// one with none, such as `report` or any part when color is off.
fn pick_fold_part_style(style: Style) -> Style {
    if style.fg.is_some() {
        style
    } else {
        draw::pick_dim_style()
    }
}

/// Returns `spans` as one line cut to `width` columns, as `fit` cuts
/// text. A cut line ends with `~` in the style of the span it cuts.
fn fit_spans(spans: Vec<Span<'static>>, width: usize) -> TextLine<'static> {
    let total: usize = spans.iter().map(|span| span.content.chars().count()).sum();
    if total <= width {
        return TextLine::from(spans);
    }
    let mut room = width.saturating_sub(1);
    let mut kept = Vec::new();
    for span in spans {
        let count = span.content.chars().count();
        if count < room {
            room -= count;
            kept.push(span);
            continue;
        }
        let cut: String = span.content.chars().take(room).collect();
        kept.push(Span::styled(format!("{cut}~"), span.style));
        break;
    }
    TextLine::from(kept)
}

/// Returns ACTIVE's fold words for its `hidden` rows, such as
/// `21 more, none at work` or `3 more, 1 at work`.
fn describe_active_fold(hidden: &[&Item]) -> String {
    let at_work = hidden
        .iter()
        .filter(|item| item.status == Status::Run)
        .count();
    match at_work {
        0 => format!("{} more, none at work", hidden.len()),
        at_work => format!("{} more, {at_work} at work", hidden.len()),
    }
}

/// Returns `count` with the noun that fits it, such as `1 card` or
/// `7 cards`.
fn format_count(count: usize, one: &str, many: &str) -> String {
    let noun = if count == 1 { one } else { many };
    format!("{count} {noun}")
}
