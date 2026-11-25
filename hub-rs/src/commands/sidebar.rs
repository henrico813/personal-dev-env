//! Runs `hub go`, `hub gather`, `hub desk` and `hub restore-sidebar`,
//! which find the hubs and the desk in Herdr's sidebar. A hub is an
//! agent whose name ends in `-hub-<n>`, other than a session review,
//! whose name starts with `review-`. The desk is the user's own
//! workspace, and each agent in it is work the user does directly.
//! `gather` and `desk` put the kanban hubs' workspaces at the top,
//! then the desk, then the other hubs' workspaces. `run_handoff`, `run_adopt` and `run_auto_now`
//! gather too, through `gather_after_hub_start`, so no hub has to run
//! `hub gather`.
//!
//! Herdr's agent list, headed "go to", shows each machine's front hub,
//! then the desk's agents in tab order, then only the sessions that
//! wait on the user. Read `sync_go_to_list` first. It sets every
//! `go_to` token on this machine from scratch, and the `pde.hub` Herdr
//! plugin, `hub watch` and `hub flag` run it. `restore_other_machines`
//! fills in the tokens of machines whose Herdr has no `pde.hub` plugin.

use std::fs;
use std::io;
use std::path::Path;
use std::time::SystemTime;

use super::Outcome;
use super::cli::is_auto_run;
use crate::external::herdr::{
    Agent, AgentStatus, Herdr, HerdrError, NewPlace, PaneDestination, Workspace,
};
use crate::shared::lifecycle::{is_auto_hub, is_hub_name, parse_kanban_hub_number};
use crate::shared::run_folder::{RecordedHub, RunFolder};
use crate::shared::{AgentName, HubError, Tools};
use crate::watch::RUN_PREFIX;

/// The desk workspace's label, which is how `hub desk` and `hub gather`
/// find it.
const DESK_LABEL: &str = "desk";
/// Typed into the desk's first tab.
const BOARD_COMMAND: &str = "hub board";
/// Starts a front hub's `go_to` value, such as `1-hub-0`.
const FRONT_HUB_PLACE: &str = "1-hub";
/// Starts a desk agent's `go_to` value, such as `2-desk-0-03`.
const DESK_PLACE: &str = "2-desk";

/// What one pass over a machine's `go_to` tokens did.
pub(crate) struct Synced {
    /// Each pane whose token it set, changed or cleared, by agent name,
    /// or by pane ID for an agent nobody named.
    changed: Vec<String>,
    /// It reported the desk's board pane as agent `desk` again.
    board_shown: bool,
    /// The desk's board pane, when the machine has a desk.
    board_pane_id: Option<String>,
}

/// Which `go_to` tokens one pass may write.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Writes {
    /// Sets, changes and clears them, on this machine.
    All,
    /// Only sets missing ones, on another machine, whose own pass
    /// decides the rest.
    MissingOnly,
}

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
    Ok(Outcome::from_line(format!("showing {}", next_hub.name)))
}

/// Moves the desk, when it is open, and every hub's workspace into one
/// block at the top of the sidebar. Kanban hubs come first, then the
/// desk, then the other hubs. It then fills the "go to" agent list.
pub fn run_gather(tools: &Tools<'_>) -> Result<Outcome, HubError> {
    let Some(block) = gather_hubs(tools)? else {
        return Err(HubError::NotDone("no hub is running".to_owned()));
    };
    Ok(Outcome::from_line(format!(
        "at the top: {}",
        block.join(" ")
    )))
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
    show_go_to_list(tools, &agents)?;
    Ok(Outcome::from_line(line))
}

/// Moves the agent's pane into the desk as its last tab, named after
/// the agent, then fills the "go to" list again. It never focuses the
/// desk.
pub fn run_desk_add(tools: &Tools<'_>, name: &AgentName) -> Result<Outcome, HubError> {
    let agent = tools.herdr.get_agent(name.as_str())?;
    let desk_id = find_desk_id(tools)?;
    if agent.workspace_id == desk_id {
        return Ok(Outcome::from_line(format!(
            "{} is already on the desk",
            name.as_str()
        )));
    }
    let destination = PaneDestination::NewTab {
        workspace_id: desk_id,
        label: name.as_str().to_owned(),
    };
    tools.herdr.move_pane(&agent.pane_id, &destination)?;
    refresh_after_desk_change(tools, format!("{} is on the desk", name.as_str()))
}

