//! Runs `hub go`, `hub gather` and `hub desk`, which find the hubs in
//! Herdr's sidebar. A hub is an agent whose name ends in `-hub-<n>`.
//! The desk is the user's own workspace, which holds no session.
//! `gather` and `desk` put the kanban hubs at the top, then the desk,
//! then the other hubs. They also cut Herdr's agent list down to a "go
//! to" list in that order, followed by the sessions that wait on the
//! user. `hub handoff`, `hub adopt` and `hub auto now` gather too,
//! through `gather_after_hub_start`, so no hub has to run `hub gather`.

use std::path::Path;

use super::Outcome;
use crate::external::herdr::{Agent, AgentStatus, HubKind, NewPlace, Workspace};
use crate::shared::lifecycle::is_hub_name;
use crate::shared::{HubError, Tools};

/// The desk workspace's label, which is how `hub desk` and `hub gather`
/// find it.
const DESK_LABEL: &str = "desk";
/// Typed into the desk's first tab.
const BOARD_COMMAND: &str = "hub board";
/// Starts the name of each hub the user works the kanban board with.
const KANBAN_HUB_PREFIX: &str = "kanban-hub-";

/// Shows the hub after the one the user looks at, in sidebar order,
/// wrapping to the first.
pub fn run_go(tools: &Tools<'_>) -> Result<Outcome, HubError> {
    let agents = tools.herdr.list_agents()?;
    let workspaces = tools.herdr.list_workspaces()?;
    let hubs = list_hubs(&agents, &workspaces);
    let Some(next_hub) = choose_next_hub(&hubs) else {
        return Err(HubError::NotDone("no hub is running".to_owned()));
    };
    tools.herdr.focus_agent(&next_hub.name)?;
    Ok(Outcome::line(format!("showing {}", next_hub.name)))
}

/// Moves the desk, when it is open, and every hub's workspace into one
/// block at the top of the sidebar. Kanban hubs come first, then the
/// desk, then the other hubs. It then fills the "go to" agent list.
pub fn run_gather(tools: &Tools<'_>) -> Result<Outcome, HubError> {
    let Some(block) = gather_hubs(tools)? else {
        return Err(HubError::NotDone("no hub is running".to_owned()));
    };
    Ok(Outcome::line(format!("at the top: {}", block.join(" "))))
}

/// Does what `hub gather` does, for a command that just started or
/// took over a hub. Returns a warning line when that fails, since a
/// sidebar problem must never fail the command. Returns `None` when it
/// worked or when no hub runs.
pub(crate) fn gather_after_hub_start(tools: &Tools<'_>) -> Option<String> {
    match gather_hubs(tools) {
        Ok(_) => None,
        Err(error) => Some(format!(
            "could not put the hubs on top: {error}; hub gather tries again"
        )),
    }
}

/// Makes the desk when it is missing, with the board in its first tab
/// and a shell in its second, so the board shows first on a phone.
/// It then moves the desk to the top, right below the kanban hubs, and
/// fills the "go to" agent list. It never focuses the desk.
pub fn run_desk(tools: &Tools<'_>, home: &Path) -> Result<Outcome, HubError> {
    let agents = tools.herdr.list_agents()?;
    let workspaces = tools.herdr.list_workspaces()?;
    let (desk_id, line) = match find_desk(&workspaces) {
        Some(desk) => (desk.workspace_id.clone(), "desk is open"),
        None => (make_desk(tools, home)?, "made desk: board, then shell"),
    };
    let hubs = list_hubs(&agents, &workspaces);
    let block = list_top_block(&hubs, Some(&desk_id));
    move_block_to_top(tools, &block, &workspaces)?;
    show_go_to_list(tools, &agents, &hubs, Some(&desk_id))?;
    Ok(Outcome::line(line))
}

/// Moves the top block and fills the "go to" agent list, then returns
/// the block. Returns `None` and changes nothing when no hub runs.
fn gather_hubs(tools: &Tools<'_>) -> Result<Option<Vec<String>>, HubError> {
    let agents = tools.herdr.list_agents()?;
    let workspaces = tools.herdr.list_workspaces()?;
    let hubs = list_hubs(&agents, &workspaces);
    if hubs.is_empty() {
        return Ok(None);
    }
    let desk_id = find_desk(&workspaces).map(|desk| desk.workspace_id.as_str());
    let block = list_top_block(&hubs, desk_id);
    move_block_to_top(tools, &block, &workspaces)?;
    show_go_to_list(tools, &agents, &hubs, desk_id)?;
    Ok(Some(block))
}

