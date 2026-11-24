//! Runs `hub clean` and `hub park`, the only commands that close or
//! park sessions. The tidy pass lives in `shared/tidy.rs`.

use std::collections::BTreeSet;

use jiff::Zoned;

use super::Outcome;
use crate::shared::lifecycle::{Lifecycle, decide_lifecycle};
use crate::shared::parking::{self, ParkOutcome};
use crate::shared::run_folder::RunFolder;
use crate::shared::tidy::{self, TAB_HOLDS_WORK, TidyAction, TidyOutcome, TidyScope};
use crate::shared::{AgentName, HubError, Tools, events, facts};

/// Runs the tidy pass now on the sessions this run knows, as
/// `tidy::list_run_session_names` finds them. A tab with none of them
/// stays open, except in a finished auto run's workspace. A finished
/// `to-close` agent of another run closes with a tab it shares with
/// them. When an auto hub this run adopted is `to-close`, the tabs in
/// its workspace and in each workspace of a session its run started
/// may change too. A workspace that holds this hub's pane, another
/// open hub or the user's desk is left out. Prints one line per close,
/// park, forget or keep, then `nothing to close` when nothing changed. With
/// `dry_run`, it prints what it would do and changes nothing.
///
/// # Errors
///
/// A Herdr failure, or a run folder or state file it cannot read.
pub fn run_clean(
    tools: &Tools<'_>,
    run: &RunFolder,
    dry_run: bool,
    now: &Zoned,
) -> Result<Outcome, HubError> {
    clean_sessions(tools, run, None, dry_run, now)
}

/// Runs the tidy pass as `run_clean` does, but on the sessions in
/// `names` instead of this run's, such as the ones the board's clean
/// dialog listed from every hub. A listed session the pass now keeps
/// stays open, with a `kept` line. An unlisted agent keeps its tab, so
/// the dialog's y key closes no session it did not name.
///
/// # Errors
///
/// As `run_clean`.
pub fn run_clean_listed(
    tools: &Tools<'_>,
    run: &RunFolder,
    names: &[String],
    now: &Zoned,
) -> Result<Outcome, HubError> {
    clean_sessions(tools, run, Some(names), false, now)
}

/// Runs the tidy pass with this run's sessions and the workspaces of
/// each finished auto run it adopted as the scope, or with `only` as
/// the scope. With `only`, every other session is dropped first, so its
/// agent keeps its tab.
fn clean_sessions(
    tools: &Tools<'_>,
    run: &RunFolder,
    only: Option<&[String]>,
    dry_run: bool,
    now: &Zoned,
) -> Result<Outcome, HubError> {
    let lock = tools.parked.take_tidy_lock()?;
    let memory = events::load_memory(run)?;
    let mut collected = facts::collect_session_facts(tools, run, &memory)?;
    let (scope, kept_workspaces) = match only {
        Some(names) => {
            let names = names.iter().cloned().collect();
            let scope = TidyScope {
                names,
                retiring: Vec::new(),
            };
            (scope, Vec::new())
        }
        None => {
            let names = tidy::list_run_session_names(run, &memory)?;
            let (retiring, kept_workspaces) =
                tidy::find_retiring_workspaces(tools, &collected, &memory);
            (TidyScope { names, retiring }, kept_workspaces)
        }
    };
    // An agent with no facts keeps its tab. Plain clean keeps every
    // session, so another run's finished agent can close with a shared tab.
    if only.is_some() {
        collected
            .sessions
            .retain(|facts| scope.names.contains(&facts.name));
    }
    let mut outcomes = tidy::run_tidy_pass(tools, run, &collected, &scope, &lock, now, dry_run)?;
    outcomes.extend(kept_workspaces);
    let mut lines: Vec<String> = outcomes
        .iter()
        .map(|outcome| outcome.format_line(dry_run))
        .collect();
    if !outcomes.iter().any(TidyOutcome::is_change) {
        lines.push("nothing to close".to_owned());
    }
    Ok(Outcome { lines })
}

/// Parks one session now, by the same steps as the tidy pass, and
/// prints `parked NAME on KEY`. Its tab closes only when the tidy pass
/// would close it, so no other pane in the tab closes with it.
///
/// # Errors
///
/// `HubError::NotDone` when NAME is no session, or its state is not
/// `to-park`, with the reason, or its tab holds other work, or a card read
/// or request file failed. A Herdr, card or file failure otherwise.
pub fn run_park(
    tools: &Tools<'_>,
    run: &RunFolder,
    name: &AgentName,
    now: &Zoned,
) -> Result<Outcome, HubError> {
    let name = name.as_str();
    let _lock = tools.parked.take_tidy_lock()?;
    let memory = events::load_memory(run)?;
    let collected = facts::collect_session_facts(tools, run, &memory)?;
    let Some(facts) = collected.sessions.iter().find(|facts| facts.name == name) else {
        return Err(HubError::NotDone(format!("no session {name}")));
    };
    let make_stays_error = |why: &str| HubError::NotDone(format!("{name} stays: {why}"));
    if let Some(problem) = tidy::find_unsafe_problem(&collected) {
        return Err(make_stays_error(problem));
    }
    let (panes, quiet_shells) = match &facts.agent {
        Some(agent) => tidy::read_tab_panes(tools, &agent.tab_id)?,
        None => (Vec::new(), BTreeSet::new()),
    };
    let verdict = decide_lifecycle(facts);
    let is_park = verdict.state == Lifecycle::ToPark;
    let judged = [(facts, verdict)];
    let scope = BTreeSet::from([name.to_owned()]);
    let actions = tidy::plan_tidy_actions(
        &judged,
        &scope,
        &BTreeSet::new(),
        &panes,
        tools.own_pane,
        &quiet_shells,
    );
    if is_park && !matches!(actions.as_slice(), [TidyAction::Park { .. }]) {
        return Err(make_stays_error(TAB_HOLDS_WORK));
    }
    match parking::park_session(tools, run, facts, &panes, now)? {
        ParkOutcome::Parked(card) => Ok(Outcome::from_line(format!(
            "parked {name} on {}",
            card.as_str()
        ))),
        ParkOutcome::Stays(verdict) => Err(make_stays_error(verdict.pick_why_or_state())),
        ParkOutcome::TabInUse => Err(make_stays_error(TAB_HOLDS_WORK)),
    }
}
