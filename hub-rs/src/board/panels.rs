//! Names the board's views, writes their help pages, and draws the
//! text of the away, next and machine views. Read `Panel` and
//! `format_machine_lines` first.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line as TextLine;
use ratatui::widgets::Paragraph;

use super::away::{self, AwayEvent};
use super::draw;
use super::elements::{Element, HiddenBy};
use super::groups::{self, Group, Item};
use super::machine::{FEED_LENGTH, FeedLine, Gauges, Usage};
use super::sessions::fit;

/// The width from which the glance page puts its blocks side by side.
pub(super) const WIDE_COLUMNS: u16 = 100;

/// The blocks in a gauge's bar.
const GAUGE_BLOCKS: u64 = 10;

/// The glance page's help, which `?` opens there.
pub(super) const GLANCE_HELP: &[&str] = &[
    "HUB BOARD",
    "AWAY: what happened since you last used the board, newest first.",
    "QUEUE: what waits on you and what runs now.",
    "NEXT: your next step, then the hub's.",
    "Tap a block, or press 1 to 3, to open it. Esc or q goes back. 4 opens the sessions and 5 the machine.",
    "Each queue row starts with its status. you: it waits on you. run: a worker is at work on it. idle: it has a worker, and none is at work. rev: a pull request to read. none: no worker.",
];

/// The board's views, in the order Tab and 1 to 5 open them. The first
/// three are also the glance page's blocks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Panel {
    Away,
    Queue,
    Next,
    Sessions,
    Machine,
}

impl Panel {
    pub(super) const ALL: [Panel; 5] = [
        Panel::Away,
        Panel::Queue,
        Panel::Next,
        Panel::Sessions,
        Panel::Machine,
    ];

    /// Returns the view's key, 1 to 5.
    pub(super) fn number(self) -> usize {
        Panel::ALL
            .iter()
            .position(|&each| each == self)
            .unwrap_or(0)
            + 1
    }

    /// Returns the view Tab goes to, or `None`, the glance page, after
    /// the last.
    pub(super) fn find_next_panel(self) -> Option<Panel> {
        Panel::ALL.get(self.number()).copied()
    }

    /// Returns the view's name in its breadcrumb and element ids, such
    /// as `queue`.
    pub(super) fn slug(self) -> &'static str {
        match self {
            Panel::Away => "away",
            Panel::Queue => "queue",
            Panel::Next => "next",
            Panel::Sessions => "sessions",
            Panel::Machine => "machine",
        }
    }

    /// Returns the lines of the view's help page, not yet wrapped.
    pub(super) fn help(self) -> &'static [&'static str] {
        match self {
            Panel::Away => &[
                "AWAY",
                "Worker reports and the hub's notes.md lines, newest first.",
            ],
            Panel::Queue => &[
                "QUEUE",
                "Your cards and the hub's questions, by their next step. The step that needs you comes first.",
                "Needs you: someone waits on you.",
                "Active: a worker builds it.",
                "In review: a pull request to read.",
                "Blocked: someone outside the hub has to act.",
                "Todo: planned. Draft: an idea.",
                "Each row starts with its status: you, run, idle, rev or none. ? on the glance page says what each means.",
                "Tap a group to fold it, a row to read it, a count to jump to it.",
            ],
            Panel::Next => &[
                "NEXT",
                "you: each question or session that waits on you.",
                "then: how many cards wait for your review.",
                "hub: the Todo cards in card list order. The hub may start another one first.",
            ],
            Panel::Sessions => &[
                "SESSIONS",
                "Every agent tab of this hub, grouped by what hub clean would do with it.",
                "NEEDS YOU: the tab waits for you.",
                "WORKING: it works or waits on the hub. Leave it.",
                "KEPT: done, kept for a reason.",
                "SAFE TO CLOSE: c closes or parks it.",
                "PARKED: closed, and it can come back.",
                "A full bar, 150K tokens: hand off.",
                "Tap a row to choose it, then tap the bar under the list to jump to it.",
            ],
            Panel::Machine => &[
                "MACHINE",
                "WATCH: file-watch slots in use. New sessions crash near the limit, so it says high from 90%.",
                "DISK: free space on the home folder's disk.",
                "REPORTS: the six newest worker reports, new or read.",
            ],
        }
    }
}

/// Returns `lines` as elements drawn in `area` from line `scroll` on,
/// with ids such as `machine:0`. Lines scrolled off come too, hidden.
pub(super) fn build_text_elements(
    panel: Panel,
    lines: &[PanelText],
    area: Rect,
    scroll: usize,
) -> Vec<Element> {
    lines
        .iter()
        .enumerate()
        .map(|(number, line)| {
            let id = format!("{}:{number}", panel.slug());
            match draw::find_row_or_scroll(number, scroll, area) {
                Ok(row) => draw::build_text_element(&id, &line.text, Rect { y: row, ..area }),
                Err(step) => draw::build_text_element(&id, &line.text, area)
                    .hide(HiddenBy::Scroll, step.to_owned()),
            }
        })
        .collect()
}

/// One line of a panel, a heading or a row.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct PanelText {
    pub is_heading: bool,
    /// True on the watch gauge at or past 90% of its limit, drawn in red.
    pub is_high: bool,
    pub text: String,
}

/// Returns one line per event for the AWAY block, the list of what
/// happened while the user was away, cut to `width`. A line about a
/// card starts with its card key, such as
/// ` 10:40 DEMO-KB-001 demo-kb-001-build reported`.
pub(super) fn format_away_lines(events: &[AwayEvent], width: usize) -> Vec<PanelText> {
    events
        .iter()
        .map(|event| {
            let card = away::find_event_card(event);
            let words = groups::put_card_key_first(card.as_ref(), &event.text);
            PanelText {
                is_heading: false,
                is_high: false,
                text: fit(&format!(" {} {words}", event.time), width),
            }
        })
        .collect()
}

