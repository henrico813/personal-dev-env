//! Runs the tidy pass, which closes and parks finished sessions. Read
//! `run_tidy_pass` first. `plan_tidy_actions` is pure and holds every
//! rule about which tabs may close.

use std::collections::{BTreeMap, BTreeSet};

use jiff::Zoned;

use super::events::{self, Event, EventKind};
use super::facts::CollectedFacts;
use super::lifecycle::{Lifecycle, SessionFacts, Verdict, decide_lifecycle};
use super::parking::{self, ParkOutcome, TidyLock};
use super::run_folder::RunFolder;
use super::{HubError, Tools};
use crate::external::cards::{CardKey, Stage};
use crate::external::herdr::{Herdr, Pane};
use crate::external::processes;

/// The problem lines that leave facts out. A failed `card show` can
/// hide a session's card. A request file that does not parse can hide
/// a waiting approval. The pass changes nothing while either problem
/// shows.
const UNSAFE_SOURCES: [&str; 2] = ["cards:", "requests:"];

/// The reason a finished session stays when another pane in its tab
/// would close with it.
pub(crate) const TAB_HOLDS_WORK: &str = "its tab holds other work";

/// One thing the tidy pass will do. It borrows each session's facts
/// from the collected facts, so no facts are copied.
pub(crate) enum TidyAction<'a> {
    Park {
        facts: &'a SessionFacts,
        card: CardKey,
    },
    /// Closes a whole tab. Every agent in it is `to-close`, and every
    /// other pane is a shell that runs nothing.
    Close {
        tab_id: String,
        sessions: Vec<&'a SessionFacts>,
        why: String,
    },
    /// Deletes a parked record whose card is done or whose session is
    /// live again.
    Forget { name: String },
}

/// What one action did, worded as `hub clean` prints it.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum TidyOutcome {
    Closed {
        names: Vec<String>,
        why: String,
    },
    Parked {
        name: String,
        card: CardKey,
    },
    Forgot {
        name: String,
    },
    Kept {
        name: String,
        reason: String,
    },
    /// The pass changed nothing while this problem line shows.
    Stopped {
        problem: String,
    },
}

impl TidyOutcome {
    /// Returns the line `hub clean` prints, such as
    /// `parked demo-kb-007-impl: DEMO-KB-007 review`. A dry run says
    /// `would close`, `would park` and `would forget`.
    pub(crate) fn line(&self, dry_run: bool) -> String {
        let pick = |done: &str, planned: &str| if dry_run { planned } else { done }.to_owned();
        match self {
            TidyOutcome::Closed { names, why } => {
                format!(
                    "{} {}: {why}",
                    pick("closed", "would close"),
                    names.join(", ")
                )
            }
            TidyOutcome::Parked { name, card } => format!(
                "{} {name}: {} review",
                pick("parked", "would park"),
                card.as_str()
            ),
            TidyOutcome::Forgot { name } => format!("{} {name}", pick("forgot", "would forget")),
            TidyOutcome::Kept { name, reason } => format!("kept {name}: {reason}"),
            TidyOutcome::Stopped { problem } => format!("kept every session: {problem}"),
        }
    }

    /// Tells whether the outcome closed, parked or forgot something, or
    /// would in a dry run.
    pub(crate) fn is_change(&self) -> bool {
        matches!(
            self,
            TidyOutcome::Closed { .. } | TidyOutcome::Parked { .. } | TidyOutcome::Forgot { .. }
        )
    }
}

