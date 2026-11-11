//! Runs `hub go` and `hub gather`, which find the hubs in Herdr's
//! sidebar. A hub is an agent whose name ends in `-hub-<n>`.

use super::Outcome;
use crate::external::herdr::{Agent, Workspace};
use crate::shared::lifecycle::is_hub_name;
use crate::shared::{HubError, Tools};

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

/// Moves every hub's workspace into one block at the top of the
/// sidebar. Two hubs can share a workspace, and Herdr refuses a
/// repeated ID, so each workspace is moved once.
pub fn run_gather(tools: &Tools<'_>) -> Result<Outcome, HubError> {
    let agents = tools.herdr.list_agents()?;
    let workspaces = tools.herdr.list_workspaces()?;
    let hubs = list_hubs(&agents, &workspaces);
    if hubs.is_empty() {
        return Err(HubError::NotDone("no hub is running".to_owned()));
    }
    let mut block: Vec<String> = Vec::new();
    for hub in &hubs {
        if !block.contains(&hub.workspace_id) {
            block.push(hub.workspace_id.clone());
        }
    }
    // Herdr also refuses a block placed before one of its own members.
    let first_other = workspaces
        .iter()
        .find(|workspace| !block.contains(&workspace.workspace_id));
    if let Some(first_other) = first_other {
        tools
            .herdr
            .move_workspaces(&block, &first_other.workspace_id)?;
    }
    Ok(Outcome::line(format!(
        "hubs at the top: {}",
        block.join(" ")
    )))
}

/// Returns the hubs ordered by their workspace's place, then by name.
/// Includes the hub in this pane, since that hub runs `hub gather`
/// itself.
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
