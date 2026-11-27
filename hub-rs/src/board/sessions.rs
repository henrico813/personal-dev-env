//! Draws the sessions list of page 4, which groups every session by
//! the verdict the tidy pass acts on, above the jump bar. Kept, safe
//! to close and parked sessions fold into one row. Read
//! `format_session_lines` and `build_session_elements` first.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::Line as TextLine;
use ratatui::widgets::{List, ListItem, ListState, Paragraph};

use super::draw;
use super::elements::{Action, Element, HiddenBy, Kind};
use super::queue::SessionRow;
use crate::shared::context::HANDOFF_TOKENS;
use crate::shared::lifecycle::Lifecycle;

/// The most columns a session's name takes, so the bars line up.
const NAME_WIDTH: usize = 19;

/// The jump bar's words before a session is chosen.
const CHOOSE_HINT: &str = "tap a session to choose it";

/// The element id of the row that folds the done groups.
const FOLD_ID: &str = "sessions:fold";

/// The groups of the sessions panel, in the order it shows them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SessionGroup {
    NeedsYou,
    Working,
    Kept,
    SafeToClose,
    Parked,
}

impl SessionGroup {
    /// The groups shown above the fold row.
    const OPEN: [SessionGroup; 2] = [SessionGroup::NeedsYou, SessionGroup::Working];

    /// The groups the fold row counts, shown below it once unfolded.
    const DONE: [SessionGroup; 3] = [
        SessionGroup::Kept,
        SessionGroup::SafeToClose,
        SessionGroup::Parked,
    ];

    /// Returns the group of a session in `state`. Safe to close holds
    /// every session the clean pass closes or parks.
    pub(super) fn from_state(state: Lifecycle) -> SessionGroup {
        match state {
            Lifecycle::WaitsOnUser => SessionGroup::NeedsYou,
            Lifecycle::Working | Lifecycle::WaitsOnHub => SessionGroup::Working,
            Lifecycle::Kept(_) => SessionGroup::Kept,
            Lifecycle::ToClose | Lifecycle::ToPark => SessionGroup::SafeToClose,
            Lifecycle::Parked => SessionGroup::Parked,
        }
    }

    /// Returns the heading's words, upper case so a heading stands out
    /// from rows without color. Working holds sessions that wait on the
    /// hub too, so its words do not say working alone.
    fn title(self) -> &'static str {
        match self {
            SessionGroup::NeedsYou => "NEEDS YOU",
            SessionGroup::Working => "AT WORK OR WAITING ON THE HUB",
            SessionGroup::Kept => "KEPT",
            SessionGroup::SafeToClose => "SAFE TO CLOSE",
            SessionGroup::Parked => "PARKED",
        }
    }
}

/// One line of the sessions panel.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct PanelLine {
    /// The row's place in the queue's sessions, or `None` on a heading.
    pub session: Option<usize>,
    /// True on the extra line that holds a row's why, when the why did
    /// not fit beside the row.
    pub is_why: bool,
    /// True on the row that folds the kept, safe to close and parked
    /// groups.
    pub is_fold: bool,
    pub text: String,
}

/// Returns each group that holds a session as a heading with its
/// count, followed by its rows, each cut to `width`. When it fits, the
/// AT WORK OR WAITING ON THE HUB heading adds how many of its sessions
/// work right now, such as `· 1 working`. A why too long
/// for its row goes on the line below it. Empty groups are left out,
/// so a phone screen holds more rows.
///
/// Kept, safe to close and parked sessions end in one fold row, such
/// as `> KEPT 11 · SAFE TO CLOSE 3 · PARKED 18`. With `show_done` the
/// row starts with `v` and their groups follow it.
pub(super) fn format_session_lines(
    sessions: &[SessionRow],
    width: usize,
    show_done: bool,
) -> Vec<PanelLine> {
    let mut lines = Vec::new();
    for group in SessionGroup::OPEN {
        push_group_lines(&mut lines, sessions, group, width);
    }
    let counts: Vec<String> = SessionGroup::DONE
        .into_iter()
        .map(|group| (group, list_group_members(sessions, group).len()))
        .filter(|&(_, count)| count > 0)
        .map(|(group, count)| format!("{} {count}", group.title()))
        .collect();
    if counts.is_empty() {
        return lines;
    }
    let mark = if show_done { "v" } else { ">" };
    lines.push(PanelLine {
        session: None,
        is_why: false,
        is_fold: true,
        text: fit(&format!("{mark} {}", counts.join(" · ")), width),
    });
    if show_done {
        for group in SessionGroup::DONE {
            push_group_lines(&mut lines, sessions, group, width);
        }
    }
    lines
}

