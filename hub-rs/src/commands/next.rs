//! Runs `hub next`, which waits until something needs the hub and
//! prints one line. Read `run_next`, then `check_for_news`.
//! `choose_news` is pure and holds the order of the news lines.

use std::thread;
use std::time::{Duration, Instant};

use jiff::Zoned;
use serde_json::json;

use super::Outcome;
use crate::outside::herdr::{Agent, AgentStatus};
use crate::shared::context::{HANDOFF_TOKENS, format_tokens};
use crate::shared::events::{self, Event, EventKind, Memory};
use crate::shared::facts::{self, HubLine};
use crate::shared::messages::{self, SendOutcome};
use crate::shared::run_folder::{ReportFile, RunFolder};
use crate::shared::{HubError, Tools};

/// A report younger than this may still be half written by hand, so
/// `hub next` waits for it.
const MIN_REPORT_AGE: Duration = Duration::from_secs(2);

/// How long `hub next` waits and how it prints.
pub struct NextOptions {
    /// `None` waits until news comes. Zero runs one check.
    pub timeout: Option<Duration>,
    /// The wait between checks, from `HUB_POLL_SECONDS`.
    pub poll: Duration,
    pub json: bool,
    /// This process's ID, written to `.watcher-pid`.
    pub process_id: u32,
}

/// The one line `hub next` prints.
#[derive(Debug, PartialEq, Eq)]
enum News {
    Report { name: String, mtime_ns: u64 },
    Blocked { name: String, state_seq: u64 },
    NotSent { name: String, reason: String },
    Unsure { name: String, reason: String },
    HubHandOff { session_id: String, tokens: u64 },
    Replaced,
    Timeout,
}

/// What one check found, for `choose_news`.
struct NewsFacts<'a> {
    /// The held message that was not sent or is unsure, if any.
    outcome: Option<&'a SendOutcome>,
    reports: &'a [ReportFile],
    agents: &'a [Agent],
    memory: &'a Memory,
    hub: &'a HubLine,
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
        if let Some(news) = check_for_news(tools, run, &hub, &now)? {
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

/// Runs one check. Delivers held messages and returns the news to
/// print, if any. Records nothing about the news itself, since
/// `run_next` does that before printing.
fn check_for_news(
    tools: &Tools<'_>,
    run: &RunFolder,
    hub: &HubLine,
    now: &Zoned,
) -> Result<Option<News>, HubError> {
    let memory = events::load_memory(run)?;
    let mut agents = tools.herdr.list_agents()?;
    agents.retain(|agent| Some(agent.pane_id.as_str()) != tools.own_pane);
    // Delivery comes last, since an error after it would lose its
    // not-sent or unsure line.
    let reports = run.list_reports(now)?;
    let outcome = messages::deliver_held_messages(tools, run, &agents, now)?;
    Ok(choose_news(&NewsFacts {
        outcome: outcome.as_ref(),
        reports: &reports,
        agents: &agents,
        memory: &memory,
        hub,
    }))
}

/// Picks the most urgent news in a fixed order, or `None` when nothing
/// is new. A message that was not sent or is unsure comes first, since
/// nothing prints it again. Then come reports, blocked states and the
/// hub's own handoff.
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
    if let Some(report) = find_unannounced_reports(facts.reports, facts.memory).first() {
        return Some(News::Report {
            name: report.name.clone(),
            mtime_ns: report.mtime_ns,
        });
    }
    if let Some(agent) = find_new_blocked_states(facts.agents, facts.memory).first() {
        return Some(News::Blocked {
            name: agent.name.clone(),
            state_seq: agent.state_seq,
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
    None
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
        News::Report { name, mtime_ns } => Some(EventKind::Announced {
            name: name.clone(),
            mtime_ns: *mtime_ns,
        }),
        News::Blocked { name, state_seq } => Some(EventKind::BlockedSeen {
            name: name.clone(),
            state_seq: *state_seq,
        }),
        News::HubHandOff { session_id, .. } => Some(EventKind::HubContextTold {
            session: session_id.clone(),
        }),
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
/// such as `report docs-fix (hub 131k)`.
fn format_news_line(news: &News, hub_tokens: Option<u64>) -> String {
    let hub = format!("(hub {})", format_tokens(hub_tokens));
    match news {
        News::Report { name, .. } => format!("report {name} {hub}"),
        News::Blocked { name, .. } => format!("blocked {name} {hub}"),
        News::NotSent { name, reason } => format!("not sent {name}: {reason} {hub}"),
        News::Unsure { name, reason } => format!("unsure {name}: {reason} {hub}"),
        News::HubHandOff { tokens, .. } => {
            format!("hand off now: hub at {}", format_tokens(Some(*tokens)))
        }
        News::Replaced => "replaced by a newer hub next".to_owned(),
        News::Timeout => format!("timeout {hub}"),
    }
}

/// Writes news as one JSON object with the same facts, such as
/// `{"news":"report","name":"docs-fix","detail":null,"hub_context_tokens":131204}`.
fn format_news_json(news: &News, hub_tokens: Option<u64>) -> String {
    let (word, name, detail) = match news {
        News::Report { name, .. } => ("report", Some(name), None),
        News::Blocked { name, .. } => ("blocked", Some(name), None),
        News::NotSent { name, reason } => ("not_sent", Some(name), Some(reason)),
        News::Unsure { name, reason } => ("unsure", Some(name), Some(reason)),
        News::HubHandOff { .. } => ("hand_off", None, None),
        News::Replaced => ("replaced", None, None),
        News::Timeout => ("timeout", None, None),
    };
    json!({
        "news": word,
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
            reports: &reports,
            agents: &agents,
            memory: &memory,
            hub: &hub,
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
        ];

        for (case_name, news, hub_tokens, expected) in cases {
            let printed: Value =
                serde_json::from_str(&format_news_json(&news, hub_tokens)).unwrap();

            let expected: Value = serde_json::from_str(expected).unwrap();
            assert_eq!(printed, expected, "{case_name}");
        }
    }
}
