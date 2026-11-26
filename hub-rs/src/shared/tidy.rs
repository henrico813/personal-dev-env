//! Runs the tidy pass, which closes and parks finished sessions. Read
//! `run_tidy_pass` first. `plan_tidy_actions` is pure and holds every
//! rule about which tabs may close, and `find_retiring_workspaces`
//! picks the workspaces of a finished auto run whose every tab the
//! pass judges.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use jiff::Zoned;

use super::events::{self, Event, EventKind, Memory};
use super::facts::CollectedFacts;
use super::lifecycle::{Lifecycle, SessionFacts, Verdict, decide_lifecycle, is_hub_name};
use super::parking::{self, ParkOutcome, TidyLock};
use super::run_folder::RunFolder;
use super::{HubError, Tools};
use crate::external::cards::{CardKey, Stage};
use crate::external::herdr::{Herdr, Pane, Workspace};
use crate::external::processes;

/// The problem lines that leave facts out. A failed card read can
/// hide a session's card. A request file that does not parse can hide
/// a waiting approval. The pass changes nothing while either problem
/// shows.
const UNSAFE_SOURCES: [&str; 2] = ["cards:", "requests:"];

/// The reason a finished session stays when another pane in its tab
/// would close with it.
pub(crate) const TAB_HOLDS_WORK: &str = "its tab holds other work";

/// Why a retiring workspace's shell tab or the workspace itself closed.
const RUN_OVER: &str = "run over";

// Why a pane keeps a retiring workspace's tab open.
const SHELL_IN_USE: &str = "shell in use";
const UNNAMED_AGENT: &str = "agent with no name";
const PANE_FOCUSED: &str = "focused";

/// Why no workspace of an adopted auto run retires.
const OWN_PANE_UNKNOWN: &str = "this hub's pane is unknown";

/// The label of the user's desk workspace, as `hub desk` makes it. The
/// desk never retires, even when an agent of the run was moved there.
const DESK_LABEL: &str = "desk";

/// A workspace of a finished auto run, with the tabs Herdr listed,
/// whose every tab the tidy pass judges.
pub(crate) struct RetiringWorkspace {
    pub workspace: Workspace,
    pub tabs: BTreeSet<String>,
}

/// What one tidy pass may change. Only a tab that holds a session in
/// `names`, or a tab of a `retiring` workspace, may close or park.
pub(crate) struct TidyScope {
    pub names: BTreeSet<String>,
    pub retiring: Vec<RetiringWorkspace>,
}

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
    /// Deletes a parked record whose card's work is over or whose
    /// session is live again.
    Forget { name: String },
    /// Leaves a `to-close` or `to-park` session open because closing
    /// its tab would close another pane's work too.
    Keep { facts: &'a SessionFacts },
    /// Closes a retiring workspace's tab that holds only shells that
    /// run nothing. `names` holds `pane PANE-ID` for each shell.
    CloseShells { tab_id: String, names: Vec<String> },
    /// Names a pane that keeps a retiring workspace's tab open, such as
    /// a shell that runs something.
    KeepPane { name: String, reason: &'static str },
}

/// What one action did, worded as `run_clean` prints it.
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
    /// Returns the line `run_clean` prints, such as
    /// `parked demo-kb-007-impl: DEMO-KB-007 in-review`. A dry run says
    /// `would close`, `would park` and `would forget`.
    pub(crate) fn format_line(&self, dry_run: bool) -> String {
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
                "{} {name}: {} in-review",
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
/// action at a time, and adds a `Kept` outcome for each kept session
/// in `scope.names`. Only tabs that hold a session in `scope.names`,
/// or tabs of a retiring workspace, close or park, as
/// `plan_tidy_actions` says. Each agent in a retiring workspace that
/// is not finished gets a `Kept` outcome too, and a retiring workspace
/// whose every tab closed gets a `Closed` outcome. A live session's
/// old parked record is deleted whatever its run.
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
    scope: &TidyScope,
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
    let retiring_tabs: BTreeSet<&str> = scope
        .retiring
        .iter()
        .flat_map(|retiring| retiring.tabs.iter().map(String::as_str))
        .collect();
    let mut closing_tabs: BTreeSet<&str> = judged
        .iter()
        .filter(|(facts, verdict)| {
            scope.names.contains(&facts.name)
                && matches!(verdict.state, Lifecycle::ToClose | Lifecycle::ToPark)
        })
        .filter_map(|(facts, _)| facts.agent.as_ref().map(|agent| agent.tab_id.as_str()))
        .collect();
    closing_tabs.extend(retiring_tabs.iter().copied());
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
        &scope.names,
        &retiring_tabs,
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
        let in_retiring_tab = facts
            .agent
            .as_ref()
            .is_some_and(|agent| retiring_tabs.contains(agent.tab_id.as_str()));
        let reason = match verdict.state {
            Lifecycle::Kept(reason) if in_retiring_tab || scope.names.contains(&facts.name) => {
                reason.label()
            }
            Lifecycle::Working | Lifecycle::WaitsOnHub | Lifecycle::WaitsOnUser
                if in_retiring_tab =>
            {
                verdict.pick_why_or_state()
            }
            Lifecycle::Kept(_)
            | Lifecycle::Working
            | Lifecycle::WaitsOnHub
            | Lifecycle::WaitsOnUser
            | Lifecycle::ToClose
            | Lifecycle::ToPark
            | Lifecycle::Parked => continue,
        };
        outcomes.push(TidyOutcome::Kept {
            name: facts.name.clone(),
            reason: reason.to_owned(),
        });
    }
    outcomes.extend(list_closed_workspaces(
        tools,
        &scope.retiring,
        &actions,
        dry_run,
    ));
    Ok(outcomes)
}

