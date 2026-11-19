//! Starts a worker from its handoff file and does the bookkeeping a
//! hub once did by hand. A worker also runs it with `--successor` to
//! start its own next session. Read `run_launch` first.

use std::path::PathBuf;

use jiff::Zoned;

use super::Outcome;
use super::handoff::{SessionPlace, make_handoff_prompt, start_session};
use crate::external::cards::{CardKey, Stage};
use crate::external::herdr::Agent;
use crate::shared::parking::find_card_workspace;
use crate::shared::run_folder::RunFolder;
use crate::shared::{AgentName, HubError, Tools};
use crate::watch::handoff_ask;

/// What `run_launch` starts, from its command line.
pub struct LaunchRequest {
    /// A card key at its start, as in `demo-kb-007-impl`, makes the
    /// worker that card's worker.
    pub name: AgentName,
    pub handoff: PathBuf,
    pub mode: LaunchMode,
    /// The folder the worker opens in.
    pub cwd: PathBuf,
}

/// Who picks the new session's model and place.
pub enum LaunchMode {
    /// A hub starts a worker on this model, in `--workspace` or by the
    /// sidebar rule. The model holds its effort, such as
    /// `github-copilot/claude-sonnet-5#high`.
    Worker {
        model: String,
        workspace: Option<String>,
    },
    /// A worker starts its own successor, in a new tab beside it on its
    /// own model and effort. The session is the caller's, from
    /// `OPENCODE_SESSION_ID`, or `None` outside OpenCode.
    Successor { session_id: Option<String> },
}

/// Starts the worker NAME and gives it its handoff. A worker with a
/// card also gets a `Handoff PATH` note and its session on the card.
/// Every start adds a line to `notes.md`. A card ready to start moves
/// into progress before the start, so the work limit is checked
/// first. A successor opens beside its caller, and the caller's "needs
/// you" mark goes once the successor has its handoff.
///
/// # Errors
///
/// `HubError::NotDone` before anything changes, for a missing handoff,
/// a live agent named NAME, a card `Cards::list_cards` does not give,
/// or no workspace to open in. For a successor, also when no live agent
/// runs the caller's session or the session has no reply to take the
/// model from. `HubError::Card` when
/// `Cards::move_to_active` refuses the move, also before the start. A
/// start that fails after the move is `HubError::PartlyDone` worded
/// `moved KEY to active, but ...`. After the start,
/// `HubError::PartlyDone` starts with `started NAME` and lists each
/// later step that finished, as in `started NAME, sent the handoff
/// prompt, but ...`, so the hub finishes the rest by hand instead of
/// running the command again.
pub fn run_launch(
    tools: &Tools<'_>,
    run: &RunFolder,
    request: &LaunchRequest,
    now: &Zoned,
) -> Result<Outcome, HubError> {
    let name = request.name.as_str();
    if !request.handoff.is_file() {
        return Err(HubError::NotDone(format!(
            "{} does not exist",
            request.handoff.display()
        )));
    }
    let handoff = std::path::absolute(&request.handoff).map_err(|source| HubError::File {
        path: request.handoff.clone(),
        source,
    })?;
    // A rerun after a failed step must not start a second worker.
    let agents = tools.herdr.list_agents()?;
    if agents.iter().any(|agent| agent.name == name) {
        return Err(HubError::NotDone(format!("{name} already runs")));
    }
    let card = match CardKey::find_in_name(name) {
        Some(key) => {
            let listed = tools.cards.list_cards()?;
            let Some(found) = listed.into_iter().find(|card| card.key == key) else {
                return Err(HubError::NotDone(format!(
                    "card list shows no {}",
                    key.as_str()
                )));
            };
            Some((key, found.stage))
        }
        None => None,
    };
    let (place, model, caller) = match &request.mode {
        LaunchMode::Worker { model, workspace } => {
            let card_key = card.as_ref().map(|(key, _)| key);
            let place = choose_place(tools, name, workspace.as_deref(), card_key)?;
            (place, model.clone(), None)
        }
        LaunchMode::Successor { session_id } => {
            let (caller, model) = find_caller_and_model(tools, &agents, session_id.as_deref())?;
            let place = SessionPlace::Tab {
                workspace_id: caller.workspace_id.clone(),
            };
            (place, model, Some(caller))
        }
    };
    let moved = match &card {
        Some((key, Stage::Todo)) => {
            tools.cards.move_to_active(key)?;
            Some(key)
        }
        _ => None,
    };

    let started = start_session(tools.herdr, &place, name, &model, run, &request.cwd);
    let ids = match (started, moved) {
        (Ok(ids), _) => ids,
        (Err(error), Some(key)) => {
            return Err(HubError::PartlyDone {
                done: format!("moved {} to active", key.as_str()),
                error: Box::new(error.into()),
            });
        }
        (Err(error), None) => return Err(error.into()),
    };
    // The error lists each step that finished, so the hub redoes only
    // the rest by hand.
    let mut done = vec![format!("started {name}")];
    let partly_done = |done: &[String], error: HubError| HubError::PartlyDone {
        done: done.join(", "),
        error: Box::new(error),
    };
    tools
        .herdr
        .prompt(name, &make_handoff_prompt(&handoff))
        .map_err(|error| partly_done(&done, error.into()))?;
    done.push("sent the handoff prompt".to_owned());
    let mut lines = vec![format!("started {name} ({}) on {model}", ids.root_pane_id)];
    if let SessionPlace::Workspace { label } = &place {
        lines.push(format!("new workspace {label}"));
    }
    if let Some((key, _)) = &card {
        let note = format!("Handoff {}", handoff.display());
        tools
            .cards
            .add_note(key, &note)
            .map_err(|error| partly_done(&done, error.into()))?;
        done.push("added the card note".to_owned());
        tools
            .cards
            .link_session(key, name)
            .map_err(|error| partly_done(&done, error.into()))?;
        done.push("linked the session".to_owned());
    }
    let note = format!(
        "started {name} ({}) from {}",
        ids.root_pane_id,
        handoff.display()
    );
    run.append_note_line(&note, now)
        .map_err(|error| partly_done(&done, error))?;
    done.push("wrote the notes line".to_owned());
    if let Some(caller) = &caller {
        // A marked session waits on the user, so `hub clean` would keep
        // the caller open after its successor took the work. This comes
        // last, so a failure here cannot skip the card link that `hub
        // clean` needs to close the caller.
        tools
            .herdr
            .clear_marks(&caller.pane_id)
            .map_err(|error| partly_done(&done, error.into()))?;
        done.push(format!("cleared the mark of {}", caller.name));
        if let Some(session) = &caller.session_id {
            handoff_ask::stop_asking(tools.state_folder, session)
                .map_err(|error| partly_done(&done, error))?;
            done.push(format!("stopped the handoff asks to {}", caller.name));
        }
    }
    Ok(Outcome { lines })
}