/// Returns the shown sessions' indexes in the order the panel shows
/// them, which j and k follow. Folded sessions are left out, so j and
/// k never choose a row the user cannot see.
pub(super) fn order_shown_sessions(sessions: &[SessionRow], show_done: bool) -> Vec<usize> {
    let done: &[SessionGroup] = if show_done { &SessionGroup::DONE } else { &[] };
    SessionGroup::OPEN
        .iter()
        .chain(done)
        .flat_map(|&group| list_group_members(sessions, group))
        .collect()
}

/// Pushes `group`'s heading and rows onto `lines`, or nothing when the
/// group is empty.
fn push_group_lines(
    lines: &mut Vec<PanelLine>,
    sessions: &[SessionRow],
    group: SessionGroup,
    width: usize,
) {
    let members = list_group_members(sessions, group);
    if members.is_empty() {
        return;
    }
    let mut heading = format!("{} {}", group.title(), members.len());
    if group == SessionGroup::Working {
        let working = members
            .iter()
            .filter(|&&index| sessions[index].working)
            .count();
        let counted = format!("{heading} · {working} working");
        // At 40 columns the long title leaves no room for the count.
        if counted.chars().count() <= width {
            heading = counted;
        }
    }
    lines.push(PanelLine {
        session: None,
        is_why: false,
        is_fold: false,
        text: heading,
    });
    for index in members {
        let row = describe_session(&sessions[index]);
        let why = describe_why(&sessions[index]);
        let beside = format!("{row} {why}");
        if beside.chars().count() <= width {
            lines.push(PanelLine {
                session: Some(index),
                is_why: false,
                is_fold: false,
                text: beside,
            });
            continue;
        }
        lines.push(PanelLine {
            session: Some(index),
            is_why: false,
            is_fold: false,
            text: fit(&row, width),
        });
        lines.push(PanelLine {
            session: Some(index),
            is_why: true,
            is_fold: false,
            text: fit(&format!("  {why}"), width),
        });
    }
}

/// Returns the indexes of the sessions in `group`, in queue order.
fn list_group_members(sessions: &[SessionRow], group: SessionGroup) -> Vec<usize> {
    (0..sessions.len())
        .filter(|&index| SessionGroup::from_state(sessions[index].state) == group)
        .collect()
}

/// Returns the clean dialog's top line, such as `close 1  park 1`, and
/// one row per session in Safe to close with what y does to it, such
/// as `old-search          close`.
pub(super) fn format_clean_dialog_lines(sessions: &[SessionRow]) -> (String, Vec<String>) {
    let count = |state| {
        sessions
            .iter()
            .filter(|session| session.state == state)
            .count()
    };
    let title = format!(
        "close {}  park {}",
        count(Lifecycle::ToClose),
        count(Lifecycle::ToPark)
    );
    let rows = sessions
        .iter()
        .filter_map(|session| {
            let verb = match session.state {
                Lifecycle::ToClose => "close",
                Lifecycle::ToPark => "park",
                _ => return None,
            };
            Some(format!(
                "{:<NAME_WIDTH$} {verb}",
                fit(&session.name, NAME_WIDTH)
            ))
        })
        .collect();
    (title, rows)
}

