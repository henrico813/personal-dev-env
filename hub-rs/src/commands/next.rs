//! Runs `hub next`, which waits until something needs the hub and
//! prints one line. Read `run_next`, then `check_for_news`.
//! `choose_news` is pure and holds the order of the news lines.
//! `run_minute_pass` is the slow part, at most once a minute, which
//! reads the board, finds finished sessions and asks workers to hand
//! off. It never closes or parks a session, which only `hub clean`
//! does. The one session it starts is an old hub's session review,
//! once that hub's tab has closed, and only with `--start-reviews`.

use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant};

use jiff::{SignedDuration, Zoned};
use serde_json::json;

use super::Outcome;
use super::at;
use super::handoff::{self, ReviewStart};
use crate::external::cards::{BoardCounts, CardKey};
use crate::external::herdr::{Agent, AgentStatus};
use crate::shared::context::{HANDOFF_TOKENS, format_tokens};
use crate::shared::events::{self, Event, EventKind, Memory};
use crate::shared::facts::{self, CollectedFacts, HubLine};
use crate::shared::lifecycle::{self, Lifecycle, decide_lifecycle};
use crate::shared::messages::{self, SendOutcome};
use crate::shared::run_folder::{FinishedSession, MinuteMark, ReportFile, RunFolder};
use crate::shared::tidy;
use crate::shared::{HubError, Tools};

/// A report younger than this may still be half written by hand, so
/// `hub next` waits for it.
const MIN_REPORT_AGE: Duration = Duration::from_secs(2);

/// The least time between two minute passes. Each pass calls `card` on
/// the network drive, git in many worktrees and the database once per
/// worker, which is too slow for every 5-second check.
const MINUTE_PASS_GAP: SignedDuration = SignedDuration::from_secs(60);

/// How long `hub next` waits and how it prints.
pub struct NextOptions {
    /// `None` waits until news comes. Zero runs one check.
    pub timeout: Option<Duration>,
    /// The wait between checks, from `HUB_POLL_SECONDS`.
    pub poll: Duration,
    pub json: bool,
    /// This process's ID, written to `.watcher-pid`.
    pub process_id: u32,
    /// The folder an old hub's session review starts in.
    pub cwd: PathBuf,
    /// Starts old hubs' session reviews, from `--start-reviews`. Off by
    /// default, since the user wants reviews only on the runs they pick.
    pub start_reviews: bool,
}

/// The one line `hub next` prints.
#[derive(Debug, PartialEq, Eq)]
enum News {
    Report {
        name: String,
        mtime_ns: u64,
        /// Set when `hub watch` found this report waited 10 minutes
        /// with no `hub next` line.
        missed: bool,
    },
    Blocked {
        name: String,
        state_seq: u64,
    },
    NotSent {
        name: String,
        reason: String,
    },
    Unsure {
        name: String,
        reason: String,
    },
    /// A timed message held for `sessions` sessions in a check.
    Timed {
        id: String,
        file: PathBuf,
        sessions: usize,
    },
    HubHandOff {
        session_id: String,
        tokens: u64,
    },
    BoardOver {
        busy: u32,
        limit: u32,
    },
    /// Sessions that became `state` for the same `why`.
    Finished {
        state: String,
        names: Vec<String>,
        why: String,
    },
    /// The old hub's message file, which this hub never sends.
    OldSendFile {
        path: PathBuf,
    },
    /// What this check did about an old hub's session review.
    Review(ReviewStart),
    /// An older hub of the run that no `handoff` event names.
    UnadoptedHub {
        old: String,
    },
    Replaced,
    Timeout,
}

/// What one check found, for `choose_news`.
struct NewsFacts<'a> {
    /// The held message that was not sent or is unsure, if any.
    outcome: Option<&'a SendOutcome>,
    /// The old hub's session review this check started or failed to
    /// start, if any.
    review: Option<&'a ReviewStart>,
    reports: &'a [ReportFile],
    agents: &'a [Agent],
    memory: &'a Memory,
    hub: &'a HubLine,
    /// Read only when the minute pass ran in this check.
    board: Option<&'a BoardCounts>,
    /// From the last minute pass, kept until each is listed.
    finished: &'a [FinishedSession],
    old_send_files: &'a [PathBuf],
}

