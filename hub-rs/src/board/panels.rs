//! Draws the kanban panel, which lists the board's cards by stage, and
//! the machine panel, with the gauges and the report feed. Read
//! `kanban_lines` and `machine_lines` first. Below 100 columns the
//! board shows one panel at a time; from 100 on `split_wide` places
//! all four.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line as TextLine;
use ratatui::widgets::Paragraph;

use super::draw;
use super::elements::{Element, HiddenBy};
use super::machine::{FeedLine, Gauges, Usage};
use super::queue::CardLine;
use super::sessions::fit;
use crate::external::cards::Stage;

/// The stages the kanban panel shows, the user's next action first.
const KANBAN_STAGES: [Stage; 4] = [Stage::Review, Stage::Active, Stage::Ready, Stage::Draft];

/// The width from which the board shows all four panels at once.
pub(super) const WIDE_COLUMNS: u16 = 100;

/// The blocks in a gauge's bar.
const GAUGE_BLOCKS: u64 = 10;

/// The board's panels, in the order Tab and 1 to 4 pick them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Panel {
    Queue,
    Kanban,
    Sessions,
    Machine,
}

impl Panel {
    pub(super) const ALL: [Panel; 4] =
        [Panel::Queue, Panel::Kanban, Panel::Sessions, Panel::Machine];

    /// Returns the panel's key, 1 to 4.
    pub(super) fn number(self) -> usize {
        Panel::ALL
            .iter()
            .position(|&each| each == self)
            .unwrap_or(0)
            + 1
    }

    /// Returns the panel Tab goes to, back to the queue after the last.
    pub(super) fn next(self) -> Panel {
        Panel::ALL[self.number() % Panel::ALL.len()]
    }

    /// Returns the panel's title row, such as `2 KANBAN`, with `>` in
    /// front when keys act on it.
    pub(super) fn title(self, is_current: bool) -> String {
        let word = match self {
            Panel::Queue => "QUEUE",
            Panel::Kanban => "KANBAN",
            Panel::Sessions => "SESSIONS",
            Panel::Machine => "MACHINE",
        };
        let mark = if is_current { ">" } else { " " };
        format!("{mark}{} {word}", self.number())
    }

    /// Returns the word in this panel's element ids, such as `kanban`.
    pub(super) fn slug(self) -> &'static str {
        match self {
            Panel::Queue => "queue",
            Panel::Kanban => "kanban",
            Panel::Sessions => "sessions",
            Panel::Machine => "machine",
        }
    }
}

/// Returns each panel's area on a wide screen, in `Panel::ALL` order.
/// The queue takes the left 40%; sessions take the top of the right,
/// with kanban and the machine side by side below.
pub(super) fn split_wide(area: Rect) -> [Rect; 4] {
    let [queue, right] =
        Layout::horizontal([Constraint::Percentage(40), Constraint::Fill(1)]).areas(area);
    let [sessions, bottom] =
        Layout::vertical([Constraint::Percentage(50), Constraint::Fill(1)]).areas(right);
    let [kanban, machine] =
        Layout::horizontal([Constraint::Percentage(50), Constraint::Fill(1)]).areas(bottom);
    [queue, kanban, sessions, machine]
}

/// Returns `lines` as elements drawn in `area` from line `scroll` on,
/// with ids such as `kanban:0`. Lines scrolled off come too, hidden.
pub(super) fn text_elements(
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
            match draw::row_or_scroll(number, scroll, area) {
                Ok(row) => draw::text_element(&id, &line.text, Rect { y: row, ..area }),
                Err(step) => draw::text_element(&id, &line.text, area)
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

/// Returns each stage that holds a card as a heading with its count,
/// such as `REVIEW 1`, followed by one row per card, its key and
/// title cut to `width`.
pub(super) fn kanban_lines(cards: &[CardLine], width: usize) -> Vec<PanelText> {
    let mut lines = Vec::new();
    for stage in KANBAN_STAGES {
        let members: Vec<&CardLine> = cards.iter().filter(|card| card.stage == stage).collect();
        if members.is_empty() {
            continue;
        }
        lines.push(PanelText {
            is_heading: true,
            is_high: false,
            text: format!("{} {}", stage.as_str().to_uppercase(), members.len()),
        });
        lines.extend(members.into_iter().map(|card| PanelText {
            is_heading: false,
            is_high: false,
            text: fit(&format!("{} {}", card.key.as_str(), card.title), width),
        }));
    }
    lines
}

/// Returns the watch and disk gauges, then a `REPORTS` heading and
/// one line per report, such as `10:40 new api-build`.
pub(super) fn machine_lines(gauges: &Gauges, feed: &[FeedLine], width: usize) -> Vec<PanelText> {
    let row = |is_heading, is_high, text: String| PanelText {
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
            format!("{} {}/{}{high}", gauge_bar(usage), usage.used, usage.limit)
        },
    );
    let disk = gauges.disk.map_or_else(
        || "unknown".to_owned(),
        |usage| {
            let percent = usage.used * 100 / usage.limit.max(1);
            // `df -Pk` counts in kibibytes, so this is gibibytes.
            let free = usage.limit.saturating_sub(usage.used) / (1024 * 1024);
            format!("{} {percent}% {free}G free", gauge_bar(usage))
        },
    );
    let mut lines = vec![
        row(false, watch_high, format!("WATCH {watch}")),
        row(false, false, format!("DISK  {disk}")),
        row(true, false, format!("REPORTS {}", feed.len())),
    ];
    lines.extend(feed.iter().map(|report| {
        let word = if report.unread { "new" } else { "read" };
        row(
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
fn gauge_bar(usage: Usage) -> String {
    let filled = (usage.used * GAUGE_BLOCKS / usage.limit.max(1)).min(GAUGE_BLOCKS);
    (0..GAUGE_BLOCKS)
        .map(|block| if block < filled { '█' } else { '░' })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::board::view::tests::sample_queue;

    /// The kanban panel is where the user sees which cards are in
    /// review and in work. A card under the wrong stage, or review
    /// below draft, hides the next thing to act on.
    #[test]
    fn kanban_lines_list_cards_by_stage() {
        let queue = sample_queue();

        let lines: Vec<String> = kanban_lines(&queue.cards, 40)
            .into_iter()
            .map(|line| line.text)
            .collect();

        let expected = [
            "REVIEW 1",
            "DEMO-KB-010 Retry a failed upload twice~",
            "ACTIVE 5",
            "DEMO-KB-001 Faster search",
            "DEMO-KB-002 Board for the phone",
            "DEMO-KB-003 Trim the build cache",
            "DEMO-KB-004 Lint every Dockerfile in th~",
            "DEMO-KB-005 Rotate the backup keys",
            "READY 1",
            "DEMO-KB-020 Sort ready cards",
            "DRAFT 1",
            "OPS-KB-001 Watch the disks",
        ];
        assert_eq!(lines, expected);
    }

    /// New OpenCode sessions crashed at 122 of 128 watch slots, so the
    /// gauge must warn from 90%, 116 of 128, and not at 115. A warning
    /// shown only in red is lost on a screen without color, so it also
    /// says `high`.
    #[test]
    fn machine_lines_mark_watch_high_at_90() {
        let queue = sample_queue();
        let below = Gauges {
            watch: Some(Usage {
                used: 115,
                limit: 128,
            }),
            ..queue.gauges
        };

        let high = machine_lines(&queue.gauges, &[], 40);
        let calm = machine_lines(&below, &[], 40);

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
        let queue = sample_queue();

        let lines = machine_lines(&queue.gauges, &queue.feed, 40);

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