/// Draws `lines` in `area` above the jump bar, which holds the bottom
/// row. The row `list` chooses is drawn reversed and kept on screen.
pub(super) fn draw_sessions(
    frame: &mut Frame,
    area: Rect,
    lines: &[PanelLine],
    chosen: Option<&str>,
    list: &mut ListState,
) {
    let [rows, bar] = split_bar(area);
    let bold = Style::new().add_modifier(Modifier::BOLD);
    let items: Vec<ListItem> = lines
        .iter()
        .map(|line| match line.session {
            None => ListItem::new(TextLine::styled(line.text.as_str(), bold)),
            Some(_) => ListItem::new(line.text.as_str()),
        })
        .collect();
    let list_widget =
        List::new(items).highlight_style(Style::new().add_modifier(Modifier::REVERSED));
    frame.render_stateful_widget(list_widget, rows, list);
    // The bar is bold once it can jump, and dim while it only hints.
    let style = if chosen.is_some() {
        bold
    } else {
        draw::pick_dim_style()
    };
    let text = format_bar_text(chosen, usize::from(bar.width));
    frame.render_widget(Paragraph::new(text).style(style), bar);
}

/// Returns the sessions panel's elements as `draw_sessions` drew them
/// in `area`, with `list` scrolled as the last draw left it. A row
/// and its why line tap `choose`, the fold row taps `unfold` or
/// `fold`, and the bar taps `go` once a session is chosen. Rows
/// scrolled off come too, hidden, and so do folded sessions.
pub(super) fn build_session_elements(
    lines: &[PanelLine],
    sessions: &[SessionRow],
    area: Rect,
    list: &ListState,
    chosen: Option<&str>,
    show_done: bool,
) -> Vec<Element> {
    let [rows, bar] = split_bar(area);
    let mut elements: Vec<Element> = lines
        .iter()
        .enumerate()
        .map(|(number, line)| {
            let id = match line.session {
                Some(index) if line.is_why => format!("why:{}", sessions[index].name),
                Some(index) => format!("session:{}", sessions[index].name),
                None if line.is_fold => FOLD_ID.to_owned(),
                None => format!("sessions:{number}"),
            };
            let element = match draw::find_row_or_scroll(number, list.offset(), rows) {
                Ok(row) => draw::build_text_element(&id, &line.text, Rect { y: row, ..rows }),
                Err(step) => draw::build_text_element(&id, &line.text, rows)
                    .hide(HiddenBy::Scroll, step.to_owned()),
            };
            match line.session {
                Some(index) => Element {
                    kind: Kind::Item,
                    tap: Some("choose"),
                    chosen: !line.is_why && chosen == Some(sessions[index].name.as_str()),
                    action: Action::Choose(index),
                    ..element
                },
                None if line.is_fold => Element {
                    kind: Kind::Heading,
                    tap: Some(if show_done { "fold" } else { "unfold" }),
                    action: Action::Toggle(number),
                    ..element
                },
                None => Element {
                    kind: Kind::Heading,
                    ..element
                },
            }
        })
        .collect();
    if !show_done {
        elements.extend(build_folded_elements(sessions));
    }
    let bar_element =
        draw::build_text_element("bar", &format_bar_text(chosen, usize::from(bar.width)), bar);
    elements.push(match chosen {
        Some(_) => Element {
            kind: Kind::Bar,
            tap: Some("go"),
            action: Action::Go,
            ..bar_element
        },
        None => Element {
            kind: Kind::Bar,
            ..bar_element
        },
    });
    elements
}

/// Returns the folded sessions as hidden elements, each shown by a tap
/// on the fold row.
fn build_folded_elements(sessions: &[SessionRow]) -> Vec<Element> {
    SessionGroup::DONE
        .into_iter()
        .flat_map(|group| list_group_members(sessions, group))
        .map(|index| {
            let session = &sessions[index];
            let text = format!("{} {}", describe_session(session), describe_why(session));
            let element = draw::build_text_element(
                &format!("session:{}", session.name),
                &text,
                Rect::default(),
            );
            Element {
                kind: Kind::Item,
                tap: Some("choose"),
                ..element
            }
            .hide(HiddenBy::Fold, format!("tap {FOLD_ID}"))
        })
        .collect()
}

/// Returns the panel's rows area and the jump bar's row below it.
fn split_bar(area: Rect) -> [Rect; 2] {
    Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).areas(area)
}

/// Returns the jump bar's words, such as `-> jump to docs-fix`, cut to
/// `width`.
fn format_bar_text(chosen: Option<&str>, width: usize) -> String {
    match chosen {
        Some(name) => fit(&format!("-> jump to {name}"), width),
        None => fit(CHOOSE_HINT, width),
    }
}

