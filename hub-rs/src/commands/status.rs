//! Runs `hub status`. Read `run_status` first. `build_status_rows` and
//! the two format functions are pure and hold every rule of the
//! output, so tests check them without Herdr.

use jiff::Zoned;
use jiff::tz::TimeZone;
use serde_json::{Value, json};

use super::Outcome;
use crate::external::cards::{Board, CardKey, Stage};
use crate::shared::context::{HANDOFF_TOKENS, format_tokens};
use crate::shared::events;
use crate::shared::facts::{self, HubLine};
use crate::shared::lifecycle::{self, Lifecycle, SessionFacts, Verdict};
use crate::shared::messages::MessageCounts;
use crate::shared::run_folder::RunFolder;
use crate::shared::{HubError, Tools};

/// Prints the hub line, the board line, the counts, the unread reports
/// and one row per session with its lifecycle state.
///
/// # Errors
///
/// `HubError::Herdr` when Herdr cannot list the agents, since no row
/// is left to print. A failed read of the run, parked or request
/// folder also stops it. A failed `card` or OpenCode database read
/// becomes a problem line instead.
pub fn run_status(
    tools: &Tools<'_>,
    run: &RunFolder,
    as_json: bool,
    now: &Zoned,
) -> Result<Outcome, HubError> {
    let memory = events::load_memory(run)?;
    let collected = facts::collect_session_facts(tools, run, &memory)?;
    let hub = facts::read_hub_line(tools);
    let unadopted = hub.name.as_deref().and_then(|own| {
        collected
            .sessions
            .iter()
            .filter(|session| session.agent.is_some())
            .find(|session| {
                lifecycle::is_unadopted_hub(own, &session.name, memory.predecessor.as_deref())
            })
            .map(|session| session.name.clone())
    });
    let verdicts = collected
        .sessions
        .into_iter()
        .map(|facts| {
            let verdict = lifecycle::decide_lifecycle(&facts);
            (facts, verdict)
        })
        .collect();
    let mut problems = collected.problems;
    for old in run.find_old_send_files()? {
        problems.push(format!("old send file: {}", old.display()));
    }
    if let Some(old) = unadopted {
        problems.push(format!("no hub handoff for {old} -> hub adopt {old}"));
    }
    let (counts, rows) = build_status_rows(verdicts, now.time_zone());
    let view = StatusView {
        hub,
        watcher_running: run.is_watcher_running()?,
        board: tools.cards.read_board().map_err(|error| error.to_string()),
        counts,
        unread: collected.unread,
        rows,
        no_session: collected.no_session,
        problems,
    };
    let lines = if as_json {
        vec![format_status_json(&view)]
    } else {
        format_status_text(&view)
    };
    Ok(Outcome { lines })
}

/// Everything `hub status` shows, in the order it shows it.
#[derive(Debug)]
struct StatusView {
    hub: HubLine,
    watcher_running: bool,
    /// The board counts and waiting cards, or why `card board` failed.
    board: Result<Board, String>,
    counts: StateCounts,
    unread: Vec<String>,
    rows: Vec<StatusRow>,
    no_session: Vec<(CardKey, Stage)>,
    /// One line each, such as `cards: card: no default vault`.
    problems: Vec<String>,
}

/// How many sessions are in each state, in the order the counts line
/// shows them. Every kept session counts once, whatever its reason.
#[derive(Debug, Default, PartialEq, Eq)]
struct StateCounts {
    working: usize,
    hub: usize,
    you: usize,
    kept: usize,
    to_close: usize,
    to_park: usize,
    parked: usize,
}

impl StateCounts {
    /// Counts one session in its state's field.
    fn add(&mut self, state: Lifecycle) {
        let field = match state {
            Lifecycle::Working => &mut self.working,
            Lifecycle::WaitsOnHub => &mut self.hub,
            Lifecycle::WaitsOnUser => &mut self.you,
            Lifecycle::Kept(_) => &mut self.kept,
            Lifecycle::ToClose => &mut self.to_close,
            Lifecycle::ToPark => &mut self.to_park,
            Lifecycle::Parked => &mut self.parked,
        };
        *field += 1;
    }
}