/// Moves the agent's pane out of the desk into a new workspace named
/// after the agent, then fills the "go to" list again, which clears
/// its desk place.
pub fn run_desk_rm(tools: &Tools<'_>, name: &AgentName) -> Result<Outcome, HubError> {
    let agent = find_desk_agent(tools, name)?;
    let destination = PaneDestination::NewWorkspace {
        label: name.as_str().to_owned(),
    };
    tools.herdr.move_pane(&agent.pane_id, &destination)?;
    refresh_after_desk_change(tools, format!("{} is off the desk", name.as_str()))
}

/// Moves the agent's tab so it is the `place`th agent on the desk,
/// counting from 1, then fills the "go to" list again.
pub fn run_desk_move(
    tools: &Tools<'_>,
    name: &AgentName,
    place: usize,
) -> Result<Outcome, HubError> {
    let agent = find_desk_agent(tools, name)?;
    let agents = tools.herdr.list_agents()?;
    let tabs = tools.herdr.list_tabs(&agent.workspace_id)?;
    let desk_agents = list_desk_agents(&agents, &tabs);
    let Some(&(index, _)) = place.checked_sub(1).and_then(|at| desk_agents.get(at)) else {
        return Err(HubError::Usage(format!(
            "the place must be 1 to {}",
            desk_agents.len()
        )));
    };
    // Herdr inserts the tab before `index` and then closes the gap the
    // tab left, so a tab moving down must name the slot after its target.
    let source = tabs.iter().position(|tab_id| *tab_id == agent.tab_id);
    let insert_index = if source.is_some_and(|source| source < index) {
        index + 1
    } else {
        index
    };
    tools.herdr.move_tab(&agent.tab_id, insert_index)?;
    let done = format!("{} is at place {place} on the desk", name.as_str());
    refresh_after_desk_change(tools, done)
}

/// Sets the "go to" list as `sync_go_to_list` does, after a Herdr
/// restart dropped it. With `start_board`, it also types `hub board`
/// into the desk's board pane when that pane's shell runs nothing, as
/// in a desk Herdr just restored.
pub fn run_restore_sidebar(tools: &Tools<'_>, start_board: bool) -> Result<Outcome, HubError> {
    let synced = sync_go_to_list(tools)?;
    let mut changed = synced.changed;
    if synced.board_shown {
        changed.push(DESK_LABEL.to_owned());
    }
    let mut lines = vec![if changed.is_empty() {
        "nothing changed".to_owned()
    } else {
        format!("changed: {}", changed.join(" "))
    }];
    if start_board && let Some(board_pane_id) = &synced.board_pane_id {
        // The sidebar is already right, so a board that fails to start
        // does not fail the command.
        match start_board_if_quiet(tools.herdr, board_pane_id) {
            Ok(true) => lines.push("started the board in the desk".to_owned()),
            Ok(false) => {}
            Err(error) => lines.push(format!("could not start the board: {error}")),
        }
    }
    Ok(Outcome { lines })
}

/// Sets every `go_to` token on this machine, then sets Herdr's agent
/// filter. The front hub sorts first, then each desk agent by its tab,
/// and every other pane loses its token, so it shows only while it
/// waits on the user. It writes only the tokens that differ, since
/// `hub watch` runs it every 5 seconds. Herdr drops the filter on
/// restart, and setting it is one socket call, so it is set each time.
pub(crate) fn sync_go_to_list(tools: &Tools<'_>) -> Result<Synced, HubError> {
    let agents = tools.herdr.list_agent_panes()?;
    let machine_place = find_machine_place(tools.herdr)?;
    let front_hub = find_front_hub(tools.runs_folder, &agents)?;
    let synced = write_go_to_tokens(tools.herdr, &agents, front_hub, machine_place, Writes::All)?;
    tools.herdr.filter_agent_list()?;
    Ok(synced)
}

/// Sets the missing `go_to` tokens on every saved machine, for a
/// machine whose Herdr has no `pde.hub` plugin. This machine's Herdr
/// lists those machines' agents through its own filter. Returns a line
/// for each machine it could not reach, and goes on to the next.
pub(crate) fn restore_other_machines(tools: &Tools<'_>) -> Vec<String> {
    let labels = match tools.herdr.list_machines() {
        Ok(labels) => labels,
        Err(error) => return vec![format!("could not list the saved machines: {error}")],
    };
    let mut warnings = Vec::new();
    for label in labels {
        if let Err(error) = fill_in_go_to_tokens(tools.herdr.clone_for_machine(&label).as_ref()) {
            warnings.push(format!("could not restore the sidebar on {label}: {error}"));
        }
    }
    warnings
}