/// Closes, parks and forgets what the collected facts allow, one
/// action at a time, and adds a `Kept` outcome for each kept session.
/// One failed action becomes a `Kept` outcome, and the pass keeps
/// going. So one bad card call does not leave every other finished
/// session open.
/// With `dry_run`, it only plans. The caller holds the tidy lock from
/// before it collected the facts.
///
/// # Errors
///
/// When Herdr cannot list the panes.
pub(crate) fn run_tidy_pass(
    tools: &Tools<'_>,
    run: &RunFolder,
    collected: &CollectedFacts,
    _lock: &TidyLock,
    now: &Zoned,
    dry_run: bool,
) -> Result<Vec<TidyOutcome>, HubError> {
    if let Some(problem) = find_unsafe_problem(collected) {
        return Ok(vec![TidyOutcome::Stopped {
            problem: problem.to_owned(),
        }]);
    }
    let judged: Vec<(&SessionFacts, Verdict)> = collected
        .sessions
        .iter()
        .map(|facts| (facts, decide_lifecycle(facts)))
        .collect();
    let closing_tabs: BTreeSet<&str> = judged
        .iter()
        .filter(|(_, verdict)| matches!(verdict.state, Lifecycle::ToClose | Lifecycle::ToPark))
        .filter_map(|(facts, _)| facts.agent.as_ref().map(|agent| agent.tab_id.as_str()))
        .collect();
    // Most passes close nothing, so Herdr's pane list is read only when
    // a tab may close.
    let panes = if closing_tabs.is_empty() {
        Vec::new()
    } else {
        tools.herdr.list_panes()?
    };
    let quiet_shells = find_quiet_shells(tools.herdr, &panes, &closing_tabs);
    // A live session's old record goes before any park, so a park in
    // this pass saves its new record after the delete.
    let mut actions: Vec<TidyAction<'_>> = collected
        .live_records
        .iter()
        .map(|name| TidyAction::Forget { name: name.clone() })
        .collect();
    actions.extend(plan_tidy_actions(
        &judged,
        &panes,
        tools.own_pane,
        &quiet_shells,
    ));

    let mut outcomes: Vec<TidyOutcome> = actions
        .iter()
        .flat_map(|action| {
            if dry_run {
                vec![describe_planned_action(action)]
            } else {
                apply_tidy_action(tools, run, action, &panes, now)
            }
        })
        .collect();
    for (facts, verdict) in &judged {
        if let Lifecycle::Kept(reason) = verdict.state {
            outcomes.push(TidyOutcome::Kept {
                name: facts.name.clone(),
                reason: reason.label().to_owned(),
            });
        }
    }
    Ok(outcomes)
}

/// Returns the first problem line that stops the tidy pass, if any.
pub(crate) fn find_unsafe_problem(collected: &CollectedFacts) -> Option<&str> {
    collected
        .problems
        .iter()
        .find(|line| UNSAFE_SOURCES.iter().any(|source| line.starts_with(source)))
        .map(String::as_str)
}

