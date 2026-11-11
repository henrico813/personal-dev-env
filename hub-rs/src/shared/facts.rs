//! Gathers every session's facts from the outside systems, calling
//! each as few times as it can. Read `collect_session_facts` first.
//! The small functions after it decide which slow calls a session
//! needs.

use std::collections::{BTreeMap, BTreeSet};

use super::events::Memory;
use super::lifecycle::{CardFacts, ReportFacts, SessionFacts};
use super::messages;
use super::parking::ParkedRecord;
use super::run_folder::{ReportedNames, RunFolder};
use super::{HubError, Tools};
use crate::outside::cards::{CardError, CardKey, CardNote, Stage};
use crate::outside::git::{self, Unsaved};
use crate::outside::herdr::{Agent, AgentKind, AgentStatus};
use crate::outside::requests::PrRequest;

/// The facts of every session, and what the status view needs beside
/// them.
#[derive(Debug)]
pub(crate) struct CollectedFacts {
    pub sessions: Vec<SessionFacts>,
    /// The names with an unread report, a gone session's included.
    pub unread: Vec<String>,
    /// Active and review cards with no live or parked session.
    pub no_session: Vec<(CardKey, Stage)>,
    /// One line per failed source, such as `cards: ...`.
    pub problems: Vec<String>,
}

/// The hub's own name, session and context, for the first status line
/// and the end of each news line.
#[derive(Debug, Default)]
pub(crate) struct HubLine {
    pub name: Option<String>,
    pub session_id: Option<String>,
    pub context_tokens: Option<u64>,
}

/// Gathers the facts of every live session but the hub's own, and of
/// every parked session that is not live again. A card, database or
/// request file failure becomes a problem line, not an error. A
/// request file that does not parse also keeps every live session.
pub(crate) fn collect_session_facts(
    tools: &Tools<'_>,
    run: &RunFolder,
    memory: &Memory,
) -> Result<CollectedFacts, HubError> {
    let mut problems = Vec::new();
    let agents: Vec<Agent> = tools
        .herdr
        .list_agents()?
        .into_iter()
        .filter(|agent| Some(agent.pane_id.as_str()) != tools.own_pane)
        .collect();
    let live_names: BTreeSet<&str> = agents.iter().map(|agent| agent.name.as_str()).collect();
    let parked: Vec<ParkedRecord> = tools
        .parked
        .list_parked_records()?
        .into_iter()
        .filter(|record| !live_names.contains(record.name.as_str()))
        .collect();
    let (stages, stages_failed) = match tools.cards.list_stages() {
        Ok(stages) => (stages, false),
        Err(error) => {
            add_card_problem(&mut problems, &error);
            (BTreeMap::new(), true)
        }
    };
    let named_keys = agents
        .iter()
        .filter_map(|agent| CardKey::find_in_name(&agent.name))
        .chain(parked.iter().map(|record| record.card.clone()));
    let mut notes = BTreeMap::new();
    let mut failed_keys = BTreeSet::new();
    for key in named_keys {
        let is_open = matches!(
            stages.get(&key),
            Some(Stage::Draft | Stage::Ready | Stage::Active | Stage::Review)
        );
        if !is_open || notes.contains_key(&key) {
            continue;
        }
        match tools.cards.read_card(&key) {
            Ok(note) => {
                notes.insert(key, note);
            }
            Err(error) => {
                add_card_problem(&mut problems, &error);
                failed_keys.insert(key);
            }
        }
    }
    let open_requests = tools
        .requests
        .list_open_requests()
        .map_err(|source| HubError::File {
            path: tools.requests.folder().to_owned(),
            source,
        })?;
    // A file that does not parse may hold a session's waiting approval,
    // so every live session stays until the file is fixed.
    let has_bad_request = open_requests.bad_files > 0;
    if has_bad_request {
        problems.push(format!(
            "requests: {} files did not parse",
            open_requests.bad_files
        ));
    }
    let reported = run.list_reported_names()?;
    let counts = messages::count_messages(run)?;
    let live_ids: BTreeSet<String> = agents
        .iter()
        .filter_map(|agent| agent.session_id.clone())
        .collect();

    let mut sessions = Vec::new();
    for agent in agents {
        let session_id = agent.session_id.as_deref();
        let key = find_session_card(&agent.name, session_id, &notes);
        let card = key.as_ref().and_then(|key| {
            let note = notes.get(key);
            let stage = *stages.get(key)?;
            Some(CardFacts {
                key: key.clone(),
                stage,
                needs_you_open: note.is_some_and(|note| note.needs_you_open),
                newer_session_live: note
                    .is_some_and(|note| has_newer_live_session(note, session_id, &live_ids)),
            })
        });
        let card_unknown = stages_failed
            || CardKey::find_in_name(&agent.name)
                .is_some_and(|key| !stages.contains_key(&key) || failed_keys.contains(&key));
        let reports = read_report_facts(&agent.name, &reported, memory);
        let report_read = reports.ever_reported && !reports.unread;
        let stage = card.as_ref().map(|card| card.stage);
        let unsaved = if needs_git_check(&agent, stage, report_read) {
            check_card_worktrees(&agent, key.as_ref())
        } else {
            Unsaved::NotChecked
        };
        let context_tokens = match (&agent.kind, session_id, &agent.machine) {
            // Only this machine's OpenCode database can be read.
            (AgentKind::OpenCode, Some(session), None) => tools
                .opencode
                .read_context_tokens(session)
                .unwrap_or_else(|error| {
                    add_problem_once(&mut problems, format!("context: {error}"));
                    None
                }),
            _ => None,
        };
        sessions.push(SessionFacts {
            name: agent.name.clone(),
            approval_pending: has_bad_request
                || match_open_request(&agent, &open_requests.requests),
            is_predecessor: memory.predecessor.as_deref() == Some(agent.name.as_str()),
            messages: counts.get(&agent.name).copied().unwrap_or_default(),
            agent: Some(agent),
            parked: None,
            card,
            card_unknown,
            reports,
            unsaved,
            context_tokens,
        });
    }
    for record in parked {
        let card = stages.get(&record.card).map(|stage| CardFacts {
            key: record.card.clone(),
            stage: *stage,
            needs_you_open: notes
                .get(&record.card)
                .is_some_and(|note| note.needs_you_open),
            newer_session_live: false,
        });
        sessions.push(SessionFacts {
            name: record.name.clone(),
            agent: None,
            card,
            card_unknown: false,
            reports: read_report_facts(&record.name, &reported, memory),
            messages: counts.get(&record.name).copied().unwrap_or_default(),
            unsaved: Unsaved::NotChecked,
            approval_pending: false,
            is_predecessor: false,
            context_tokens: None,
            parked: Some(record),
        });
    }

    let worked_on: BTreeSet<&CardKey> = sessions
        .iter()
        .filter_map(|facts| facts.card.as_ref().map(|card| &card.key))
        .collect();
    let no_session = stages
        .iter()
        .filter(|(key, stage)| {
            matches!(stage, Stage::Active | Stage::Review) && !worked_on.contains(key)
        })
        .map(|(key, stage)| (key.clone(), *stage))
        .collect();
    Ok(CollectedFacts {
        sessions,
        unread: reported.unread.into_iter().collect(),
        no_session,
        problems,
    })
}