/// Sets the missing `go_to` tokens on another machine as its place 1,
/// and clears none, so it never undoes that machine's own pass. Only
/// that machine can read its run folders, so this tags the newest
/// `kanban-hub-N` only when no pane there has a front hub's token.
fn fill_in_go_to_tokens(herdr: &dyn Herdr) -> Result<(), HerdrError> {
    let agents = herdr.list_agent_panes()?;
    let has_front_hub = agents.iter().any(|agent| {
        agent
            .go_to
            .as_deref()
            .is_some_and(|go_to| go_to.starts_with(FRONT_HUB_PLACE))
    });
    let front_hub = if has_front_hub {
        None
    } else {
        find_newest_kanban_hub(&agents)
    };
    write_go_to_tokens(herdr, &agents, front_hub, 1, Writes::MissingOnly).map(drop)
}

/// Sets each agent pane's `go_to` token on one machine to the value
/// `choose_go_to` gives, writing only those that differ from Herdr's.
/// Then it reports the desk's board pane as agent `desk` when Herdr
/// does not show it. That check comes after the tags, so a pane this
/// pass tagged is never reported, since each report sets off the
/// `pde.hub` plugin's agent hook, which runs this pass again.
fn write_go_to_tokens(
    herdr: &dyn Herdr,
    agents: &[Agent],
    front_hub: Option<&Agent>,
    machine_place: u8,
    writes: Writes,
) -> Result<Synced, HerdrError> {
    let workspaces = herdr.list_workspaces()?;
    let desk = find_desk(&workspaces);
    let desk_tabs = match desk {
        Some(desk) => herdr.list_tabs(&desk.workspace_id)?,
        None => Vec::new(),
    };
    let mut changed = Vec::new();
    for agent in agents {
        let is_front_hub = front_hub.is_some_and(|hub| hub.pane_id == agent.pane_id);
        let desk_tab = desk
            .filter(|desk| desk.workspace_id == agent.workspace_id)
            .and_then(|_| desk_tabs.iter().position(|tab_id| *tab_id == agent.tab_id));
        let wanted = choose_go_to(is_front_hub, desk_tab, machine_place);
        let may_write = writes == Writes::All || agent.go_to.is_none();
        if agent.go_to == wanted || !may_write {
            continue;
        }
        match wanted.as_deref() {
            Some(go_to) if is_front_hub => herdr.tag_hub_pane(&agent.pane_id, go_to)?,
            go_to => herdr.set_go_to(&agent.pane_id, go_to)?,
        }
        let shown_name = if agent.name.is_empty() {
            &agent.pane_id
        } else {
            &agent.name
        };
        changed.push(shown_name.clone());
    }
    let Some(desk) = desk else {
        return Ok(Synced {
            changed,
            board_shown: false,
            board_pane_id: None,
        });
    };
    let board = herdr.find_desk_board(&desk.workspace_id)?;
    if !board.shown {
        herdr.show_desk_as_agent(&board.pane_id, &format_desk_go_to(machine_place, 0))?;
    }
    Ok(Synced {
        changed,
        board_shown: !board.shown,
        board_pane_id: Some(board.pane_id),
    })
}

/// Returns a pane's `go_to` value, which Herdr sorts as text. The front
/// hub gets `1-hub-M`, and a pane in the desk's tab NN, 00 first, gets
/// `2-desk-M-NN`, where M is the machine's place. Every other pane gets
/// `None`.
fn choose_go_to(is_front_hub: bool, desk_tab: Option<usize>, machine_place: u8) -> Option<String> {
    if is_front_hub {
        return Some(format!("{FRONT_HUB_PLACE}-{machine_place}"));
    }
    desk_tab.map(|tab| format_desk_go_to(machine_place, tab))
}

/// Returns the `go_to` value of a pane in the desk's tab `tab`.
fn format_desk_go_to(machine_place: u8, tab: usize) -> String {
    format!("{DESK_PLACE}-{machine_place}-{tab:02}")
}