/// Claims the watcher, takes the watcher lock, then checks every
/// `poll` until news comes or `timeout` runs out. The news's event is
/// recorded before the line prints. A process killed between recording
/// the event and printing the line loses the line. Printing the line
/// first could instead wake the hub twice for one report.
pub fn run_next(
    tools: &Tools<'_>,
    run: &RunFolder,
    options: &NextOptions,
) -> Result<Outcome, HubError> {
    run.claim_watcher(options.process_id)?;
    // Waits until the older watcher sees the claim and exits.
    let _lock = run.take_watcher_lock()?;
    let started = Instant::now();
    loop {
        if run.is_replaced(options.process_id)? {
            return Ok(make_outcome(&News::Replaced, None, options.json));
        }
        let now = Zoned::now();
        let hub = facts::read_hub_line(tools);
        if let Some(news) = check_for_news(tools, run, &hub, options, &now)? {
            // A newer watcher that took over during the check finds this
            // news again, so this one records nothing.
            if run.is_replaced(options.process_id)? {
                return Ok(make_outcome(&News::Replaced, None, options.json));
            }
            if let Some(kind) = record_news(&news) {
                let event = Event {
                    at: now.timestamp(),
                    kind,
                };
                events::append_event(run, &event)?;
            }
            return Ok(make_outcome(&news, hub.context_tokens, options.json));
        }
        if options
            .timeout
            .is_some_and(|timeout| started.elapsed() >= timeout)
        {
            return Ok(make_outcome(
                &News::Timeout,
                hub.context_tokens,
                options.json,
            ));
        }
        thread::sleep(options.poll);
    }
}

/// Runs one check. Holds the timed messages that are due, runs the
/// minute pass when it is due, starts an old hub's session review when
/// one is due and reviews are on, delivers held messages and returns
/// the news to print, if any. Records nothing about the news itself,
/// since `run_next` does that before printing.
fn check_for_news(
    tools: &Tools<'_>,
    run: &RunFolder,
    hub: &HubLine,
    options: &NextOptions,
    now: &Zoned,
) -> Result<Option<News>, HubError> {
    let mut agents = tools.herdr.list_agents()?;
    agents.retain(|agent| Some(agent.pane_id.as_str()) != tools.own_pane);
    at::hold_due_messages(run, &agents, now)?;
    // Read after the holds, so a timed message held in this check can
    // print its line in this check.
    let memory = events::load_memory(run)?;
    // Delivery comes last, since an error after it would lose its
    // not-sent or unsure line.
    let reports = run.list_reports(now)?;
    let old_send_files = run.find_old_send_files()?;
    let mut mark = run.read_minute_mark()?;
    let mut board = None;
    if is_minute_pass_due(&agents, &memory, mark.as_ref(), now) {
        board = run_minute_pass(tools, run, &agents, &memory, now)?;
        mark = run.read_minute_mark()?;
    }
    let review = if options.start_reviews {
        handoff::start_due_review(tools, run, &memory, &agents, &options.cwd, now)?
    } else {
        None
    };
    let outcome = messages::deliver_held_messages(tools, run, &agents, now)?;
    Ok(choose_news(&NewsFacts {
        outcome: outcome.as_ref(),
        review: review.as_ref(),
        reports: &reports,
        agents: &agents,
        memory: &memory,
        hub,
        board: board.as_ref(),
        finished: mark.as_ref().map_or(&[], |mark| &mark.finished),
        old_send_files: &old_send_files,
    }))
}

/// Tells whether the minute pass should run. It runs at most once a
/// minute, and only after an agent's state changed or the hub read a
/// report since the last one. With no mark, it runs.
fn is_minute_pass_due(
    agents: &[Agent],
    memory: &Memory,
    last: Option<&MinuteMark>,
    now: &Zoned,
) -> bool {
    let Some(last) = last else {
        return true;
    };
    if now.timestamp().duration_since(last.at) < MINUTE_PASS_GAP {
        return false;
    }
    let state_changed = agents
        .iter()
        .any(|agent| last.state_seqs.get(&agent.name) != Some(&agent.state_seq));
    state_changed || memory.read_count != last.read_count
}