/// Reads the hub's own agent and its context. Anything that fails
/// leaves a field `None`, so a news line still prints.
pub(crate) fn read_hub_line(tools: &Tools<'_>) -> HubLine {
    let Some(pane) = tools.own_pane else {
        return HubLine::default();
    };
    let Ok(agent) = tools.herdr.get_agent(pane) else {
        return HubLine::default();
    };
    let context_tokens = agent
        .session_id
        .as_deref()
        .and_then(|session| tools.opencode.read_context_tokens(session).ok().flatten());
    HubLine {
        name: Some(agent.name),
        session_id: agent.session_id,
        context_tokens,
    }
}

/// Returns the card key in a session's name, else the key of a card
/// read here whose `sessions` list holds its ID.
fn find_session_card(
    name: &str,
    session_id: Option<&str>,
    notes: &BTreeMap<CardKey, CardNote>,
) -> Option<CardKey> {
    if let Some(key) = CardKey::find_in_name(name) {
        return Some(key);
    }
    let session_id = session_id?;
    notes
        .iter()
        .find(|(_, note)| note.sessions.iter().any(|session| session == session_id))
        .map(|(key, _)| key.clone())
}

/// Reads the session's reports from the run folder and the hub's
/// memory. A message delivered after the last `hub read` counts as a
/// reply.
fn read_report_facts(name: &str, reported: &ReportedNames, memory: &Memory) -> ReportFacts {
    let last_read = memory.last_read.get(name);
    let last_delivered = memory.last_delivered.get(name);
    ReportFacts {
        unread: reported.unread.contains(name),
        ever_reported: reported.any.contains(name),
        replied_since_report: last_delivered.is_some() && last_delivered > last_read,
    }
}

/// Tells whether a session could be parked or closed, so git must
/// look first. Only an idle or done session on this machine needs it,
/// and only when its card is in review, accepted or dropped, or when
/// it has no card and a read report.
fn needs_git_check(agent: &Agent, stage: Option<Stage>, report_read: bool) -> bool {
    let is_free = matches!(agent.status, AgentStatus::Idle | AgentStatus::Done);
    let could_finish = match stage {
        Some(Stage::Review | Stage::Accepted | Stage::Dropped) => true,
        Some(Stage::Draft | Stage::Ready | Stage::Active) => false,
        None => report_read,
    };
    agent.machine.is_none() && is_free && could_finish
}