/// Returns 0 when this machine lists saved machines, else 1. The
/// user's Herdr client lists them, and 0 keeps its rows above those of
/// a machine it reaches.
fn find_machine_place(herdr: &dyn Herdr) -> Result<u8, HerdrError> {
    Ok(if herdr.list_machines()?.is_empty() {
        1
    } else {
        0
    })
}

/// Returns this machine's front hub. That is the live hub recorded
/// last by `hub next` or `hub watch start` in a run folder that is not
/// an auto run's. Herdr may show it with no name or a hub name, but a
/// worker that loops on `hub next` has a worker name. With none, it is
/// the live `kanban-hub-N` with the highest N, since a hub of an older
/// build records nothing.
pub(crate) fn find_front_hub<'a>(
    runs_folder: &Path,
    agents: &'a [Agent],
) -> Result<Option<&'a Agent>, HubError> {
    let recorded = find_recorded_hub(runs_folder, agents)?;
    Ok(recorded.or_else(|| find_newest_kanban_hub(agents)))
}

/// Returns the live hub that `RunFolder::record_hub` recorded last in
/// a `herdr-hub.*` folder of `runs_folder`, leaving out auto runs.
fn find_recorded_hub<'a>(
    runs_folder: &Path,
    agents: &'a [Agent],
) -> Result<Option<&'a Agent>, HubError> {
    let wrap_file_error = |source| HubError::File {
        path: runs_folder.to_owned(),
        source,
    };
    let entries = match fs::read_dir(runs_folder) {
        Ok(entries) => entries,
        // No state folder means no hub ever ran here.
        Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(source) => return Err(wrap_file_error(source)),
    };
    let mut newest: Option<(SystemTime, &Agent)> = None;
    for entry in entries {
        let path = entry.map_err(wrap_file_error)?.path();
        let is_run_name = path
            .file_name()
            .is_some_and(|name| name.to_string_lossy().starts_with(RUN_PREFIX));
        if !is_run_name {
            continue;
        }
        // A file with that name is not a run folder.
        let Ok(run) = RunFolder::open(&path) else {
            continue;
        };
        if is_auto_run(&run)? {
            continue;
        }
        let Some((recorded, recorded_at)) = run.read_recorded_hub()? else {
            continue;
        };
        let Some(hub) = find_recorded_agent(&recorded, agents) else {
            continue;
        };
        if !has_front_hub_name(&hub.name) {
            continue;
        }
        if newest.is_none_or(|(newest_at, _)| recorded_at > newest_at) {
            newest = Some((recorded_at, hub));
        }
    }
    Ok(newest.map(|(_, hub)| hub))
}

/// Tells whether a recorded agent named `name` may be the front hub:
/// one with no name, or a hub that is neither an auto run's nor a
/// review. So a hub renamed to `notes` falls back to the newest
/// `kanban-hub-N`.
fn has_front_hub_name(name: &str) -> bool {
    name.is_empty() || (is_hub_name(name) && !is_auto_hub(name))
}

/// Returns the agent with the recorded session, or the agent in the
/// recorded pane when no session was recorded. OpenCode's background
/// service can give a command another pane's `HERDR_PANE_ID`, so a
/// recorded session wins over the pane.
fn find_recorded_agent<'a>(recorded: &RecordedHub, agents: &'a [Agent]) -> Option<&'a Agent> {
    match (&recorded.session_id, &recorded.pane_id) {
        (Some(session_id), _) => agents
            .iter()
            .find(|agent| agent.session_id.as_ref() == Some(session_id)),
        (None, Some(pane_id)) => agents.iter().find(|agent| agent.pane_id == *pane_id),
        (None, None) => None,
    }
}

/// Returns the live `kanban-hub-N` with the highest N.
fn find_newest_kanban_hub(agents: &[Agent]) -> Option<&Agent> {
    agents
        .iter()
        .filter_map(|agent| Some((parse_kanban_hub_number(&agent.name)?, agent)))
        .max_by_key(|(number, _)| *number)
        .map(|(_, agent)| agent)
}

/// Types `hub board` into the board pane when its shell runs nothing,
/// so the board never lands in an editor or a running command. Returns
/// whether it typed.
fn start_board_if_quiet(herdr: &dyn Herdr, board_pane_id: &str) -> Result<bool, HerdrError> {
    let info = herdr.read_process_info(board_pane_id)?;
    if info.foreground_group != info.shell_pid {
        return Ok(false);
    }
    herdr.run_in_pane(board_pane_id, BOARD_COMMAND)?;
    Ok(true)
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
    show_go_to_list(tools, &agents)?;
    Ok(Some(block))
}