/// Returns the workspaces whose every tab `run_tidy_pass` may close.
/// Each holds an adopted auto hub that is `to-close`, as
/// `memory.auto_runs` names it, or a live session that hub's run
/// started, as its `notes.md` says. The desk and a workspace with this
/// hub's pane or another open hub are left out. When it cannot tell
/// which workspaces those are, none retire, and each such hub gets a
/// `kept workspaces of HUB` outcome that says why.
pub(crate) fn find_retiring_workspaces(
    tools: &Tools<'_>,
    collected: &CollectedFacts,
    memory: &Memory,
) -> (Vec<RetiringWorkspace>, Vec<TidyOutcome>) {
    let old_hubs: Vec<(&SessionFacts, &Path)> = collected
        .sessions
        .iter()
        .filter_map(|facts| Some((facts, memory.auto_runs.get(&facts.name)?.as_path())))
        .filter(|(facts, _)| decide_lifecycle(facts).state == Lifecycle::ToClose)
        .collect();
    if old_hubs.is_empty() {
        return (Vec::new(), Vec::new());
    }
    match list_auto_run_workspaces(tools, collected, &old_hubs) {
        Ok(retiring) => (retiring, Vec::new()),
        Err(why) => {
            let kept = old_hubs.iter().map(|(facts, _)| TidyOutcome::Kept {
                name: format!("workspaces of {}", facts.name),
                reason: why.clone(),
            });
            (Vec::new(), kept.collect())
        }
    }
}

/// Returns the workspaces of `old_hubs` and of the live sessions their
/// runs started, or why none may retire. A workspace is left out when
/// its tabs hold this hub's own pane, when it holds a hub that stays
/// open, or when it is the user's desk, since its other tabs are not
/// the run's.
fn list_auto_run_workspaces(
    tools: &Tools<'_>,
    collected: &CollectedFacts,
    old_hubs: &[(&SessionFacts, &Path)],
) -> Result<Vec<RetiringWorkspace>, String> {
    let panes = tools
        .herdr
        .list_panes()
        .map_err(|error| error.to_string())?;
    let own_tab = tools
        .own_pane
        .and_then(|own_pane| panes.into_iter().find(|pane| pane.pane_id == own_pane));
    // Without this hub's own tab, its workspace cannot be told apart.
    let Some(own_tab) = own_tab.map(|pane| pane.tab_id) else {
        return Err(OWN_PANE_UNKNOWN.to_owned());
    };
    let mut started = BTreeSet::new();
    for (_, path) in old_hubs {
        let run = RunFolder::open(path).map_err(|error| error.to_string())?;
        started.extend(
            run.list_started_names()
                .map_err(|error| error.to_string())?,
        );
    }
    let run_workspaces: BTreeSet<&str> = collected
        .sessions
        .iter()
        .filter(|facts| {
            started.contains(&facts.name) || old_hubs.iter().any(|(hub, _)| hub.name == facts.name)
        })
        .filter_map(|facts| facts.agent.as_ref())
        .map(|agent| agent.workspace_id.as_str())
        .collect();
    let open_hub_workspaces: BTreeSet<&str> = collected
        .sessions
        .iter()
        .filter(|facts| {
            is_hub_name(&facts.name) && decide_lifecycle(facts).state != Lifecycle::ToClose
        })
        .filter_map(|facts| facts.agent.as_ref())
        .map(|agent| agent.workspace_id.as_str())
        .collect();
    let mut retiring = Vec::new();
    for workspace in tools
        .herdr
        .list_workspaces()
        .map_err(|error| error.to_string())?
    {
        let id = workspace.workspace_id.as_str();
        if !run_workspaces.contains(id)
            || open_hub_workspaces.contains(id)
            || workspace.label == DESK_LABEL
        {
            continue;
        }
        let tabs: BTreeSet<String> = tools
            .herdr
            .list_tabs(id)
            .map_err(|error| error.to_string())?
            .into_iter()
            .collect();
        if !tabs.contains(&own_tab) {
            retiring.push(RetiringWorkspace { workspace, tabs });
        }
    }
    Ok(retiring)
}

