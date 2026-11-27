//! Names the board's five pages, writes their help, and draws the
//! fault and gauge lines on top of page 4. Read `Panel` and
//! `format_machine_lines` first.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line as TextLine;
use ratatui::widgets::Paragraph;

use super::draw;
use super::elements::{Element, HiddenBy};
use super::groups::Group;
use super::machine::{Gauges, Usage};
use super::sessions::fit;

/// The width from which the glance page puts its blocks side by side.
pub(super) const WIDE_COLUMNS: u16 = 100;

/// The blocks in a gauge's bar.
const GAUGE_BLOCKS: u64 = 10;

/// The glance page's help, which `?` opens there.
pub(super) const GLANCE_HELP: &[&str] = &[
    "HUB BOARD",
    "WAITING ON YOU: each card with a question for you, answer, or a worker that waits in its tab, see tab.",
    "SINCE YOU LEFT: each card that changed since your last visit, with its newest event.",
    "ACTIVE: the active cards, and how many have a worker at work now.",
    "IN REVIEW: the cards to review, and how many pull requests fail a check.",
    "A red ! row on top is a fault, such as a stopped hub watcher. Tap it to open page 4.",
    "j, k or the arrow keys choose a row, as on the pages.",
    "From 100 columns wide, h, l or the left and right arrows move to the nearest row in the other column.",
    "Enter, space or a tap acts on a row.",
    "A WAITING ON YOU, ACTIVE or IN REVIEW row opens its details.",
    "A heading, a ! row or a SINCE YOU LEFT row opens its page.",
    "A + N more row opens its page with all its rows.",
    "Press 1 to 5 to open a page. 4 opens the sessions and machine. Esc or q goes back.",
    "Magenta waits on you, cyan works now, green is done or passing, yellow is held up, red is a problem, and dim is quiet.",
];

/// The board's pages, in the order Tab and 1 to 5 open them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Panel {
    Waiting,
    Since,
    Active,
    Machine,
    Review,
}

impl Panel {
    pub(super) const ALL: [Panel; 5] = [
        Panel::Waiting,
        Panel::Since,
        Panel::Active,
        Panel::Machine,
        Panel::Review,
    ];

    /// Returns the page's key, 1 to 5.
    pub(super) fn number(self) -> usize {
        Panel::ALL
            .iter()
            .position(|&each| each == self)
            .unwrap_or(0)
            + 1
    }

    /// Returns the page Tab goes to, or `None`, the glance page, after
    /// the last.
    pub(super) fn find_next_panel(self) -> Option<Panel> {
        Panel::ALL.get(self.number()).copied()
    }