/// Groups the sessions by tab and returns what may change. A whole tab
/// stays when any pane in it is the hub's own pane, the focused pane,
/// an agent that is not finished or has no name, or a shell that runs
/// something. A tab of `to-close` agents closes. A tab whose one agent
/// is `to-park` parks. A parked record whose state is `to-close` is
/// forgotten, since its card is done.
pub(crate) fn plan_tidy_actions<'a>(
    judged: &[(&'a SessionFacts, Verdict)],
    panes: &[Pane],
    own_pane: Option<&str>,
    quiet_shells: &BTreeSet<String>,
) -> Vec<TidyAction<'a>> {
    let by_pane: BTreeMap<&str, (&'a SessionFacts, &Verdict)> = judged
        .iter()
        .filter_map(|(facts, verdict)| {
            let agent = facts.agent.as_ref()?;
            Some((agent.pane_id.as_str(), (*facts, verdict)))
        })
        .collect();
    let mut tabs: BTreeMap<&str, Vec<&Pane>> = BTreeMap::new();
    for pane in panes {
        tabs.entry(pane.tab_id.as_str()).or_default().push(pane);
    }

    let mut actions = Vec::new();
    for (tab_id, tab_panes) in tabs {
        let mut finished = Vec::new();
        let mut keeps_tab = false;
        for pane in tab_panes {
            let found = by_pane.get(pane.pane_id.as_str());
            if own_pane == Some(pane.pane_id.as_str()) || pane.focused {
                keeps_tab = true;
            } else if let Some((facts, verdict)) = found {
                if matches!(verdict.state, Lifecycle::ToClose | Lifecycle::ToPark) {
                    finished.push((*facts, *verdict));
                } else {
                    keeps_tab = true;
                }
            } else if pane.has_agent || !quiet_shells.contains(&pane.pane_id) {
                keeps_tab = true;
            }
        }
        if keeps_tab || finished.is_empty() {
            continue;
        }
        if let [(facts, verdict)] = finished.as_slice()
            && verdict.state == Lifecycle::ToPark
        {
            if let Some(card) = &facts.card {
                actions.push(TidyAction::Park {
                    facts,
                    card: card.key.clone(),
                });
            }
        } else if finished
            .iter()
            .all(|(_, verdict)| verdict.state == Lifecycle::ToClose)
        {
            let mut whys: Vec<String> = finished
                .iter()
                .map(|(facts, verdict)| describe_close_why(facts, verdict))
                .collect();
            whys.dedup();
            actions.push(TidyAction::Close {
                tab_id: tab_id.to_owned(),
                sessions: finished.iter().map(|(facts, _)| *facts).collect(),
                why: whys.join(", "),
            });
        }
        // A `to-park` agent beside another agent stays, since closing the
        // tab would close the other one without its record or line.
    }
    for (facts, verdict) in judged {
        if facts.agent.is_none() && verdict.state == Lifecycle::ToClose {
            actions.push(TidyAction::Forget {
                name: facts.name.clone(),
            });
        }
    }
    actions
}

/// Carries out one action after working out each session's state
/// again, with `panes` as the pass read them. Any failure becomes a
/// `Kept` outcome with its reason.
fn apply_tidy_action(
    tools: &Tools<'_>,
    run: &RunFolder,
    action: &TidyAction<'_>,
    panes: &[Pane],
    now: &Zoned,
) -> Vec<TidyOutcome> {
    let kept = |name: &str, reason: String| TidyOutcome::Kept {
        name: name.to_owned(),
        reason,
    };
    match action {
        TidyAction::Park { facts, card } => {
            let outcome = match parking::park_session(tools, run, facts, panes, now) {
                Ok(ParkOutcome::Parked(_)) => TidyOutcome::Parked {
                    name: facts.name.clone(),
                    card: card.clone(),
                },
                Ok(ParkOutcome::Stays(verdict)) => {
                    kept(&facts.name, verdict.why_or_state().to_owned())
                }
                Ok(ParkOutcome::TabInUse) => kept(&facts.name, TAB_HOLDS_WORK.to_owned()),
                Err(error) => kept(&facts.name, error.to_string()),
            };
            vec![outcome]
        }
        TidyAction::Close {
            tab_id,
            sessions,
            why,
        } => close_finished_tab(tools, run, tab_id, sessions, why, panes, now).unwrap_or_else(
            |error| {
                sessions
                    .iter()
                    .map(|facts| kept(&facts.name, error.to_string()))
                    .collect()
            },
        ),
        TidyAction::Forget { name } => match tools.parked.delete_parked_record(name) {
            Ok(()) => vec![TidyOutcome::Forgot { name: name.clone() }],
            Err(error) => vec![kept(name, error.to_string())],
        },
    }
}

/// Closes a tab once every session in it is still `to-close` and its
/// panes still lose nothing, then logs `closed` and one `notes.md` line
/// for each. A kept tab gives one `Kept` outcome per session.
fn close_finished_tab(
    tools: &Tools<'_>,
    run: &RunFolder,
    tab_id: &str,
    sessions: &[&SessionFacts],
    why: &str,
    planned: &[Pane],
    now: &Zoned,
) -> Result<Vec<TidyOutcome>, HubError> {
    let mut kept_whys = Vec::new();
    for facts in sessions {
        let (_, verdict) = parking::decide_fresh_lifecycle(tools, run, facts)?;
        let kept_why =
            (verdict.state != Lifecycle::ToClose).then(|| verdict.why_or_state().to_owned());
        kept_whys.push(kept_why);
    }
    let is_free =
        kept_whys.iter().all(Option::is_none) && is_tab_still_free(tools, tab_id, planned)?;
    if !is_free {
        // A session still `to-close` stays only because its tab does.
        let kept = sessions
            .iter()
            .zip(kept_whys)
            .map(|(facts, kept_why)| TidyOutcome::Kept {
                name: facts.name.clone(),
                reason: kept_why.unwrap_or_else(|| TAB_HOLDS_WORK.to_owned()),
            });
        return Ok(kept.collect());
    }
    tools.herdr.close_tab(tab_id)?;

    let names: Vec<String> = sessions.iter().map(|facts| facts.name.clone()).collect();
    let closed_but = |error: HubError| HubError::PartlyDone {
        done: format!("closed {}", names.join(", ")),
        error: Box::new(error),
    };
    for name in &names {
        let closed = Event {
            at: now.timestamp(),
            kind: EventKind::Closed {
                name: name.clone(),
                tab: tab_id.to_owned(),
                why: why.to_owned(),
            },
        };
        events::append_event(run, &closed).map_err(closed_but)?;
        run.append_note_line(&format!("closed {name} ({why})"), now)
            .map_err(closed_but)?;
    }
    Ok(vec![TidyOutcome::Closed {
        names,
        why: why.to_owned(),
    }])
}

/// Reads one tab's panes and the plain shells among them that run
/// nothing.
///
/// # Errors
///
/// When Herdr cannot list the panes.
pub(crate) fn read_tab_panes(
    tools: &Tools<'_>,
    tab_id: &str,
) -> Result<(Vec<Pane>, BTreeSet<String>), HubError> {
    let panes: Vec<Pane> = tools
        .herdr
        .list_panes()?
        .into_iter()
        .filter(|pane| pane.tab_id == tab_id)
        .collect();
    let quiet_shells = find_quiet_shells(tools.herdr, &panes, &BTreeSet::from([tab_id]));
    Ok((panes, quiet_shells))
}

/// Reads a tab's panes again just before it closes, and tells whether
/// closing it still loses nothing. That holds when the tab has the
/// same panes it had in `planned`, each agent pane is still an agent
/// and each shell still a shell, none is focused or the hub's own, and
/// each plain shell still runs nothing.
///
/// # Errors
///
/// When Herdr cannot list the panes.
pub(crate) fn is_tab_still_free(
    tools: &Tools<'_>,
    tab_id: &str,
    planned: &[Pane],
) -> Result<bool, HubError> {
    let (panes, quiet_shells) = read_tab_panes(tools, tab_id)?;
    // A quiet shell where the user then started an agent changes its
    // pair, so the tab stays with the user's new session.
    let list_tab_panes = |list: &[Pane]| -> BTreeSet<(String, bool)> {
        list.iter()
            .filter(|pane| pane.tab_id == tab_id)
            .map(|pane| (pane.pane_id.clone(), pane.has_agent))
            .collect()
    };
    if list_tab_panes(&panes) != list_tab_panes(planned) {
        return Ok(false);
    }
    Ok(panes.iter().all(|pane| {
        tools.own_pane != Some(pane.pane_id.as_str())
            && !pane.focused
            && (pane.has_agent || quiet_shells.contains(&pane.pane_id))
    }))
}

/// Returns the plain shell panes in `tabs` that run nothing.
fn find_quiet_shells(herdr: &dyn Herdr, panes: &[Pane], tabs: &BTreeSet<&str>) -> BTreeSet<String> {
    panes
        .iter()
        .filter(|pane| !pane.has_agent && tabs.contains(pane.tab_id.as_str()))
        .filter(|pane| is_shell_at_prompt(herdr, &pane.pane_id))
        .map(|pane| pane.pane_id.clone())
        .collect()
}

/// Returns the outcome an action would have, for a dry run.
fn describe_planned_action(action: &TidyAction<'_>) -> TidyOutcome {
    match action {
        TidyAction::Park { facts, card } => TidyOutcome::Parked {
            name: facts.name.clone(),
            card: card.clone(),
        },
        TidyAction::Close { sessions, why, .. } => TidyOutcome::Closed {
            names: sessions.iter().map(|facts| facts.name.clone()).collect(),
            why: why.clone(),
        },
        TidyAction::Forget { name } => TidyOutcome::Forgot { name: name.clone() },
    }
}

/// Returns why a session's tab closes, such as `DEMO-KB-007 accepted`
/// for a done card, else the verdict's why, such as `report read`.
pub(crate) fn describe_close_why(facts: &SessionFacts, verdict: &Verdict) -> String {
    match &facts.card {
        Some(card) if matches!(card.stage, Stage::Accepted | Stage::Dropped) => {
            format!("{} {}", card.key.as_str(), card.stage.as_str())
        }
        _ => verdict.why.clone(),
    }
}

/// Tells whether a plain shell pane runs nothing, so closing its tab
/// loses nothing. Any error means it cannot tell, and returns false.
fn is_shell_at_prompt(herdr: &dyn Herdr, pane_id: &str) -> bool {
    let Ok(info) = herdr.read_process_info(pane_id) else {
        return false;
    };
    // A program in front of the shell, such as an editor, owns the
    // terminal's process group.
    info.foreground_group == info.shell_pid
        && processes::has_shell_jobs(info.shell_pid).is_ok_and(|has_jobs| !has_jobs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::lifecycle::CardFacts;
    use crate::shared::test_values::{make_facts, make_parked_record};

    /// An unfocused pane of tab `tab_id`, holding an agent or a plain
    /// shell.
    fn make_pane(pane_id: &str, tab_id: &str, has_agent: bool) -> Pane {
        Pane {
            pane_id: pane_id.to_owned(),
            tab_id: tab_id.to_owned(),
            focused: false,
            has_agent,
        }
    }

    /// A finished worker in tab `tab_id` whose card DEMO-KB-001 is
    /// accepted, with its `to-close` verdict.
    fn make_closing_worker(name: &str, tab_id: &str) -> (SessionFacts, Verdict) {
        let mut facts = make_facts(name);
        if let Some(agent) = facts.agent.as_mut() {
            agent.tab_id = tab_id.to_owned();
        }
        facts.card = Some(CardFacts {
            key: "DEMO-KB-001".parse().unwrap(),
            stage: Stage::Accepted,
            needs_you_open: false,
            newer_session_live: false,
            from_report: false,
        });
        let verdict = Verdict {
            state: Lifecycle::ToClose,
            why: "card done".to_owned(),
        };
        (facts, verdict)
    }

    /// Returns each action as one short line, such as
    /// `park demo-kb-007-impl DEMO-KB-007`.
    fn describe_actions(actions: &[TidyAction<'_>]) -> Vec<String> {
        actions
            .iter()
            .map(|action| match action {
                TidyAction::Park { facts, card } => {
                    format!("park {} {}", facts.name, card.as_str())
                }
                TidyAction::Close {
                    tab_id,
                    sessions,
                    why,
                } => {
                    let names: Vec<&str> =
                        sessions.iter().map(|facts| facts.name.as_str()).collect();
                    format!("close {tab_id} {}: {why}", names.join(", "))
                }
                TidyAction::Forget { name } => format!("forget {name}"),
            })
            .collect()
    }

    /// Plans the actions for these sessions and panes.
    fn plan_for(
        sessions: &[(SessionFacts, Verdict)],
        panes: &[Pane],
        own_pane: Option<&str>,
        quiet: &[&str],
    ) -> Vec<String> {
        let judged: Vec<(&SessionFacts, Verdict)> = sessions
            .iter()
            .map(|(facts, verdict)| (facts, verdict.clone()))
            .collect();
        let quiet_shells = quiet.iter().map(|pane| (*pane).to_owned()).collect();
        describe_actions(&plan_tidy_actions(&judged, panes, own_pane, &quiet_shells))
    }

    /// Closing a tab closes every pane in it, so a tab stays whole
    /// when any pane in it would lose something. Closing the hub's own
    /// tab would end the hub mid-run. Closing the focused tab would close
    /// the one the user reads. A tab with a working neighbour would cut
    /// off its turn. A tab with a busy shell would lose its job. A lone
    /// shell tab is the user's own, so it stays too. Each row below
    /// checks one such case.
    #[test]
    fn plan_tidy_actions_keeps_tab_losing_something() {
        struct Case {
            name: &'static str,
            sessions: Vec<(SessionFacts, Verdict)>,
            panes: Vec<Pane>,
            own_pane: Option<&'static str>,
            quiet: &'static [&'static str],
        }
        let tab = "w1:t1";
        let worker = || make_closing_worker("demo-kb-001-impl", tab);
        let worker_pane = || make_pane("w1:demo-kb-001-impl", tab, true);
        let working_neighbour = {
            let (facts, _) = make_closing_worker("demo-kb-002-impl", tab);
            let verdict = Verdict {
                state: Lifecycle::Working,
                why: String::new(),
            };
            (facts, verdict)
        };
        let cases = [
            Case {
                name: "own-pane",
                sessions: vec![worker()],
                panes: vec![worker_pane(), make_pane("w1:p-hub", tab, false)],
                own_pane: Some("w1:p-hub"),
                quiet: &["w1:p-hub"],
            },
            Case {
                name: "focused-pane",
                sessions: vec![worker()],
                panes: vec![
                    worker_pane(),
                    Pane {
                        focused: true,
                        ..make_pane("w1:p8", tab, false)
                    },
                ],
                own_pane: None,
                quiet: &["w1:p8"],
            },
            Case {
                name: "working-neighbour",
                sessions: vec![worker(), working_neighbour],
                panes: vec![worker_pane(), make_pane("w1:demo-kb-002-impl", tab, true)],
                own_pane: None,
                quiet: &[],
            },
            Case {
                name: "busy-shell",
                sessions: vec![worker()],
                panes: vec![worker_pane(), make_pane("w1:p8", tab, false)],
                own_pane: None,
                quiet: &[],
            },
            Case {
                name: "lone-shell",
                sessions: Vec::new(),
                panes: vec![make_pane("w1:p9", "w1:t9", false)],
                own_pane: None,
                quiet: &["w1:p9"],
            },
        ];

        for case in cases {
            let actions = plan_for(&case.sessions, &case.panes, case.own_pane, case.quiet);

            assert!(actions.is_empty(), "{}: {actions:?}", case.name);
        }
    }

    /// A review worker must park, so its session can reopen when review
    /// comments come. A done card's worker must close instead. Swapping the
    /// two closes reviewed work with no way back, or keeps a record no
    /// card needs. A parked record whose card is done is forgotten.
    #[test]
    fn plan_tidy_actions_parks_review_closes_accepted() {
        let review = (
            make_facts("demo-kb-007-impl"),
            Verdict {
                state: Lifecycle::ToPark,
                why: String::new(),
            },
        );
        let accepted = make_closing_worker("demo-kb-001-impl", "w1:t1");
        let parked_done = {
            let (mut facts, verdict) = make_closing_worker("demo-kb-003-auto", "w1:t3");
            facts.agent = None;
            facts.parked = Some(make_parked_record("demo-kb-003-auto"));
            (facts, verdict)
        };
        let panes = [
            make_pane("w1:demo-kb-007-impl", "w1:t-demo-kb-007-impl", true),
            make_pane("w1:demo-kb-001-impl", "w1:t1", true),
        ];

        let actions = plan_for(&[review, accepted, parked_done], &panes, None, &[]);

        assert_eq!(
            actions,
            [
                "park demo-kb-007-impl DEMO-KB-007",
                "close w1:t1 demo-kb-001-impl: DEMO-KB-001 accepted",
                "forget demo-kb-003-auto",
            ]
        );
    }

    /// Most workers have a plain shell beside the agent. If a shell at
    /// its prompt kept the whole tab, no finished worker's tab would
    /// ever close, and the pile of open tabs would come back.
    #[test]
    fn plan_tidy_actions_closes_shell_with_agent() {
        let worker = make_closing_worker("demo-kb-001-impl", "w1:t1");
        let panes = [
            make_pane("w1:demo-kb-001-impl", "w1:t1", true),
            make_pane("w1:p8", "w1:t1", false),
        ];

        let actions = plan_for(&[worker], &panes, None, &["w1:p8"]);

        assert_eq!(
            actions,
            ["close w1:t1 demo-kb-001-impl: DEMO-KB-001 accepted"]
        );
    }
}