/// Makes the desk workspace in `home` and returns its ID.
fn make_desk(tools: &Tools<'_>, home: &Path) -> Result<String, HubError> {
    let make_place = |label: &str| NewPlace {
        label: label.to_owned(),
        cwd: home.to_owned(),
        env: Vec::new(),
    };
    let desk = tools.herdr.create_workspace(&make_place(DESK_LABEL))?;
    tools.herdr.run_in_pane(&desk.root_pane_id, BOARD_COMMAND)?;
    tools
        .herdr
        .create_tab(&desk.workspace_id, &make_place("shell"))?;
    Ok(desk.workspace_id)
}

/// Returns the first workspace labelled `desk`.
pub(crate) fn find_desk(workspaces: &[Workspace]) -> Option<&Workspace> {
    workspaces
        .iter()
        .find(|workspace| workspace.label == DESK_LABEL)
}

/// Returns the open desk's workspace ID, for the `hub desk` steps.
fn find_desk_id(tools: &Tools<'_>) -> Result<String, HubError> {
    let workspaces = tools.herdr.list_workspaces()?;
    match find_desk(&workspaces) {
        Some(desk) => Ok(desk.workspace_id.clone()),
        None => Err(HubError::NotDone(
            "no desk is open; hub desk makes one".to_owned(),
        )),
    }
}

/// Returns the named agent, or stops when it is not in the desk.
fn find_desk_agent(tools: &Tools<'_>, name: &AgentName) -> Result<Agent, HubError> {
    let agent = tools.herdr.get_agent(name.as_str())?;
    if agent.workspace_id != find_desk_id(tools)? {
        return Err(HubError::NotDone(format!(
            "{} is not on the desk",
            name.as_str()
        )));
    }
    Ok(agent)
}

/// Returns the desk's named agents in tab order, each with its tab's
/// index among all the desk's tabs, the board's and shells' included.
fn list_desk_agents<'a>(agents: &'a [Agent], tabs: &[String]) -> Vec<(usize, &'a Agent)> {
    tabs.iter()
        .enumerate()
        .flat_map(|(index, tab_id)| {
            agents
                .iter()
                .filter(move |agent| agent.tab_id == *tab_id)
                .map(move |agent| (index, agent))
        })
        .collect()
}

/// Fills the "go to" list after a desk change that `done` says was
/// made, so a failure here does not hide that change.
fn refresh_after_desk_change(tools: &Tools<'_>, done: String) -> Result<Outcome, HubError> {
    match sync_go_to_list(tools) {
        Ok(_) => Ok(Outcome::from_line(done)),
        Err(error) => Err(HubError::PartlyDone {
            done,
            error: Box::new(error),
        }),
    }
}

/// Returns the workspace IDs that belong at the top, each once. The
/// kanban hubs' come first, then the desk, then the other hubs'. Two
/// hubs can share a workspace, and Herdr refuses a repeated ID.
fn list_top_block(hubs: &[&Agent], desk_id: Option<&str>) -> Vec<String> {
    let (kanban_hubs, other_hubs): (Vec<&Agent>, Vec<&Agent>) = hubs
        .iter()
        .copied()
        .partition(|hub| parse_kanban_hub_number(&hub.name).is_some());
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

/// Marks each flagged idle session again, then sets the "go to" list
/// as `sync_go_to_list` does.
fn show_go_to_list(tools: &Tools<'_>, agents: &[Agent]) -> Result<(), HubError> {
    // `run_flag` in older builds set the "needs you" label but not the token
    // the filter reads, so a flagged session gets its mark again.
    let flagged_idle = agents.iter().filter(|agent| {
        agent.marked && matches!(agent.status, AgentStatus::Idle | AgentStatus::Done)
    });
    for agent in flagged_idle {
        tools
            .herdr
            .mark_needs_you(&agent.pane_id, &[AgentStatus::Idle, AgentStatus::Done])?;
    }
    sync_go_to_list(tools).map(drop)
}

/// Returns the hubs ordered by their workspace's place, then by name.
/// Keeps the hub in this pane in the list, since a gathering command
/// usually runs in a hub's own pane and that hub's workspace moves too.
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