/// Returns the lines of the NEXT block, which says what the user and
/// the hub do next, cut to `width`. An example is
/// ` you: answer DEMO-KB-001 Deploy tonight?`. The `you` and `hub`
/// parts give at most `each` lines, so the glance page, the board's
/// first screen, shows one line of each with `each` 1.
pub(super) fn format_next_lines(
    groups: &[(Group, Vec<Item>)],
    no_hub: bool,
    each: usize,
    width: usize,
) -> Vec<PanelText> {
    let next = groups::write_next_lines(groups, no_hub);
    next.you
        .iter()
        .take(each)
        .chain([&next.then])
        .chain(next.hub.iter().take(each))
        .map(|line| PanelText {
            is_heading: false,
            is_high: false,
            text: fit(&format!(" {line}"), width),
        })
        .collect()
}

/// Returns the watch and disk gauges, then a `REPORTS` heading and
/// one line for each of the `FEED_LENGTH` newest reports, such as
/// `10:40 new api-build`.
pub(super) fn format_machine_lines(
    gauges: &Gauges,
    feed: &[FeedLine],
    width: usize,
) -> Vec<PanelText> {
    let make_row = |is_heading, is_high, text: String| PanelText {
        is_heading,
        is_high,
        text: fit(&text, width),
    };
    let watch_high = gauges
        .watch
        .is_some_and(|usage| usage.used * 10 >= usage.limit * 9);
    let watch = gauges.watch.map_or_else(
        || "unknown".to_owned(),
        |usage| {
            let high = if watch_high { " high" } else { "" };
            format!(
                "{} {}/{}{high}",
                format_gauge_bar(usage),
                usage.used,
                usage.limit
            )
        },
    );
    let disk = gauges.disk.map_or_else(
        || "unknown".to_owned(),
        |usage| {
            let percent = usage.used * 100 / usage.limit.max(1);
            // `df -Pk` counts in kibibytes, so this is gibibytes.
            let free = usage.limit.saturating_sub(usage.used) / (1024 * 1024);
            format!("{} {percent}% {free}G free", format_gauge_bar(usage))
        },
    );
    let shown = &feed[..feed.len().min(FEED_LENGTH)];
    let mut lines = vec![
        make_row(false, watch_high, format!("WATCH {watch}")),
        make_row(false, false, format!("DISK  {disk}")),
        make_row(true, false, format!("REPORTS {}", shown.len())),
    ];
    lines.extend(shown.iter().map(|report| {
        let word = if report.unread { "new" } else { "read" };
        make_row(
            false,
            false,
            format!("{} {word} {}", report.at, report.name),
        )
    }));
    lines
}

/// Draws `lines` in `area` from line `scroll` on. Headings are bold so
/// they stand out without color, and a high gauge is red when `color`
/// is true; its `high` word says the same without color.
pub(super) fn draw_panel_text(
    frame: &mut Frame,
    area: Rect,
    lines: &[PanelText],
    scroll: usize,
    color: bool,
) {
    let bold = Style::new().add_modifier(Modifier::BOLD);
    let shown: Vec<TextLine> = lines
        .iter()
        .skip(scroll)
        .map(|line| {
            if line.is_heading {
                TextLine::styled(line.text.as_str(), bold)
            } else if line.is_high && color {
                TextLine::styled(line.text.as_str(), Style::new().fg(Color::Red))
            } else {
                TextLine::raw(line.text.as_str())
            }
        })
        .collect();
    frame.render_widget(Paragraph::new(shown), area);
}

/// Returns `GAUGE_BLOCKS` blocks filled by the share used, such as
/// `█████████░` at 116 of 128.
fn format_gauge_bar(usage: Usage) -> String {
    let filled = (usage.used * GAUGE_BLOCKS / usage.limit.max(1)).min(GAUGE_BLOCKS);
    (0..GAUGE_BLOCKS)
        .map(|block| if block < filled { '█' } else { '░' })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::board::view::tests::make_sample_queue;

    /// New OpenCode sessions crashed at 122 of 128 watch slots, so the
    /// gauge must warn from 90%, 116 of 128, and not at 115. A warning
    /// shown only in red is lost on a screen without color, so it also
    /// says `high`.
    #[test]
    fn machine_lines_mark_watch_high_at_90() {
        let queue = make_sample_queue();
        let below = Gauges {
            watch: Some(Usage {
                used: 115,
                limit: 128,
            }),
            ..queue.gauges
        };

        let high = format_machine_lines(&queue.gauges, &[], 40);
        let calm = format_machine_lines(&below, &[], 40);

        assert_eq!(high[0].text, "WATCH █████████░ 116/128 high");
        assert!(high[0].is_high);
        assert_eq!(calm[0].text, "WATCH ████████░░ 115/128");
        assert!(!calm[0].is_high);
    }

    /// The machine panel must show free disk in GiB and the newest
    /// reports, newest first. A wrong unit misstates the free disk, and
    /// a missing feed hides what workers just reported.
    #[test]
    fn machine_lines_show_disk_and_reports() {
        let queue = make_sample_queue();

        let lines = format_machine_lines(&queue.gauges, &queue.feed, 40);

        let texts: Vec<&str> = lines.iter().map(|line| line.text.as_str()).collect();
        let expected = [
            "WATCH █████████░ 116/128 high",
            "DISK  ███████░░░ 73% 25G free",
            "REPORTS 2",
            "10:40 new api-build",
            "10:12 read docs-fix",
        ];
        assert_eq!(texts, expected);
    }
}