/// Reads the board, finds the finished sessions and holds the handoff
/// asks on one collection of facts. Records the board's state on the
/// run's first pass, and `board_under` when the board fell back within
/// its limit. Saves the minute mark with the finished sessions. Returns
/// the board counts when `card board` answered and the run's memory
/// already held the board's state.
fn run_minute_pass(
    tools: &Tools<'_>,
    run: &RunFolder,
    agents: &[Agent],
    memory: &Memory,
    now: &Zoned,
) -> Result<Option<BoardCounts>, HubError> {
    // A `card` failure skips the board, as `hub status` shows it.
    let mut board = tools.cards.read_board().ok();
    if let Some(counts) = &board {
        let is_over = counts.busy > counts.limit;
        if !memory.board_known || (memory.board_over && !is_over) {
            record_board_state(run, counts, is_over, now)?;
        }
        // A board already over its limit when the run started is not a
        // crossing, so the first pass prints no board line.
        if !memory.board_known {
            board = None;
        }
    }
    let collected = facts::collect_session_facts(tools, run, memory)?;
    let finished = find_finished_sessions(&collected);
    forget_left_sessions(run, &finished, memory, now)?;
    ask_for_worker_handoffs(run, &collected, memory, now)?;
    run.write_minute_mark(&MinuteMark {
        at: now.timestamp(),
        state_seqs: agents
            .iter()
            .map(|agent| (agent.name.clone(), agent.state_seq))
            .collect(),
        read_count: memory.read_count,
        finished,
    })?;
    Ok(board)
}

/// Records `board_over` or `board_under` with the board's counts.
fn record_board_state(
    run: &RunFolder,
    counts: &BoardCounts,
    is_over: bool,
    now: &Zoned,
) -> Result<(), HubError> {
    let (busy, limit) = (counts.busy, counts.limit);
    let kind = if is_over {
        EventKind::BoardOver { busy, limit }
    } else {
        EventKind::BoardUnder { busy, limit }
    };
    let event = Event {
        at: now.timestamp(),
        kind,
    };
    events::append_event(run, &event)
}

/// Returns each `to-close` and `to-park` session with why, as
/// `hub clean` prints it.
fn find_finished_sessions(collected: &CollectedFacts) -> Vec<FinishedSession> {
    collected
        .sessions
        .iter()
        .filter_map(|facts| {
            let verdict = decide_lifecycle(facts);
            let why = match (verdict.state, &facts.card) {
                (Lifecycle::ToClose, _) => tidy::describe_close_why(facts, &verdict),
                (Lifecycle::ToPark, Some(card)) => format!("{} review", card.key.as_str()),
                _ => return None,
            };
            Some(FinishedSession {
                name: facts.name.clone(),
                state: verdict.state.word().to_owned(),
                why,
            })
        })
        .collect()
}

/// Logs the listed sessions that left the state they were listed in,
/// so each is listed again if it comes back to that state.
fn forget_left_sessions(
    run: &RunFolder,
    finished: &[FinishedSession],
    memory: &Memory,
    now: &Zoned,
) -> Result<(), HubError> {
    let names: Vec<String> = memory
        .listed
        .iter()
        .filter(|(name, state)| {
            !finished
                .iter()
                .any(|session| &session.name == *name && &session.state == *state)
        })
        .map(|(name, _)| name.clone())
        .collect();
    if names.is_empty() {
        return Ok(());
    }
    let left = Event {
        at: now.timestamp(),
        kind: EventKind::Unlisted { names },
    };
    events::append_event(run, &left)
}