/// Returns a `Closed` outcome for each retiring workspace whose every
/// tab the actions close or park, as Herdr drops a workspace with its
/// last tab. A real run gives it only when Herdr no longer lists the
/// workspace, and a failed list gives none.
fn list_closed_workspaces(
    tools: &Tools<'_>,
    retiring: &[RetiringWorkspace],
    actions: &[TidyAction<'_>],
    dry_run: bool,
) -> Vec<TidyOutcome> {
    let planned_tabs: BTreeSet<&str> = actions
        .iter()
        .filter_map(|action| match action {
            TidyAction::Close { tab_id, .. } | TidyAction::CloseShells { tab_id, .. } => {
                Some(tab_id.as_str())
            }
            TidyAction::Park { facts, .. } => {
                facts.agent.as_ref().map(|agent| agent.tab_id.as_str())
            }
            TidyAction::Forget { .. } | TidyAction::Keep { .. } | TidyAction::KeepPane { .. } => {
                None
            }
        })
        .collect();
    let emptied: Vec<&Workspace> = retiring
        .iter()
        .filter(|retiring| {
            retiring
                .tabs
                .iter()
                .all(|tab| planned_tabs.contains(tab.as_str()))
        })
        .map(|retiring| &retiring.workspace)
        .collect();
    if emptied.is_empty() {
        return Vec::new();
    }
    let listed: BTreeSet<String> = if dry_run {
        BTreeSet::new()
    } else {
        match tools.herdr.list_workspaces() {
            Ok(workspaces) => workspaces
                .into_iter()
                .map(|workspace| workspace.workspace_id)
                .collect(),
            Err(_) => return Vec::new(),
        }
    };
    emptied
        .into_iter()
        .filter(|workspace| !listed.contains(&workspace.workspace_id))
        .map(|workspace| TidyOutcome::Closed {
            names: vec![format!("workspace {}", workspace.label)],
            why: RUN_OVER.to_owned(),
        })
        .collect()
}

/// Returns the first problem line that stops the tidy pass, if any.
pub(crate) fn find_unsafe_problem(collected: &CollectedFacts) -> Option<&str> {
    collected
        .problems
        .iter()
        .find(|line| UNSAFE_SOURCES.iter().any(|source| line.starts_with(source)))
        .map(String::as_str)
}

/// Returns the names of the sessions this run knows, the scope of
/// `run_clean`'s tidy pass. Only tabs that hold one of them may change.
/// Those are each session
/// `run_launch` started from this run, each name with a report here,
/// read or not, and each old hub a `handoff` event of this run named.
///
/// # Errors
///
/// When the run folder or its `notes.md` cannot be read.
pub(crate) fn list_run_session_names(
    run: &RunFolder,
    memory: &Memory,
) -> Result<BTreeSet<String>, HubError> {
    let mut names = run.list_started_names()?;
    names.extend(run.list_reported_names()?.any);
    names.extend(memory.handed_off.iter().cloned());
    Ok(names)
}

/// Groups the sessions by tab and returns what may change. Only a tab
/// that holds a session in `scope`, or one in `retiring_tabs`, may
/// change, and each agent in it
/// counts by its own verdict, so a finished `to-close` agent of
/// another run closes with the tab. A whole tab stays when any pane in
/// it is the hub's own pane, the focused pane, an agent that is not
/// finished or has no name, or a shell that runs something. A tab of
/// `to-close` agents closes. A tab whose one agent
/// is `to-park` parks. Each finished session of `scope` in a tab that
/// stays is kept, so `run_clean` says why. A parked record of `scope`
/// whose state is `to-close` is forgotten, since its card's work is
/// over.
/// Each finished agent in a tab of `retiring_tabs` counts as one of
/// `scope`. When such a tab stays, each pane that keeps it gets a
/// `KeepPane` action, except an agent that is not finished. When it
/// holds only shells that run nothing, it closes.
pub(crate) fn plan_tidy_actions<'a>(
    judged: &[(&'a SessionFacts, Verdict)],
    scope: &BTreeSet<String>,
    retiring_tabs: &BTreeSet<&str>,
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
        let retiring = retiring_tabs.contains(tab_id);
        let holds_scope = retiring
            || tab_panes.iter().any(|pane| {
                by_pane
                    .get(pane.pane_id.as_str())
                    .is_some_and(|(facts, _)| scope.contains(&facts.name))
            });
        if !holds_scope {
            continue;
        }
        let mut finished = Vec::new();
        let mut keeps_tab = false;
        // Each pane that keeps the tab and why, said only for a retiring tab.
        let mut keeping: Vec<(String, &'static str)> = Vec::new();
        for pane in &tab_panes {
            let found = by_pane.get(pane.pane_id.as_str());
            let name_pane = || format!("pane {}", pane.pane_id);
            if own_pane == Some(pane.pane_id.as_str()) || pane.focused {
                keeps_tab = true;
                match found {
                    // `run_tidy_pass` gives an agent that is not finished its
                    // own `kept` line.
                    Some((_, verdict))
                        if !matches!(verdict.state, Lifecycle::ToClose | Lifecycle::ToPark) => {}
                    Some((facts, _)) => keeping.push((facts.name.clone(), PANE_FOCUSED)),
                    None => keeping.push((name_pane(), PANE_FOCUSED)),
                }
            } else if let Some((facts, verdict)) = found {
                if matches!(verdict.state, Lifecycle::ToClose | Lifecycle::ToPark) {
                    finished.push((*facts, *verdict));
                } else {
                    keeps_tab = true;
                }
            } else if pane.has_agent {
                keeps_tab = true;
                keeping.push((name_pane(), UNNAMED_AGENT));
            } else if !quiet_shells.contains(&pane.pane_id) {
                keeps_tab = true;
                keeping.push((name_pane(), SHELL_IN_USE));
            }
        }
        if retiring {
            let keep_panes = keeping
                .into_iter()
                .map(|(name, reason)| TidyAction::KeepPane { name, reason });
            actions.extend(keep_panes);
        }
        if finished.is_empty() {
            if retiring && !keeps_tab {
                actions.push(TidyAction::CloseShells {
                    tab_id: tab_id.to_owned(),
                    names: tab_panes
                        .iter()
                        .map(|pane| format!("pane {}", pane.pane_id))
                        .collect(),
                });
            }
            continue;
        }
        let keep_finished = |actions: &mut Vec<TidyAction<'a>>| {
            for (facts, _) in &finished {
                if retiring || scope.contains(&facts.name) {
                    actions.push(TidyAction::Keep { facts });
                }
            }
        };
        if keeps_tab {
            keep_finished(&mut actions);
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
        } else {
            // A `to-park` agent beside another agent stays, since closing
            // the tab would close the other one without its record or line.
            keep_finished(&mut actions);
        }
    }
    for (facts, verdict) in judged {
        if facts.agent.is_none()
            && verdict.state == Lifecycle::ToClose
            && scope.contains(&facts.name)
        {
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
    let make_kept_outcome = |name: &str, reason: String| TidyOutcome::Kept {
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
                    make_kept_outcome(&facts.name, verdict.pick_why_or_state().to_owned())
                }
                Ok(ParkOutcome::TabInUse) => {
                    make_kept_outcome(&facts.name, TAB_HOLDS_WORK.to_owned())
                }
                Err(error) => make_kept_outcome(&facts.name, error.to_string()),
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
                    .map(|facts| make_kept_outcome(&facts.name, error.to_string()))
                    .collect()
            },
        ),
        TidyAction::Forget { name } => match tools.parked.delete_parked_record(name) {
            Ok(()) => vec![TidyOutcome::Forgot { name: name.clone() }],
            Err(error) => vec![make_kept_outcome(name, error.to_string())],
        },
        TidyAction::CloseShells { tab_id, names } => match close_shell_tab(tools, tab_id, panes) {
            Ok(true) => vec![describe_planned_action(action)],
            Ok(false) => names
                .iter()
                .map(|name| make_kept_outcome(name, TAB_HOLDS_WORK.to_owned()))
                .collect(),
            Err(error) => names
                .iter()
                .map(|name| make_kept_outcome(name, error.to_string()))
                .collect(),
        },
        TidyAction::Keep { .. } | TidyAction::KeepPane { .. } => {
            vec![describe_planned_action(action)]
        }
    }
}