    /// Returns the page's name in element ids, such as `waiting`.
    pub(super) fn slug(self) -> &'static str {
        match self {
            Panel::Waiting => "waiting",
            Panel::Since => "since",
            Panel::Active => "active",
            Panel::Machine => "machine",
            Panel::Review => "review",
        }
    }

    /// Returns the page's name in its breadcrumb, such as
    /// `WAITING ON YOU`.
    pub(super) fn as_title(self) -> &'static str {
        match self {
            Panel::Waiting => "WAITING ON YOU",
            Panel::Since => "SINCE YOU LEFT",
            Panel::Active => "ACTIVE",
            Panel::Machine => "SESSIONS AND MACHINE",
            Panel::Review => "IN REVIEW AND PULL REQUESTS",
        }
    }

    /// Returns the groups the page lists, in screen order. Pages 2 and
    /// 4 draw lists of their own and list no group.
    pub(super) fn list_groups(self) -> &'static [Group] {
        match self {
            Panel::Waiting => &[Group::NeedsYou],
            Panel::Active => &[
                Group::Working,
                Group::NoWorker,
                Group::Idle,
                Group::Blocked,
                Group::Todo,
                Group::Draft,
            ],
            Panel::Review => &[Group::InReview, Group::OtherPrs],
            Panel::Since | Panel::Machine => &[],
        }
    }

    /// Returns the lines of the page's help, not yet wrapped.
    pub(super) fn help(self) -> &'static [&'static str] {
        match self {
            Panel::Waiting => &[
                "WAITING ON YOU",
                "One row per card. answer: the card has an open question, and the line below gives its age and the newest question. see tab: its worker waits in its tab, and the line below says why.",
                "Tap a row to read it.",
            ],
            Panel::Since => &[
                "SINCE YOU LEFT",
                "One row per card that changed since your last visit, newest first, with its newest event: report, new worker or finished. An event of another kind shows its own text.",
                "Tap a row to show or hide its events.",
            ],
            Panel::Active => &[
                "ACTIVE",
                "WORKING: a worker is at work now. NO WORKER: no session works on the card. IDLE: it has a worker, and none is at work.",
                "BLOCKED: someone outside the hub has to act. UP NEXT: planned. DRAFT: an idea.",
                "Each row starts with its status: run, you, idle or none.",
                "Tap a group to fold it, a row to read it.",
            ],
            Panel::Machine => &[
                "SESSIONS AND MACHINE",
                "A red ! row is a fault, such as a stopped hub watcher.",
                "WATCH: file-watch slots in use. New sessions crash near the limit, so it says high from 90%.",
                "DISK: free space on the home folder's disk.",
                "NEEDS YOU: the tab waits for you. AT WORK OR WAITING ON THE HUB: leave it.",
                "The last row folds KEPT, done and kept for a reason, SAFE TO CLOSE, which c closes or parks, and PARKED, closed and able to come back. Tap it to unfold.",
                "A full bar, 150K tokens: hand off.",
                "Tap a row to choose it, then tap the bar under the list to jump to it.",
            ],
            Panel::Review => &[
                "IN REVIEW AND PULL REQUESTS",
                "IN REVIEW: the cards to review, each with its pull request below when the card's prs: link names it.",
                "OTHER PULL REQUESTS: the other open pull requests, failing first. Passing ones of no card share one row.",
                "fails: a check failed. running: checks run. pass: checks pass.",
                "The pull requests come from the prs tool's saved list.",
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
    /// True on the `hub watcher ok` row, drawn in green.
    pub is_ok: bool,
    pub text: String,
    /// The line drawn in place of `text` when parts of it have their
    /// own colors, such as page 2's event words.
    pub colored: Option<TextLine<'static>>,
}

/// Returns a red `!` row for each fault the read found, such as
/// `! hub watcher stopped`, then the watch and disk gauges. With no
/// fault and a hub running, a green `hub watcher ok` row comes first.
/// With `no_hub` the read checked no watcher, so that row is left out.
pub(super) fn format_machine_lines(
    problems: &[String],
    gauges: &Gauges,
    no_hub: bool,
    width: usize,
) -> Vec<PanelText> {
    let make_row = |is_high, text: String| PanelText {
        is_heading: false,
        is_high,
        is_ok: false,
        text: fit(&text, width),
        colored: None,
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
    let mut lines: Vec<PanelText> = problems
        .iter()
        .map(|problem| make_row(true, format!("! {problem}")))
        .collect();
    if problems.is_empty() && !no_hub {
        let room = width.saturating_sub(2);
        lines.push(PanelText {
            is_ok: true,
            ..make_row(false, format!("{:room$}ok", "hub watcher"))
        });
    }
    lines.push(make_row(watch_high, format!("WATCH {watch}")));
    lines.push(make_row(false, format!("DISK  {disk}")));
    lines
}

/// Draws `lines` in `area` from line `scroll` on. Headings are bold so
/// they stand out without color. When `color` is true, a high gauge is
/// red and the ok row green; their `high` and `ok` words say the same
/// without color. A line's `colored` form, when it has one, is drawn
/// in place of its text.
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
            if let Some(colored) = &line.colored {
                let colored = colored.clone();
                if line.is_heading {
                    colored.patch_style(bold)
                } else {
                    colored
                }
            } else if line.is_heading {
                TextLine::styled(line.text.as_str(), bold)
            } else if line.is_high && color {
                TextLine::styled(line.text.as_str(), Style::new().fg(Color::Red))
            } else if line.is_ok && color {
                TextLine::styled(line.text.as_str(), Style::new().fg(Color::Green))
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

        let high = format_machine_lines(&[], &queue.gauges, false, 40);
        let calm = format_machine_lines(&[], &below, false, 40);

        assert_eq!(high[1].text, "WATCH █████████░ 116/128 high");
        assert!(high[1].is_high);
        assert_eq!(calm[1].text, "WATCH ████████░░ 115/128");
        assert!(!calm[1].is_high);
    }

    /// Page 4 must say the watcher runs, in green, only when a hub runs
    /// and the read found no fault. With no hub the read checked no
    /// watcher, and an ok row there would claim a watcher that may be
    /// stopped.
    #[test]
    fn machine_lines_show_watcher_ok_when_checked() {
        let queue = make_sample_queue();

        let lines = format_machine_lines(&[], &queue.gauges, false, 40);
        let hubless = format_machine_lines(&[], &queue.gauges, true, 40);

        assert_eq!(lines[0].text, "hub watcher                           ok");
        assert!(lines[0].is_ok);
        assert!(lines[1..].iter().all(|line| !line.is_ok));
        assert!(hubless.iter().all(|line| !line.is_ok));
    }

    /// Page 4 must show a fault in red above the gauges. A fault below
    /// the fold hides a stopped watcher.
    #[test]
    fn machine_lines_put_red_faults_above_gauges() {
        let queue = make_sample_queue();
        let problems = ["hub watcher stopped".to_owned()];

        let lines = format_machine_lines(&problems, &queue.gauges, false, 40);

        assert_eq!(lines[0].text, "! hub watcher stopped");
        assert!(lines[0].is_high);
        assert!(lines[1].text.starts_with("WATCH "));
    }

    /// Page 4 must show free disk in GiB, since `df -Pk` counts in
    /// kibibytes. A wrong unit misstates the free disk.
    #[test]
    fn machine_lines_show_free_disk_in_gib() {
        let queue = make_sample_queue();

        let lines = format_machine_lines(&[], &queue.gauges, false, 40);

        assert_eq!(lines[2].text, "DISK  ███████░░░ 73% 25G free");
    }
}
