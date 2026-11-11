//! Runs `hub clean` and `hub park`. The tidy pass they share with
//! `hub next` lives in `shared/tidy.rs`.

use std::collections::BTreeSet;

use jiff::Zoned;

use super::Outcome;
use crate::shared::lifecycle::{Lifecycle, decide_lifecycle};
use crate::shared::parking::{self, ParkOutcome};
use crate::shared::run_folder::RunFolder;
use crate::shared::tidy::{self, TAB_HOLDS_WORK, TidyAction, TidyOutcome};
use crate::shared::{AgentName, HubError, Tools, events, facts};

/// Runs the tidy pass now and prints one line per close, park, forget
/// or keep, then `nothing to close` when nothing changed. With
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
    let lock = tools.parked.take_tidy_lock()?;
    let memory = events::load_memory(run)?;
    let collected = facts::collect_session_facts(tools, run, &memory)?;
    let outcomes = tidy::run_tidy_pass(tools, run, &collected, &lock, now, dry_run)?;
    let mut lines: Vec<String> = outcomes
        .iter()
        .map(|outcome| outcome.line(dry_run))
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
    let stays = |why: &str| HubError::NotDone(format!("{name} stays: {why}"));
    if let Some(problem) = tidy::find_unsafe_problem(&collected) {
        return Err(stays(problem));
    }
    let (panes, quiet_shells) = match &facts.agent {
        Some(agent) => tidy::read_tab_panes(tools, &agent.tab_id)?,
        None => (Vec::new(), BTreeSet::new()),
    };
    let verdict = decide_lifecycle(facts);
    let is_park = verdict.state == Lifecycle::ToPark;
    let judged = [(facts, verdict)];
    let actions = tidy::plan_tidy_actions(&judged, &panes, tools.own_pane, &quiet_shells);
    if is_park && !matches!(actions.as_slice(), [TidyAction::Park { .. }]) {
        return Err(stays(TAB_HOLDS_WORK));
    }
    match parking::park_session(tools, run, facts, &panes, now)? {
        ParkOutcome::Parked(card) => {
            Ok(Outcome::line(format!("parked {name} on {}", card.as_str())))
        }
        ParkOutcome::Stays(verdict) => Err(stays(verdict.why_or_state())),
        ParkOutcome::TabInUse => Err(stays(TAB_HOLDS_WORK)),
    }
}