/// Closes a retiring workspace's tab of shells when its panes still
/// match the plan and run nothing, and tells whether it closed. It logs
/// no event or `notes.md` line, since no session closed.
fn close_shell_tab(tools: &Tools<'_>, tab_id: &str, planned: &[Pane]) -> Result<bool, HubError> {
    if !is_tab_still_free(tools, tab_id, planned)? {
        return Ok(false);
    }
    tools.herdr.close_tab(tab_id)?;
    Ok(true)
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
            (verdict.state != Lifecycle::ToClose).then(|| verdict.pick_why_or_state().to_owned());
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
    let make_closed_but_error = |error: HubError| HubError::PartlyDone {
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
        events::append_event(run, &closed).map_err(make_closed_but_error)?;
        run.append_note_line(&format!("closed {name} ({why})"), now)
            .map_err(make_closed_but_error)?;
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
    // A shell where the user has since started an agent now counts
    // as an agent pane, so the tab no longer matches the plan and
    // stays.
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
        TidyAction::Keep { facts } => TidyOutcome::Kept {
            name: facts.name.clone(),
            reason: TAB_HOLDS_WORK.to_owned(),
        },
        TidyAction::CloseShells { names, .. } => TidyOutcome::Closed {
            names: names.clone(),
            why: RUN_OVER.to_owned(),
        },
        TidyAction::KeepPane { name, reason } => TidyOutcome::Kept {
            name: name.clone(),
            reason: (*reason).to_owned(),
        },
    }
}