/// Returns where the worker opens, by the sidebar rule of the hub's
/// skill. `--workspace` names the workspace, made when
/// missing. Otherwise a card's worker opens in its card's workspace,
/// else in a new one named after the card. Work with no card opens in
/// the hub's own workspace.
fn choose_place(
    tools: &Tools<'_>,
    name: &str,
    workspace: Option<&str>,
    card: Option<&CardKey>,
) -> Result<SessionPlace, HubError> {
    if let Some(label) = workspace {
        let workspaces = tools.herdr.list_workspaces()?;
        let found = workspaces
            .into_iter()
            .find(|workspace| workspace.label.eq_ignore_ascii_case(label));
        return Ok(match found {
            Some(workspace) => SessionPlace::Tab {
                workspace_id: workspace.workspace_id,
            },
            None => SessionPlace::Workspace {
                label: label.to_owned(),
            },
        });
    }
    if let Some(card) = card {
        let workspaces = tools.herdr.list_workspaces()?;
        return Ok(match find_card_workspace(&workspaces, card) {
            Some(workspace) => SessionPlace::Tab {
                workspace_id: workspace.workspace_id.clone(),
            },
            None => SessionPlace::Workspace {
                label: name_card_workspace(name, card),
            },
        });
    }
    let Some(own_pane) = tools.own_pane else {
        return Err(HubError::NotDone(
            "run hub launch in the hub's own pane, or pass --workspace".to_owned(),
        ));
    };
    let hub = tools.herdr.get_agent(own_pane)?;
    Ok(SessionPlace::Tab {
        workspace_id: hub.workspace_id,
    })
}

/// Returns the agent that runs the caller's OpenCode session, and the
/// model and effort of its newest reply.
fn find_caller_and_model(
    tools: &Tools<'_>,
    agents: &[Agent],
    session_id: Option<&str>,
) -> Result<(Agent, String), HubError> {
    let Some(session_id) = session_id else {
        return Err(HubError::NotDone(
            "run hub launch --successor from an OpenCode session".to_owned(),
        ));
    };
    let Some(caller) = agents
        .iter()
        .find(|agent| agent.session_id.as_deref() == Some(session_id))
    else {
        return Err(HubError::NotDone(format!(
            "no live agent runs OpenCode session {session_id}"
        )));
    };
    let Some(model) = tools.opencode.read_model(session_id)? else {
        return Err(HubError::NotDone(format!(
            "OpenCode session {session_id} has no reply to take its model from"
        )));
    };
    Ok((caller.clone(), model))
}

/// Returns the label of a card's new workspace, the key and the rest
/// of the name, such as `demo-kb-007 plan-2` for `demo-kb-007-plan-2`.
fn name_card_workspace(name: &str, card: &CardKey) -> String {
    let key = card.to_lowercase();
    let role = name
        .strip_prefix(key.as_str())
        .unwrap_or_default()
        .trim_start_matches('-');
    format!("{key} {role}").trim_end().to_owned()
}