/// Makes the desk workspace in `home` and returns its ID.
fn make_desk(tools: &Tools<'_>, home: &Path) -> Result<String, HubError> {
    let place = |label: &str| NewPlace {
        label: label.to_owned(),
        cwd: home.to_owned(),
        env: Vec::new(),
    };
    let desk = tools.herdr.create_workspace(&place(DESK_LABEL))?;
    tools.herdr.run_in_pane(&desk.root_pane_id, BOARD_COMMAND)?;
    tools
        .herdr
        .create_tab(&desk.workspace_id, &place("shell"))?;
    Ok(desk.workspace_id)
}

/// Returns the first workspace labelled `desk`.
fn find_desk(workspaces: &[Workspace]) -> Option<&Workspace> {
    workspaces
        .iter()
        .find(|workspace| workspace.label == DESK_LABEL)
}

/// Returns the workspace IDs that belong at the top, each once. The
/// kanban hubs' come first, then the desk, then the other hubs'. Two
/// hubs can share a workspace, and Herdr refuses a repeated ID.
fn list_top_block(hubs: &[&Agent], desk_id: Option<&str>) -> Vec<String> {
    let (kanban_hubs, other_hubs): (Vec<&Agent>, Vec<&Agent>) = hubs
        .iter()
        .copied()
        .partition(|hub| find_hub_kind(hub) == HubKind::Kanban);
    let ids = kanban_hubs
        .iter()
        .map(|hub| hub.workspace_id.as_str())
        .chain(desk_id)
        .chain(other_hubs.iter().map(|hub| hub.workspace_id.as_str()));
    let mut block: Vec<String> = Vec::new();
    for id in ids {
        if !block.iter().any(|kept| kept == id) {
            block.push(id.to_owned());
        }
    }
    block
}

fn find_hub_kind(hub: &Agent) -> HubKind {
    if hub.name.starts_with(KANBAN_HUB_PREFIX) {
        HubKind::Kanban
    } else {
        HubKind::Other
    }
}

/// Moves `block` above every other workspace in `workspaces`.
fn move_block_to_top(
    tools: &Tools<'_>,
    block: &[String],
    workspaces: &[Workspace],
) -> Result<(), HubError> {
    // Herdr refuses a block placed before one of its own members, so a
    // sidebar that holds only the block stays as it is.
    let first_other = workspaces
        .iter()
        .find(|workspace| !block.contains(&workspace.workspace_id));
    if let Some(first_other) = first_other {
        tools
            .herdr
            .move_workspaces(block, &first_other.workspace_id)?;
    }
    Ok(())
}

/// Tags each hub's pane and the desk's board pane, then makes Herdr's
/// agent list show only them and the sessions that wait on the user.
/// Another machine's hubs show there once a hub on that machine runs
/// `gather` too, since each machine tags only its own panes. Herdr
/// forgets pane tokens and the filter when its server restarts, so
/// each handoff, adoption and `hub auto now` sets them again.
fn show_go_to_list(
    tools: &Tools<'_>,
    agents: &[Agent],
    hubs: &[&Agent],
    desk_id: Option<&str>,
) -> Result<(), HubError> {
    for hub in hubs {
        tools.herdr.tag_hub_pane(&hub.pane_id, find_hub_kind(hub))?;
    }
    if let Some(desk_id) = desk_id {
        tools.herdr.show_desk_as_agent(desk_id)?;
    }
    // An older `hub flag` set the "needs you" label but not the token
    // the filter reads, so a flagged session gets its mark again.
    let flagged_idle = agents.iter().filter(|agent| {
        agent.marked && matches!(agent.status, AgentStatus::Idle | AgentStatus::Done)
    });
    for agent in flagged_idle {
        tools
            .herdr
            .mark_needs_you(&agent.pane_id, &[AgentStatus::Idle, AgentStatus::Done])?;
    }
    tools.herdr.filter_agent_list()?;
    Ok(())
}

/// Returns the hubs ordered by their workspace's place, then by name.
/// Includes the hub in this pane, since the commands that gather run in
/// a hub's own pane.
fn list_hubs<'a>(agents: &'a [Agent], workspaces: &[Workspace]) -> Vec<&'a Agent> {
    let find_place = |agent: &Agent| {
        workspaces
            .iter()
            .position(|workspace| workspace.workspace_id == agent.workspace_id)
            .unwrap_or(workspaces.len())
    };
    let mut hubs: Vec<&Agent> = agents
        .iter()
        .filter(|agent| is_hub_name(&agent.name))
        .collect();
    hubs.sort_by(|left, right| {
        (find_place(left), &left.name).cmp(&(find_place(right), &right.name))
    });
    hubs
}

/// Returns the hub after the focused one, or the first when none is
/// focused.
fn choose_next_hub<'a>(hubs: &[&'a Agent]) -> Option<&'a Agent> {
    match hubs.iter().position(|hub| hub.focused) {
        Some(place) => Some(hubs[(place + 1) % hubs.len()]),
        None => hubs.first().copied(),
    }
}