/// Returns the worst unsaved state across the session's linked
/// worktrees, turning a git failure into `Unsaved::GitFailed`.
fn check_card_worktrees(agent: &Agent, key: Option<&CardKey>) -> Unsaved {
    // git finds worktrees only from a folder, so with none nothing is
    // known to be safe.
    if agent.folders.is_empty() {
        return Unsaved::GitFailed("Herdr gave no folder".to_owned());
    }
    let worktrees = match git::find_card_worktrees(&agent.folders, key) {
        Ok(worktrees) => worktrees,
        Err(error) => return Unsaved::GitFailed(error.to_string()),
    };
    let mut worst = Unsaved::Clean;
    for worktree in worktrees {
        match git::check_unsaved(&worktree) {
            Ok(Unsaved::Files(path)) => worst = Unsaved::Files(path),
            Ok(Unsaved::Commits(path)) if worst == Unsaved::Clean => {
                worst = Unsaved::Commits(path);
            }
            Ok(_) => {}
            Err(error) => return Unsaved::GitFailed(error.to_string()),
        }
    }
    worst
}

/// Tells whether this session has a later session on the card's list
/// that is still live. A session the card does not list never counts
/// as handed off.
fn has_newer_live_session(
    note: &CardNote,
    session_id: Option<&str>,
    live_ids: &BTreeSet<String>,
) -> bool {
    let Some(place) =
        session_id.and_then(|id| note.sessions.iter().position(|session| session == id))
    else {
        return false;
    };
    note.sessions[place + 1..]
        .iter()
        .any(|session| live_ids.contains(session))
}

/// Tells whether an open request came from the agent's pane or
/// session.
fn match_open_request(agent: &Agent, requests: &[PrRequest]) -> bool {
    requests.iter().any(|request| {
        request.pane_id.as_deref() == Some(agent.pane_id.as_str())
            || (request.session_id.is_some() && request.session_id == agent.session_id)
    })
}

/// Adds one `cards:` problem line, once however many card calls fail.
fn add_card_problem(problems: &mut Vec<String>, error: &CardError) {
    add_problem_once(problems, format!("cards: {error}"));
}

/// Adds a problem line unless one from the same source is there.
fn add_problem_once(problems: &mut Vec<String>, line: String) {
    let source = line.split(':').next().unwrap_or_default();
    if !problems
        .iter()
        .any(|known| known.split(':').next() == Some(source))
    {
        problems.push(line);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::test_values::make_agent;

    /// An open request keeps its session open, so the agent can read
    /// the result after the user approves. Matching the wrong session
    /// lets the tidy pass close the one that waits. A request and an
    /// agent that both lack a session ID must not match.
    #[test]
    fn match_open_request_needs_pane_or_session() {
        struct Case {
            name: &'static str,
            request: PrRequest,
            agent_session: Option<&'static str>,
            matched: bool,
        }
        let request = |pane_id: Option<&str>, session_id: Option<&str>| PrRequest {
            pane_id: pane_id.map(str::to_owned),
            session_id: session_id.map(str::to_owned),
        };
        let cases = [
            Case {
                name: "same-pane",
                request: request(Some("w1:demo-kb-007-impl"), None),
                agent_session: None,
                matched: true,
            },
            Case {
                name: "same-session",
                request: request(Some("w1:p9"), Some("ses_007")),
                agent_session: Some("ses_007"),
                matched: true,
            },
            Case {
                name: "other-session",
                request: request(None, Some("ses_009")),
                agent_session: Some("ses_007"),
                matched: false,
            },
            Case {
                name: "neither-has-session",
                request: request(Some("w1:p9"), None),
                agent_session: None,
                matched: false,
            },
        ];

        for case in cases {
            let agent = Agent {
                session_id: case.agent_session.map(str::to_owned),
                ..make_agent("demo-kb-007-impl", AgentStatus::Idle)
            };

            let matched = match_open_request(&agent, &[case.request]);

            assert_eq!(matched, case.matched, "{}", case.name);
        }
    }

    /// After a handoff, the old session on a card closes and the newest
    /// stays. Reading the order backwards, or counting a session the
    /// card never listed, would close the session that does the work.
    #[test]
    fn has_newer_live_session_needs_later_id() {
        let note = CardNote {
            sessions: vec![
                "ses_plan".to_owned(),
                "ses_old".to_owned(),
                "ses_new".to_owned(),
            ],
            ..CardNote::default()
        };
        struct Case {
            name: &'static str,
            session_id: &'static str,
            live: &'static [&'static str],
            newer: bool,
        }
        let cases = [
            Case {
                name: "newer-live",
                session_id: "ses_old",
                live: &["ses_old", "ses_new"],
                newer: true,
            },
            Case {
                name: "only-older-live",
                session_id: "ses_new",
                live: &["ses_plan", "ses_new"],
                newer: false,
            },
            Case {
                name: "newer-gone",
                session_id: "ses_old",
                live: &["ses_old"],
                newer: false,
            },
            Case {
                name: "not-listed",
                session_id: "ses_other",
                live: &["ses_other", "ses_new"],
                newer: false,
            },
        ];

        for case in cases {
            let live_ids = case.live.iter().map(|id| (*id).to_owned()).collect();

            let newer = has_newer_live_session(&note, Some(case.session_id), &live_ids);

            assert_eq!(newer, case.newer, "{}", case.name);
        }
    }
}