/// One session's row.
#[derive(Debug)]
struct StatusRow {
    name: String,
    state: Lifecycle,
    context_tokens: Option<u64>,
    /// The WHY cell, such as `1 held, hand off`.
    why: String,
    card: Option<(CardKey, Stage)>,
    messages: MessageCounts,
}

/// Counts each state and returns the rows ordered by state, then name.
/// Times print in `time_zone`.
fn build_status_rows(
    verdicts: Vec<(SessionFacts, Verdict)>,
    time_zone: &TimeZone,
) -> (StateCounts, Vec<StatusRow>) {
    let mut counts = StateCounts::default();
    let mut rows: Vec<StatusRow> = verdicts
        .into_iter()
        .map(|(facts, verdict)| {
            counts.add(verdict.state);
            StatusRow {
                why: describe_why(&facts, &verdict, time_zone),
                name: facts.name,
                state: verdict.state,
                context_tokens: facts.context_tokens,
                card: facts.card.map(|card| (card.key, card.stage)),
                messages: facts.messages,
            }
        })
        .collect();
    rows.sort_by(|left, right| {
        (left.state.rank(), &left.name).cmp(&(right.state.rank(), &right.name))
    });
    (counts, rows)
}

/// Writes the view as header lines and a table, columns two spaces
/// apart with no trailing spaces.
fn format_status_text(view: &StatusView) -> Vec<String> {
    let hub_name = view.hub.name.as_deref().unwrap_or("-");
    let tokens = format_tokens(view.hub.context_tokens);
    let hand_off = if view.hub.context_tokens >= Some(HANDOFF_TOKENS) {
        ": hand off now"
    } else {
        ""
    };
    let watcher = if view.watcher_running {
        "watcher running"
    } else {
        "watcher stopped"
    };
    let board = match &view.board {
        Ok(board) => format!("board {}", board.counts),
        Err(reason) => format!("board: {reason}"),
    };
    let waiting = view.board.as_ref().ok().and_then(Board::waiting_line);
    let counts = &view.counts;
    let open = view.rows.len() - counts.parked;
    let unread = if view.unread.is_empty() {
        "unread 0".to_owned()
    } else {
        format!("unread {}: {}", view.unread.len(), view.unread.join(" "))
    };
    let mut lines = vec![
        format!("hub {hub_name} {tokens}{hand_off}, {watcher}"),
        board,
    ];
    lines.extend(waiting);
    lines.extend([
        format!(
            "open {open}: {} working, {} hub, {} you,",
            counts.working, counts.hub, counts.you
        ),
        format!(
            "  {} kept, {} to-close, {} to-park; {} parked",
            counts.kept, counts.to_close, counts.to_park, counts.parked
        ),
        unread,
        String::new(),
    ]);
    let mut table = vec![["NAME", "STATE", "CTX", "WHY"].map(str::to_owned)];
    table.extend(view.rows.iter().map(|row| {
        [
            row.name.clone(),
            row.state.word().to_owned(),
            format_tokens(row.context_tokens),
            row.why.clone(),
        ]
    }));
    lines.extend(format_columns(&table));
    if !view.no_session.is_empty() {
        let cards: Vec<String> = view
            .no_session
            .iter()
            .map(|(key, stage)| format!("{} {}", key.as_str(), stage.as_str()))
            .collect();
        lines.push(String::new());
        lines.push(format!("no session: {}", cards.join(", ")));
    }
    if !view.problems.is_empty() {
        lines.push(String::new());
        lines.extend(view.problems.iter().cloned());
    }
    lines
}