/// Returns a session's name, context bar and context in thousands,
/// such as `docs-fix            █░░  90`.
fn describe_session(session: &SessionRow) -> String {
    let thousands = session
        .context_tokens
        .map_or_else(|| "-".to_owned(), |tokens| (tokens / 1000).to_string());
    format!(
        "{:<NAME_WIDTH$} {} {thousands:>3}",
        fit(&session.name, NAME_WIDTH),
        format_context_bar(session.context_tokens),
    )
}

/// Returns why a session is in its group, such as `unpushed
/// in-review`. A kept session and one waiting on the hub also name
/// their card's stage.
fn describe_why(session: &SessionRow) -> String {
    let why = if session.why.is_empty() {
        session.state.word().to_owned()
    } else {
        session.why.clone()
    };
    let shows_stage = matches!(session.state, Lifecycle::Kept(_) | Lifecycle::WaitsOnHub);
    match (shows_stage, session.stage) {
        (true, Some(stage)) => format!("{why} {}", stage.as_str()),
        _ => why,
    }
}

/// Returns three blocks filled by thirds of `HANDOFF_TOKENS`, such as
/// `██░` at 102K, so the level reads without color.
fn format_context_bar(tokens: Option<u64>) -> String {
    let filled = tokens.map_or(0, |tokens| (tokens * 3 / HANDOFF_TOKENS).min(3));
    (0..3)
        .map(|block| if block < filled { '█' } else { '░' })
        .collect()
}

/// Returns `text` cut to `width` columns, ending in `~` when cut.
pub(super) fn fit(text: &str, width: usize) -> String {
    if text.chars().count() <= width {
        return text.to_owned();
    }
    let kept: String = text.chars().take(width.saturating_sub(1)).collect();
    format!("{kept}~")
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::board::view::tests::make_sample_queue;

    /// The sessions panel must sort each session into the group the
    /// clean pass sees, with its context and why, at 40 columns. A
    /// to-park session shown as parked would hide what c parks, and a
    /// kept reason cut at the screen's edge leaves the user guessing
    /// why it stays.
    #[test]
    fn session_lines_group_sessions_with_reasons() {
        let queue = make_sample_queue();

        let lines: Vec<String> = format_session_lines(&queue.sessions, 40, true)
            .into_iter()
            .map(|line| line.text)
            .collect();

        let expected = [
            "NEEDS YOU 1",
            "blocked-fix         ░░░  40 needs you",
            "AT WORK OR WAITING ON THE HUB 2",
            "api-build           ███ 160 working",
            "card-sync           ░░░  20",
            "  no reply yet active",
            "v KEPT 1 · SAFE TO CLOSE 2 · PARKED 1",
            "KEPT 1",
            "docs-fix            █░░  90",
            "  request in the last hour in-review",
            "SAFE TO CLOSE 2",
            "old-search          ░░░   - card done",
            "ship-docs           ░░░  30 to-park",
            "PARKED 1",
            "upload-retry        ░░░   - parked",
        ];
        assert_eq!(lines, expected);
    }

    /// When the row fits, the AT WORK OR WAITING ON THE HUB heading
    /// must count only its sessions that work right now. Counting the
    /// ones that wait on the hub would tell the user idle workers run.
    #[test]
    fn session_lines_count_working_when_room() {
        let queue = make_sample_queue();

        let wide = format_session_lines(&queue.sessions, 60, false);
        let phone = format_session_lines(&queue.sessions, 40, false);

        assert_eq!(wide[2].text, "AT WORK OR WAITING ON THE HUB 2 · 1 working");
        assert_eq!(phone[2].text, "AT WORK OR WAITING ON THE HUB 2");
    }

    /// Folded, the kept, safe to close and parked sessions must end
    /// in one row with each group's count. A row lost or miscounted
    /// there hides sessions the user would close with c.
    #[test]
    fn session_lines_end_in_fold_row() {
        let queue = make_sample_queue();

        let lines: Vec<String> = format_session_lines(&queue.sessions, 40, false)
            .into_iter()
            .map(|line| line.text)
            .collect();

        assert_eq!(
            lines.last().map(String::as_str),
            Some("> KEPT 1 · SAFE TO CLOSE 2 · PARKED 1")
        );
    }
}