/// Holds one handoff ask for each worker on a card that is working,
/// idle or done, has `HANDOFF_TOKENS` tokens or more, and never got
/// one, then records the ask. Delivery sends it by the usual rules.
/// A `to-close` or `to-park` session gets no ask. `hub clean` may
/// close or park it, and delivering the ask would reopen finished work.
fn ask_for_worker_handoffs(
    run: &RunFolder,
    collected: &CollectedFacts,
    memory: &Memory,
    now: &Zoned,
) -> Result<(), HubError> {
    for facts in &collected.sessions {
        let Some(agent) = &facts.agent else {
            continue;
        };
        let Some(session) = &agent.session_id else {
            continue;
        };
        let tokens = facts.context_tokens.unwrap_or_default();
        let is_worker = facts.card.is_some() || CardKey::find_in_name(&facts.name).is_some();
        let is_free = matches!(
            agent.status,
            AgentStatus::Working | AgentStatus::Idle | AgentStatus::Done
        );
        let is_finished = matches!(
            decide_lifecycle(facts).state,
            Lifecycle::ToClose | Lifecycle::ToPark
        );
        if !is_worker
            || !is_free
            || tokens < HANDOFF_TOKENS
            || memory.handoff_asked.contains(session)
            || is_finished
        {
            continue;
        }
        let ask = format!(
            "Your context is about {}. Write your handoff with herdr-handoff now, \
             put its path in your report, and end your turn.",
            format_tokens(Some(tokens))
        );
        messages::hold_message(run, &facts.name, None, &ask, now)?;
        let asked = Event {
            at: now.timestamp(),
            kind: EventKind::HandoffAsked {
                name: facts.name.clone(),
                session: session.clone(),
            },
        };
        events::append_event(run, &asked)?;
    }
    Ok(())
}

/// Picks the most urgent news in a fixed order, or `None` when nothing
/// is new. A message that was not sent or is unsure comes first, since
/// nothing prints it again. Then come an old hub's review start,
/// reports, blocked states, timed messages held, the hub's own
/// handoff, the board crossing its limit, finished sessions not listed
/// yet, old hub message files not printed yet and older hubs left
/// without a handoff.
fn choose_news(facts: &NewsFacts<'_>) -> Option<News> {
    match facts.outcome {
        Some(SendOutcome::NotSent { name, reason }) => {
            return Some(News::NotSent {
                name: name.clone(),
                reason: reason.clone(),
            });
        }
        Some(SendOutcome::Unsure { name, reason, .. }) => {
            return Some(News::Unsure {
                name: name.clone(),
                reason: reason.clone(),
            });
        }
        Some(SendOutcome::Delivered) | None => {}
    }
    if let Some(review) = facts.review {
        return Some(News::Review(review.clone()));
    }
    if let Some(report) = find_unannounced_reports(facts.reports, facts.memory).first() {
        return Some(News::Report {
            name: report.name.clone(),
            mtime_ns: report.mtime_ns,
            missed: facts.memory.missed.get(&report.name) == Some(&report.mtime_ns),
        });
    }
    if let Some(agent) = find_new_blocked_states(facts.agents, facts.memory).first() {
        return Some(News::Blocked {
            name: agent.name.clone(),
            state_seq: agent.state_seq,
        });
    }
    if let Some((id, untold)) = facts.memory.timed_untold.first_key_value() {
        return Some(News::Timed {
            id: id.clone(),
            file: untold.file.clone(),
            sessions: untold.sessions,
        });
    }
    if let (Some(session_id), Some(tokens)) = (&facts.hub.session_id, facts.hub.context_tokens)
        && tokens >= HANDOFF_TOKENS
        && !facts.memory.hub_context_told.contains(session_id)
    {
        return Some(News::HubHandOff {
            session_id: session_id.clone(),
            tokens,
        });
    }
    if let Some(board) = facts.board
        && board.busy > board.limit
        && !facts.memory.board_over
    {
        return Some(News::BoardOver {
            busy: board.busy,
            limit: board.limit,
        });
    }
    if let Some(news) = find_unlisted_finished(facts.finished, facts.memory) {
        return Some(news);
    }
    if let Some(path) = facts
        .old_send_files
        .iter()
        .find(|path| !facts.memory.old_send_seen.contains(*path))
    {
        return Some(News::OldSendFile { path: path.clone() });
    }
    let own = facts.hub.name.as_deref()?;
    let predecessor = facts.memory.predecessor.as_deref();
    facts
        .agents
        .iter()
        .find(|agent| {
            lifecycle::is_unadopted_hub(own, &agent.name, predecessor)
                && !facts.memory.unadopted_seen.contains(&agent.name)
        })
        .map(|agent| News::UnadoptedHub {
            old: agent.name.clone(),
        })
}