/// Writes the view as one JSON object holding the same facts.
fn format_status_json(view: &StatusView) -> String {
    let board = match &view.board {
        Ok(board) => {
            let waiting: Vec<Value> = board
                .waiting
                .iter()
                .map(|card| {
                    json!({
                        "card": card.key.as_str(),
                        "days": card.days,
                        "title": card.title,
                        "reason": card.reason,
                    })
                })
                .collect();
            let counts = &board.counts;
            json!({
                "active": counts.active,
                "review": counts.review,
                "active_limit": counts.active_limit,
                "review_limit": counts.review_limit,
                "waiting_on_others": waiting,
            })
        }
        Err(reason) => json!({"error": reason}),
    };
    let counts = &view.counts;
    let sessions: Vec<Value> = view
        .rows
        .iter()
        .map(|row| {
            let reason = match row.state {
                Lifecycle::Kept(reason) => Some(reason.label()),
                _ => None,
            };
            json!({
                "name": row.name,
                "state": row.state.word(),
                "reason": reason,
                "why": row.why,
                "context_tokens": row.context_tokens,
                "card": row.card.as_ref().map(|(key, _)| key.as_str()),
                "stage": row.card.as_ref().map(|(_, stage)| stage.as_str()),
                "held": row.messages.held,
                "unsure": row.messages.unsure,
                "not_sent": row.messages.not_sent,
            })
        })
        .collect();
    let no_session: Vec<Value> = view
        .no_session
        .iter()
        .map(|(key, stage)| json!({"card": key.as_str(), "stage": stage.as_str()}))
        .collect();
    json!({
        "hub": {"name": view.hub.name, "context_tokens": view.hub.context_tokens},
        "watcher": if view.watcher_running { "running" } else { "stopped" },
        "board": board,
        "counts": {
            "working": counts.working,
            "hub": counts.hub,
            "you": counts.you,
            "kept": counts.kept,
            "to-close": counts.to_close,
            "to-park": counts.to_park,
            "parked": counts.parked,
        },
        "unread": view.unread,
        "sessions": sessions,
        "no_session": no_session,
        "problems": view.problems,
    })
    .to_string()
}

/// Returns the short WHY cell, such as `unread report` or `1 held`,
/// with `hand off` added at `HANDOFF_TOKENS` or more, or `-` when
/// nothing needs saying.
pub(crate) fn describe_why(
    facts: &SessionFacts,
    verdict: &Verdict,
    time_zone: &TimeZone,
) -> String {
    let mut parts = Vec::new();
    if !verdict.why.is_empty() {
        parts.push(verdict.why.clone());
    }
    if let (Lifecycle::Parked, Some(record)) = (verdict.state, &facts.parked) {
        let parked_at = record.parked_at.to_zoned(time_zone.clone());
        parts.push(format!("since {}", parked_at.strftime("%m-%d %H:%M")));
    }
    if facts.messages.held > 0 {
        parts.push(format!("{} held", facts.messages.held));
    }
    if facts.messages.unsure > 0 {
        parts.push("unsure".to_owned());
    }
    if facts.messages.not_sent > 0 {
        parts.push("not sent".to_owned());
    }
    if facts.context_tokens >= Some(HANDOFF_TOKENS) {
        parts.push("hand off".to_owned());
    }
    if parts.is_empty() {
        "-".to_owned()
    } else {
        parts.join(", ")
    }
}