/// Returns why a session's tab closes, such as `DEMO-KB-007 done`
/// for a card whose work is over, else the verdict's why, such as
/// `report read`.
pub(crate) fn describe_close_why(facts: &SessionFacts, verdict: &Verdict) -> String {
    match &facts.card {
        Some(card) if matches!(card.stage, Stage::Done | Stage::Dropped) => {
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
    /// closed, with its `to-close` verdict.
    fn make_closing_worker(name: &str, tab_id: &str) -> (SessionFacts, Verdict) {
        let mut facts = make_facts(name);
        if let Some(agent) = facts.agent.as_mut() {
            agent.tab_id = tab_id.to_owned();
        }
        facts.card = Some(CardFacts {
            key: "DEMO-KB-001".parse().unwrap(),
            stage: Stage::Done,
            has_open_question: false,
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
                TidyAction::Keep { facts } => format!("keep {}", facts.name),
                TidyAction::CloseShells { tab_id, names } => {
                    format!("close shells {tab_id} {}", names.join(", "))
                }
                TidyAction::KeepPane { name, reason } => format!("keep {name}: {reason}"),
            })
            .collect()
    }

    /// Plans the actions for these sessions and panes, with every
    /// session in the scope.
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
        let scope = sessions
            .iter()
            .map(|(facts, _)| facts.name.clone())
            .collect();
        let quiet_shells = quiet.iter().map(|pane| (*pane).to_owned()).collect();
        describe_actions(&plan_tidy_actions(
            &judged,
            &scope,
            &BTreeSet::new(),
            panes,
            own_pane,
            &quiet_shells,
        ))
    }

    /// Closing a tab closes every pane in it, so a tab stays whole
    /// when any pane in it would lose something. Closing the hub's own
    /// tab would end the hub mid-run. Closing the focused tab would close
    /// the one the user reads. A tab with a working neighbour would cut
    /// off its turn. A tab with a busy shell would lose its job. A lone
    /// shell tab is the user's own, so it stays too. A `to-park` worker
    /// beside a `to-close` one stays, or the close would end the parked
    /// session without its record. Each finished
    /// session in a kept tab is kept, so `run_clean` can say why. Each
    /// row below checks one such case.
    #[test]
    fn plan_tidy_actions_keeps_tab_losing_something() {
        struct Case {
            name: &'static str,
            sessions: Vec<(SessionFacts, Verdict)>,
            panes: Vec<Pane>,
            own_pane: Option<&'static str>,
            quiet: &'static [&'static str],
            kept: &'static [&'static str],
        }
        let tab = "w1:t1";
        let make_worker = || make_closing_worker("demo-kb-001-impl", tab);
        let make_worker_pane = || make_pane("w1:demo-kb-001-impl", tab, true);
        let working_neighbour = {
            let (facts, _) = make_closing_worker("demo-kb-002-impl", tab);
            let verdict = Verdict {
                state: Lifecycle::Working,
                why: String::new(),
            };
            (facts, verdict)
        };
        let parking_neighbour = {
            let (facts, _) = make_closing_worker("demo-kb-007-impl", tab);
            let verdict = Verdict {
                state: Lifecycle::ToPark,
                why: String::new(),
            };
            (facts, verdict)
        };
        let cases = [
            Case {
                name: "own-pane",
                sessions: vec![make_worker()],
                panes: vec![make_worker_pane(), make_pane("w1:p-hub", tab, false)],
                own_pane: Some("w1:p-hub"),
                quiet: &["w1:p-hub"],
                kept: &["keep demo-kb-001-impl"],
            },
            Case {
                name: "focused-pane",
                sessions: vec![make_worker()],
                panes: vec![
                    make_worker_pane(),
                    Pane {
                        focused: true,
                        ..make_pane("w1:p8", tab, false)
                    },
                ],
                own_pane: None,
                quiet: &["w1:p8"],
                kept: &["keep demo-kb-001-impl"],
            },
            Case {
                name: "working-neighbour",
                sessions: vec![make_worker(), working_neighbour],
                panes: vec![
                    make_worker_pane(),
                    make_pane("w1:demo-kb-002-impl", tab, true),
                ],
                own_pane: None,
                quiet: &[],
                kept: &["keep demo-kb-001-impl"],
            },
            Case {
                name: "parking-neighbour",
                sessions: vec![make_worker(), parking_neighbour],
                panes: vec![
                    make_worker_pane(),
                    make_pane("w1:demo-kb-007-impl", tab, true),
                ],
                own_pane: None,
                quiet: &[],
                kept: &["keep demo-kb-001-impl", "keep demo-kb-007-impl"],
            },
            Case {
                name: "busy-shell",
                sessions: vec![make_worker()],
                panes: vec![make_worker_pane(), make_pane("w1:p8", tab, false)],
                own_pane: None,
                quiet: &[],
                kept: &["keep demo-kb-001-impl"],
            },
            Case {
                name: "lone-shell",
                sessions: Vec::new(),
                panes: vec![make_pane("w1:p9", "w1:t9", false)],
                own_pane: None,
                quiet: &["w1:p9"],
                kept: &[],
            },
        ];

        for case in cases {
            let actions = plan_for(&case.sessions, &case.panes, case.own_pane, case.quiet);

            assert_eq!(actions, case.kept, "{}", case.name);
        }
    }

    /// A worker whose card needs the user's review must park, so its
    /// session can reopen when review comments come. A worker whose
    /// card's work is over must close instead. Swapping the two closes
    /// reviewed work with no way back.
    #[test]
    fn plan_tidy_actions_parks_review_closes_done() {
        let review = (
            make_facts("demo-kb-007-impl"),
            Verdict {
                state: Lifecycle::ToPark,
                why: String::new(),
            },
        );
        let done = make_closing_worker("demo-kb-001-impl", "w1:t1");
        let panes = [
            make_pane("w1:demo-kb-007-impl", "w1:t-demo-kb-007-impl", true),
            make_pane("w1:demo-kb-001-impl", "w1:t1", true),
        ];

        let actions = plan_for(&[review, done], &panes, None, &[]);

        assert_eq!(
            actions,
            [
                "park demo-kb-007-impl DEMO-KB-007",
                "close w1:t1 demo-kb-001-impl: DEMO-KB-001 done",
            ]
        );
    }

    /// A parked record whose card's work is over must go, or the
    /// parked folder keeps a record no card can use again.
    #[test]
    fn plan_tidy_actions_forgets_parked_done_record() {
        let (mut facts, verdict) = make_closing_worker("demo-kb-003-auto", "w1:t3");
        facts.agent = None;
        facts.parked = Some(make_parked_record("demo-kb-003-auto"));

        let actions = plan_for(&[(facts, verdict)], &[], None, &[]);

        assert_eq!(actions, ["forget demo-kb-003-auto"]);
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

        assert_eq!(actions, ["close w1:t1 demo-kb-001-impl: DEMO-KB-001 done"]);
    }
}