/// Returns the first finished session not yet listed in its state,
/// with every other such session of the same state and why. Each later
/// check lists the next group.
fn find_unlisted_finished(finished: &[FinishedSession], memory: &Memory) -> Option<News> {
    let unlisted: Vec<&FinishedSession> = finished
        .iter()
        .filter(|session| memory.listed.get(&session.name) != Some(&session.state))
        .collect();
    let first = unlisted.first()?;
    let names = unlisted
        .iter()
        .filter(|session| session.state == first.state && session.why == first.why)
        .map(|session| session.name.clone())
        .collect();
    Some(News::Finished {
        state: first.state.clone(),
        names,
        why: first.why.clone(),
    })
}

/// Returns reports at least 2 seconds old whose current version was
/// not announced.
fn find_unannounced_reports<'a>(reports: &'a [ReportFile], memory: &Memory) -> Vec<&'a ReportFile> {
    reports
        .iter()
        .filter(|report| report.age >= MIN_REPORT_AGE)
        .filter(|report| memory.announced.get(&report.name) != Some(&report.mtime_ns))
        .collect()
}

/// Returns blocked agents whose state change was not printed yet.
fn find_new_blocked_states<'a>(agents: &'a [Agent], memory: &Memory) -> Vec<&'a Agent> {
    agents
        .iter()
        .filter(|agent| agent.status == AgentStatus::Blocked)
        .filter(|agent| memory.blocked_seen.get(&agent.name) != Some(&agent.state_seq))
        .collect()
}

/// Returns the event that records this news, or `None` for news that
/// needs no memory. A not-sent or unsure message was recorded when it
/// was delivered.
fn record_news(news: &News) -> Option<EventKind> {
    match news {
        News::Report { name, mtime_ns, .. } => Some(EventKind::Announced {
            name: name.clone(),
            mtime_ns: *mtime_ns,
        }),
        News::Blocked { name, state_seq } => Some(EventKind::BlockedSeen {
            name: name.clone(),
            state_seq: *state_seq,
        }),
        News::Timed { id, .. } => Some(EventKind::TimedTold { id: id.clone() }),
        News::HubHandOff { session_id, .. } => Some(EventKind::HubContextTold {
            session: session_id.clone(),
        }),
        News::BoardOver { busy, limit } => Some(EventKind::BoardOver {
            busy: *busy,
            limit: *limit,
        }),
        News::Finished { state, names, .. } => Some(EventKind::Listed {
            state: state.clone(),
            names: names.clone(),
        }),
        News::OldSendFile { path } => Some(EventKind::OldSendSeen { path: path.clone() }),
        News::Review(ReviewStart::Started { old, reviewer }) => Some(EventKind::ReviewStarted {
            old: old.clone(),
            reviewer: reviewer.clone(),
        }),
        News::Review(ReviewStart::Failed { old, reason, .. }) => Some(EventKind::ReviewFailed {
            old: old.clone(),
            reason: events::cut_reason(reason),
        }),
        News::UnadoptedHub { old } => Some(EventKind::UnadoptedHubSeen { old: old.clone() }),
        News::NotSent { .. } | News::Unsure { .. } | News::Replaced | News::Timeout => None,
    }
}

/// Returns the one line to print, as text or as JSON.
fn make_outcome(news: &News, hub_tokens: Option<u64>, json: bool) -> Outcome {
    Outcome::line(if json {
        format_news_json(news, hub_tokens)
    } else {
        format_news_line(news, hub_tokens)
    })
}

/// Writes news as one line. Most lines end with the hub's own context,
/// such as `report docs-fix (hub 131k)`. A finished line ends with what
/// the hub should do, such as
/// `to-close demo-kb-001-impl: DEMO-KB-001 accepted; run hub clean`.
fn format_news_line(news: &News, hub_tokens: Option<u64>) -> String {
    let hub = format!("(hub {})", format_tokens(hub_tokens));
    match news {
        News::Report {
            name, missed: true, ..
        } => format!("missed report {name} {hub}"),
        News::Report { name, .. } => format!("report {name} {hub}"),
        News::Blocked { name, .. } => format!("blocked {name} {hub}"),
        News::Timed { file, sessions, .. } => {
            let sessions = match sessions {
                1 => "1 session".to_owned(),
                count => format!("{count} sessions"),
            };
            format!("timed message to {sessions}: {} {hub}", file.display())
        }
        News::NotSent { name, reason } => format!("not sent {name}: {reason} {hub}"),
        News::Unsure { name, reason } => format!("unsure {name}: {reason} {hub}"),
        News::HubHandOff { tokens, .. } => {
            format!("hand off now: hub at {}", format_tokens(Some(*tokens)))
        }
        News::BoardOver { busy, limit } => format!("board over its limit: {busy}/{limit} {hub}"),
        News::Finished { state, names, why } => {
            format!("{state} {}: {why}; run hub clean", names.join(", "))
        }
        News::OldSendFile { path } => format!("old send file: {} {hub}", path.display()),
        News::Review(ReviewStart::Started { reviewer, .. }) => {
            format!("review started {reviewer} {hub}")
        }
        News::Review(ReviewStart::Failed {
            old,
            review,
            reason,
        }) => format!(
            "review not started for {old}: {reason}; start it from {} with herdr-handoff {hub}",
            review.display()
        ),
        News::UnadoptedHub { old } => format!("no hub handoff for {old} -> hub adopt {old} {hub}"),
        News::Replaced => "replaced by a newer hub next".to_owned(),
        News::Timeout => format!("timeout {hub}"),
    }
}

