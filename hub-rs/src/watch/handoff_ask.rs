//! Asks each idle session past the handoff point to hand off, with or
//! without a hub. The ask comes again at each 20k more tokens, and from
//! 190k the session is also marked "needs you". The highest context
//! asked for each session is saved, so a restarted watch does not ask
//! again at the same step.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::Path;

use super::{IdleClock, WATCH_FOLDER, count_held, is_idle, prod_worker};
use crate::commands::sidebar;
use crate::external::herdr::{Agent, AgentKind, AgentStatus};
use crate::shared::context::{HANDOFF_TOKENS, format_tokens};
use crate::shared::lifecycle;
use crate::shared::messages::MessageCounts;
use crate::shared::run_folder;
use crate::shared::{HubError, Tools};

/// The highest context asked for each OpenCode session, in the watch's
/// folder. It keeps one entry for each session ever asked.
const ASKS_FILE: &str = "handoff-asks.json";

/// The tokens past the last ask that bring the next one.
const ASK_AGAIN_EVERY: u64 = 20_000;

/// The first step whose ask also marks the session "needs you", 190k.
const MARK_FROM_STEP: u64 = 2;

/// Asks each idle OpenCode session that is not a hub and has reached a
/// new step past `HANDOFF_TOKENS`, and saves the ask. A hub gets no ask,
/// since its pane is the user's own chat, and neither does a session in
/// the desk, the user's own workspace, where its successor would open.
/// A prod already typed in the session's present Herdr state, in
/// `prodded`, holds the ask until its next state. `held` gives each
/// run's held messages. A failure with one session adds to `failures`
/// and the pass goes on.
///
/// # Errors
///
/// Herdr's refusal to list workspaces, or a file failure with the saved
/// asks, which ends this pass.
pub(super) fn ask_for_handoffs(
    tools: &Tools<'_>,
    agents: &[Agent],
    hubs: &BTreeSet<&str>,
    held: &[&BTreeMap<String, MessageCounts>],
    clock: &mut IdleClock,
    prodded: &BTreeSet<(String, u64)>,
    failures: &mut Vec<String>,
) -> Result<(), HubError> {
    let path = tools.state_folder.join(WATCH_FOLDER).join(ASKS_FILE);
    let mut asked = read_asked_tokens(&path)?;
    let due = find_due_asks(tools, agents, hubs, clock, prodded, &asked, failures);
    if due.is_empty() {
        return Ok(());
    }
    let workspaces = tools.herdr.list_workspaces()?;
    let desk_id = sidebar::find_desk(&workspaces).map(|desk| desk.workspace_id.as_str());
    for (agent, session, tokens) in due {
        if desk_id == Some(agent.workspace_id.as_str()) {
            continue;
        }
        if find_ask_step(tokens) >= Some(MARK_FROM_STEP) && !agent.marked {
            let marked = tools
                .herdr
                .mark_needs_you(&agent.pane_id, &[AgentStatus::Idle, AgentStatus::Done]);
            if let Err(error) = marked {
                failures.push(HubError::from(error).to_string());
            }
        }
        let held_count = held.iter().map(|run| count_held(run, &agent.name)).sum();
        match prod_worker(tools, agent, held_count, &make_ask(tokens)) {
            Ok(true) => {}
            Ok(false) => continue,
            Err(error) => {
                failures.push(error.to_string());
                continue;
            }
        }
        asked.insert(session.to_owned(), tokens);
        save_asked_tokens(&path, &asked)?;
    }
    Ok(())
}

/// Saves that the session started its successor, so the watch never
/// asks or marks it again, whatever its context. Marked again, it would
/// wait on the user and `hub clean` would keep it open.
///
/// # Errors
///
/// A file failure with the saved asks.
pub(crate) fn stop_asking(state_folder: &Path, session: &str) -> Result<(), HubError> {
    let path = state_folder.join(WATCH_FOLDER).join(ASKS_FILE);
    let mut asked = read_asked_tokens(&path)?;
    asked.insert(session.to_owned(), u64::MAX);
    save_asked_tokens(&path, &asked)
}

/// Returns each idle OpenCode session, other than a hub or one prodded
/// in its present state, whose context reached a step above the one
/// `asked` saves, with its session ID and tokens.
fn find_due_asks<'a>(
    tools: &Tools<'_>,
    agents: &'a [Agent],
    hubs: &BTreeSet<&str>,
    clock: &mut IdleClock,
    prodded: &BTreeSet<(String, u64)>,
    asked: &BTreeMap<String, u64>,
    failures: &mut Vec<String>,
) -> Vec<(&'a Agent, &'a str, u64)> {
    let mut due = Vec::new();
    for agent in agents {
        let is_hub = lifecycle::is_hub_name(&agent.name) || hubs.contains(agent.name.as_str());
        let Some(session) = agent.session_id.as_deref() else {
            continue;
        };
        if agent.kind != AgentKind::OpenCode
            || !is_idle(agent)
            || is_hub
            || prodded.contains(&(agent.name.clone(), agent.state_seq))
        {
            continue;
        }
        let tokens = match clock.read_idle_context(tools.opencode, agent, session) {
            Ok(Some(tokens)) => tokens,
            Ok(None) => continue,
            Err(error) => {
                failures.push(error.to_string());
                continue;
            }
        };
        // `None` sorts below every step, so a first ask needs only a step.
        if find_ask_step(tokens) > asked.get(session).copied().and_then(find_ask_step) {
            due.push((agent, session, tokens));
        }
    }
    due
}

/// Returns how many 20k steps past `HANDOFF_TOKENS` the context is, or
/// `None` below it. 150k is step 0, 170k step 1 and 190k step 2.
fn find_ask_step(tokens: u64) -> Option<u64> {
    tokens
        .checked_sub(HANDOFF_TOKENS)
        .map(|over| over / ASK_AGAIN_EVERY)
}

/// Returns the ask for a session at this context. It names the
/// herdr-handoff steps, so a session that never loaded that skill still
/// knows what to do.
fn make_ask(tokens: u64) -> String {
    format!(
        "your context is about {}, past the {} handoff point. \
         Hand off now as the herdr-handoff skill says: write your handoff, \
         start your successor with hub launch --successor --run FOLDER, \
         where FOLDER is your run folder, else ~/.local/state/pde/handoffs, \
         report its name to your hub if one runs, and end your turn. \
         If your brief or a skill you follow says what to do near your \
         context limit, do that instead. If you already started a \
         successor, or your work is done, or your card is done or \
         dropped, just end your turn.",
        format_tokens(Some(tokens)),
        format_tokens(Some(HANDOFF_TOKENS))
    )
}

/// Returns the saved asks at `path`, or none when the file does not
/// exist.
fn read_asked_tokens(path: &Path) -> Result<BTreeMap<String, u64>, HubError> {
    let file_error = |source| HubError::File {
        path: path.to_owned(),
        source,
    };
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(BTreeMap::new()),
        Err(source) => return Err(file_error(source)),
    };
    serde_json::from_str(&text).map_err(|error| file_error(error.into()))
}

/// Writes the saved asks whole to `path`, making its folder first.
fn save_asked_tokens(path: &Path, asked: &BTreeMap<String, u64>) -> Result<(), HubError> {
    let file_error = |source| HubError::File {
        path: path.to_owned(),
        source,
    };
    if let Some(folder) = path.parent() {
        fs::create_dir_all(folder).map_err(file_error)?;
    }
    let text = serde_json::to_string(asked).map_err(|error| file_error(error.into()))?;
    run_folder::write_file_whole(path, &text)
}