/// Pads each cell to its column's widest, two spaces apart, and trims
/// the end of each line.
fn format_columns(table: &[[String; 4]]) -> Vec<String> {
    let mut widths = [0; 4];
    for row in table {
        for (width, cell) in widths.iter_mut().zip(row) {
            *width = (*width).max(cell.chars().count());
        }
    }
    table
        .iter()
        .map(|row| {
            let cells: Vec<String> = row
                .iter()
                .zip(widths)
                .map(|(cell, width)| format!("{cell:width$}"))
                .collect();
            cells.join("  ").trim_end().to_owned()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::external::cards::{BoardCounts, WaitingCard};
    use crate::external::git::Unsaved;
    use crate::shared::lifecycle::KeptReason;
    use crate::shared::test_values::{make_facts, make_parked_record};

    /// A session's facts with its context and a card in `stage`, or no
    /// card.
    fn make_row_facts(name: &str, tokens: Option<u64>, stage: Option<Stage>) -> SessionFacts {
        let mut facts = make_facts(name);
        facts.context_tokens = tokens;
        match stage {
            Some(stage) => facts.card.as_mut().unwrap().stage = stage,
            None => facts.card = None,
        }
        facts
    }

    /// A verdict with this state and why.
    fn make_verdict(state: Lifecycle, why: &str) -> Verdict {
        Verdict {
            state,
            why: why.to_owned(),
        }
    }

    /// A status view with one session in each state, given out of
    /// order.
    fn make_example_view() -> StatusView {
        let mut docs_fix = make_row_facts("docs-fix", Some(152_000), None);
        docs_fix.messages.held = 1;
        let mut parked = make_row_facts("demo-kb-081-auto", None, Some(Stage::Review));
        parked.agent = None;
        parked.parked = Some(make_parked_record("demo-kb-081-auto"));
        parked.unsaved = Unsaved::NotChecked;
        let verdicts = vec![
            (parked, make_verdict(Lifecycle::Parked, "")),
            (
                make_row_facts("demo-kb-090-auto", Some(104_000), Some(Stage::Review)),
                make_verdict(Lifecycle::ToPark, ""),
            ),
            (
                make_row_facts("review-kanban-hub-16", Some(60_000), None),
                make_verdict(Lifecycle::ToClose, "report read"),
            ),
            (
                make_row_facts("demo-kb-012-auto", Some(77_310), Some(Stage::Review)),
                make_verdict(
                    Lifecycle::Kept(KeptReason::UnsavedFiles),
                    "unsaved files in /home/u/wt/demo-kb-012",
                ),
            ),
            (
                make_row_facts("demo-kb-009-auto", Some(12_000), Some(Stage::Active)),
                make_verdict(Lifecycle::WaitsOnUser, "blocked"),
            ),
            (
                make_row_facts("demo-kb-007-impl", Some(98_000), Some(Stage::Active)),
                make_verdict(Lifecycle::WaitsOnHub, "unread report"),
            ),
            (docs_fix, make_verdict(Lifecycle::Working, "")),
        ];
        let (counts, rows) = build_status_rows(verdicts, &TimeZone::UTC);
        StatusView {
            hub: HubLine {
                name: Some("kanban-hub-18".to_owned()),
                session_id: Some("ses_hub".to_owned()),
                context_tokens: Some(131_204),
            },
            watcher_running: true,
            board: Ok(Board {
                counts: BoardCounts {
                    active: 5,
                    review: 19,
                    active_limit: 8,
                    review_limit: 6,
                },
                waiting: vec![WaitingCard {
                    key: "DEMO-KB-003".parse().unwrap(),
                    days: 8,
                    title: "Retry failed uploads".to_owned(),
                    reason: "platform team's review".to_owned(),
                }],
            }),
            counts,
            unread: vec!["demo-kb-007-impl".to_owned(), "docs-fix".to_owned()],
            rows,
            no_session: vec![("DEMO-KB-004".parse().unwrap(), Stage::Active)],
            problems: Vec::new(),
        }
    }

    /// The hub agent reads these lines to decide what to do next, so a
    /// wrong word, count, order or column misleads every hub. The rows
    /// come in working-first order with the parked time in local time,
    /// and the WHY cell adds held messages and the hand-off point.
    #[test]
    fn format_status_text_matches_example_lines() {
        let view = make_example_view();

        let lines = format_status_text(&view);

        assert_eq!(
            lines,
            [
                "hub kanban-hub-18 131k, watcher running",
                "board active 5 of 8, review 19 of 6",
                "waiting-on-others 1, oldest 8 days",
                "open 6: 1 working, 1 hub, 1 you,",
                "  1 kept, 1 to-close, 1 to-park; 1 parked",
                "unread 2: demo-kb-007-impl docs-fix",
                "",
                "NAME                  STATE     CTX   WHY",
                "docs-fix              working   152k  1 held, hand off",
                "demo-kb-007-impl      hub       98k   unread report",
                "demo-kb-009-auto      you       12k   blocked",
                "demo-kb-012-auto      kept      77k   unsaved files in /home/u/wt/demo-kb-012",
                "review-kanban-hub-16  to-close  60k   report read",
                "demo-kb-090-auto      to-park   104k  -",
                "demo-kb-081-auto      parked    -     since 11-10 22:41",
                "",
                "no session: DEMO-KB-004 active",
            ]
        );
    }

    /// Scripts and the later board view read `--json`, so a renamed or
    /// missing field breaks them. It must hold the same facts as the
    /// text, including the kept reason apart from the WHY cell.
    #[test]
    fn format_status_json_matches_example_object() {
        let view = make_example_view();

        let printed: Value = serde_json::from_str(&format_status_json(&view)).unwrap();

        let session = |name: &str,
                       state: &str,
                       why: &str,
                       tokens: Option<u64>,
                       card: Option<(&str, &str)>| {
            json!({
                "name": name,
                "state": state,
                "reason": null,
                "why": why,
                "context_tokens": tokens,
                "card": card.map(|(key, _)| key),
                "stage": card.map(|(_, stage)| stage),
                "held": 0,
                "unsure": 0,
                "not_sent": 0,
            })
        };
        let mut docs_fix = session(
            "docs-fix",
            "working",
            "1 held, hand off",
            Some(152_000),
            None,
        );
        docs_fix["held"] = json!(1);
        let mut home = session(
            "demo-kb-012-auto",
            "kept",
            "unsaved files in /home/u/wt/demo-kb-012",
            Some(77_310),
            Some(("DEMO-KB-012", "review")),
        );
        home["reason"] = json!("unsaved files");
        assert_eq!(
            printed,
            json!({
                "hub": {"name": "kanban-hub-18", "context_tokens": 131_204},
                "watcher": "running",
                "board": {
                    "active": 5,
                    "review": 19,
                    "active_limit": 8,
                    "review_limit": 6,
                    "waiting_on_others": [{
                        "card": "DEMO-KB-003",
                        "days": 8,
                        "title": "Retry failed uploads",
                        "reason": "platform team's review",
                    }],
                },
                "counts": {
                    "working": 1,
                    "hub": 1,
                    "you": 1,
                    "kept": 1,
                    "to-close": 1,
                    "to-park": 1,
                    "parked": 1,
                },
                "unread": ["demo-kb-007-impl", "docs-fix"],
                "sessions": [
                    docs_fix,
                    session(
                        "demo-kb-007-impl",
                        "hub",
                        "unread report",
                        Some(98_000),
                        Some(("DEMO-KB-007", "active")),
                    ),
                    session(
                        "demo-kb-009-auto",
                        "you",
                        "blocked",
                        Some(12_000),
                        Some(("DEMO-KB-009", "active")),
                    ),
                    home,
                    session("review-kanban-hub-16", "to-close", "report read", Some(60_000), None),
                    session(
                        "demo-kb-090-auto",
                        "to-park",
                        "-",
                        Some(104_000),
                        Some(("DEMO-KB-090", "review")),
                    ),
                    session(
                        "demo-kb-081-auto",
                        "parked",
                        "since 11-10 22:41",
                        None,
                        Some(("DEMO-KB-081", "review")),
                    ),
                ],
                "no_session": [{"card": "DEMO-KB-004", "stage": "active"}],
                "problems": [],
            })
        );
    }
}