/// Writes news as one JSON object with the same facts, such as
/// `{"news":"report","name":"docs-fix","detail":null,"hub_context_tokens":131204}`.
fn format_news_json(news: &News, hub_tokens: Option<u64>) -> String {
    let (word, name, detail) = match news {
        News::Report {
            name, missed: true, ..
        } => ("missed_report", Some(name.clone()), None),
        News::Report { name, .. } => ("report", Some(name.clone()), None),
        News::Blocked { name, .. } => ("blocked", Some(name.clone()), None),
        News::Timed { file, .. } => ("timed_message", None, Some(file.display().to_string())),
        News::NotSent { name, reason } => ("not_sent", Some(name.clone()), Some(reason.clone())),
        News::Unsure { name, reason } => ("unsure", Some(name.clone()), Some(reason.clone())),
        News::HubHandOff { .. } => ("hand_off", None, None),
        News::BoardOver { busy, limit } => ("board_over", None, Some(format!("{busy}/{limit}"))),
        News::Finished { state, names, why } => {
            (state.as_str(), Some(names.join(", ")), Some(why.clone()))
        }
        News::OldSendFile { path } => ("old_send_file", None, Some(path.display().to_string())),
        News::Review(ReviewStart::Started { reviewer, .. }) => {
            ("review_started", Some(reviewer.clone()), None)
        }
        News::Review(ReviewStart::Failed {
            old,
            review,
            reason,
        }) => (
            "review_failed",
            Some(old.clone()),
            Some(format!("{reason}; start it from {}", review.display())),
        ),
        News::UnadoptedHub { old } => ("unadopted_hub", Some(old.clone()), None),
        News::Replaced => ("replaced", None, None),
        News::Timeout => ("timeout", None, None),
    };
    json!({
        // A state such as `to-park` prints as the word `to_park`.
        "news": word.replace('-', "_"),
        "name": name,
        "detail": detail,
        "hub_context_tokens": hub_tokens,
    })
    .to_string()
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::*;
    use crate::shared::test_values::make_agent;

    /// A report announced again on every restart costs the hub a turn
    /// each time, and one rewritten after it was announced must wake the
    /// hub again or the hub never reads the new version. A report
    /// younger than 2 seconds may be half written by hand.
    #[test]
    fn find_unannounced_reports_follows_age_and_memory() {
        let memory = Memory {
            announced: [("docs-fix".to_owned(), 100)].into(),
            ..Memory::default()
        };
        let report = |mtime_ns, seconds| ReportFile {
            name: "docs-fix".to_owned(),
            mtime_ns,
            age: Duration::from_secs(seconds),
        };
        let cases = [
            ("announced-same-version", report(100, 60), false),
            ("rewritten-after-announce", report(200, 60), true),
            ("younger-than-two-seconds", report(200, 1), false),
            ("two-seconds-old", report(200, 2), true),
        ];

        for (case_name, report, expected) in cases {
            let reports = [report];

            let found = find_unannounced_reports(&reports, &memory);

            assert_eq!(!found.is_empty(), expected, "{case_name}");
        }
    }

    /// The minute pass reads the board on the network drive, runs git
    /// in many worktrees and asks the database about every worker. Run
    /// on every check, it slows each check down. Skipped after a state
    /// change or a read, it leaves finished tabs open and misses a
    /// worker past 150k. With no mark, as after the first start, it
    /// runs.
    #[test]
    fn is_minute_pass_due_follows_mark() {
        struct Case {
            name: &'static str,
            mark: Option<MinuteMark>,
            read_count: usize,
            due: bool,
        }
        let now: Zoned = "2026-11-11T22:14:00+00:00[UTC]".parse().unwrap();
        let mark = |at: &str, state_seq: u64| MinuteMark {
            at: at.parse().unwrap(),
            state_seqs: [("docs-fix".to_owned(), state_seq)].into(),
            read_count: 3,
            finished: Vec::new(),
        };
        let cases = [
            Case {
                name: "no-mark",
                mark: None,
                read_count: 3,
                due: true,
            },
            Case {
                name: "state-changed-within-minute",
                mark: Some(mark("2026-11-11T22:13:30Z", 0)),
                read_count: 3,
                due: false,
            },
            Case {
                name: "nothing-changed",
                mark: Some(mark("2026-11-11T22:12:00Z", 1)),
                read_count: 3,
                due: false,
            },
            Case {
                name: "state-changed",
                mark: Some(mark("2026-11-11T22:12:00Z", 0)),
                read_count: 3,
                due: true,
            },
            Case {
                name: "report-read",
                mark: Some(mark("2026-11-11T22:12:00Z", 1)),
                read_count: 4,
                due: true,
            },
        ];
        let agents = [make_agent("docs-fix", AgentStatus::Idle)];

        for case in cases {
            let memory = Memory {
                read_count: case.read_count,
                ..Memory::default()
            };

            let due = is_minute_pass_due(&agents, &memory, case.mark.as_ref(), &now);

            assert_eq!(due, case.due, "{}", case.name);
        }
    }

    /// A message that was not sent already left the queue, so if a
    /// report or a blocked state wins the line instead, nothing ever
    /// tells the hub and the message is silently lost.
    #[test]
    fn choose_news_puts_send_problems_first() {
        let outcome = SendOutcome::NotSent {
            name: "docs-fix".to_owned(),
            reason: "the session is gone".to_owned(),
        };
        let reports = [ReportFile {
            name: "notes-fix".to_owned(),
            mtime_ns: 100,
            age: Duration::from_secs(60),
        }];
        let agents = [make_agent("demo-kb-009-auto", AgentStatus::Blocked)];
        let memory = Memory::default();
        let hub = HubLine::default();
        let facts = NewsFacts {
            outcome: Some(&outcome),
            review: None,
            reports: &reports,
            agents: &agents,
            memory: &memory,
            hub: &hub,
            board: None,
            finished: &[],
            old_send_files: &[],
        };

        let news = choose_news(&facts);

        assert_eq!(
            news,
            Some(News::NotSent {
                name: "docs-fix".to_owned(),
                reason: "the session is gone".to_owned(),
            })
        );
    }

    /// Scripts and the later board read `hub next --json`, so a renamed
    /// field or a missing null breaks them without a word.
    #[test]
    fn format_news_json_matches_example_object() {
        let cases = [
            (
                "report",
                News::Report {
                    name: "docs-fix".to_owned(),
                    mtime_ns: 100,
                    missed: false,
                },
                Some(131_204),
                r#"{"news":"report","name":"docs-fix","detail":null,"hub_context_tokens":131204}"#,
            ),
            (
                "not-sent",
                News::NotSent {
                    name: "docs-fix".to_owned(),
                    reason: "the session is gone".to_owned(),
                },
                None,
                r#"{"news":"not_sent","name":"docs-fix","detail":"the session is gone","hub_context_tokens":null}"#,
            ),
            (
                "board-over",
                News::BoardOver { busy: 25, limit: 8 },
                Some(131_204),
                r#"{"news":"board_over","name":null,"detail":"25/8","hub_context_tokens":131204}"#,
            ),
        ];

        for (case_name, news, hub_tokens, expected) in cases {
            let printed: Value =
                serde_json::from_str(&format_news_json(&news, hub_tokens)).unwrap();

            let expected: Value = serde_json::from_str(expected).unwrap();
            assert_eq!(printed, expected, "{case_name}");
        }
    }
}
