// Each test file uses only some of these helpers.
#![allow(dead_code)]

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use hub::external::cards::{
    BlockedCard, Board, BoardCounts, CardError, CardKey, CardNote, Cards, ListedCard, Stage,
};
use hub::external::herdr::{
    Agent, AgentKind, AgentStart, AgentStatus, DeskBoard, Herdr, HerdrCode, HerdrError, NewPlace,
    Pane, PaneDestination, PlaceIds, ProcessInfo, Workspace,
};
use hub::external::opencode::OpenCodeDb;
use hub::external::requests::RequestFolder;
use hub::shared::Tools;
use hub::shared::parking::ParkedFolder;
use hub::shared::run_folder::RunFolder;
use jiff::Zoned;
use rusqlite::Connection;
use serde_json::Value;

/// Herdr, as the `Herdr` trait, with its agents and workspaces in
/// memory. It records each prompt, Enter press, mark, clear, hub tag,
/// `go_to` write, desk agent, agent filter, new workspace, new tab,
/// command run in a pane, started agent, closed tab, focus, workspace
/// move, pane move and tab move it is asked for. Its panes are the
/// agents' panes and those `on_list_panes` adds or changes, and an
/// agent whose tab was closed is gone.
#[derive(Default)]
pub struct InMemoryHerdr {
    /// The agents, with an empty name for one nobody named, which only
    /// `list_agent_panes` returns.
    pub agents: Vec<Agent>,
    /// Agents as they are once any prompt was tried, each in place of
    /// the agent with its name, such as one the user focused meanwhile.
    pub after_prompt: Vec<Agent>,
    /// Agents as they are once any agent was started, in the same way,
    /// such as the session a reopen resumed. Two agents with one name
    /// show one per `get_agent` call after the start, and the last one
    /// stays, as when Herdr learns a session ID late.
    pub after_start: Vec<Agent>,
    /// How many `get_agent` calls came after a start.
    pub start_reads: Cell<usize>,
    /// What `read_screen` shows, by agent name. Others show nothing.
    pub screens: BTreeMap<String, String>,
    /// The double each `clone_for_machine` label returns.
    pub machines: BTreeMap<String, InMemoryHerdr>,
    /// Runs inside each `prompt` call, as another process acting
    /// meanwhile.
    pub on_prompt: Option<Box<dyn Fn()>>,
    /// Runs inside each `list_panes` call, as another process acting
    /// between the tidy pass's verdicts and its actions. The panes it
    /// returns are added, such as a shell the user opened meanwhile.
    /// A returned pane replaces the listed pane with its ID.
    pub on_list_panes: Option<Box<dyn Fn() -> Vec<Pane>>>,
    /// The panes whose shell runs nothing. `read_process_info` names the
    /// test process as their shell, and it starts no job of its own.
    pub quiet_shells: Vec<String>,
    /// The panes whose shell has a program in front, such as an editor.
    /// `read_process_info` gives them a process group in front that is
    /// not the shell's.
    pub program_in_front: Vec<String>,
    /// The workspaces in sidebar order, top first. As Herdr does,
    /// `list_workspaces` leaves out one whose listed tabs have all closed.
    pub workspaces: Vec<Workspace>,
    /// Each workspace's tab IDs in tab order, by workspace ID, before
    /// any `move_tab` or `close_tab`. A workspace not listed has no tabs.
    pub tabs: BTreeMap<String, Vec<String>>,
    /// Each `mark_needs_you` call, as its pane ID and states.
    pub marks: RefCell<Vec<(String, Vec<AgentStatus>)>>,
    /// The pane ID of each `clear_marks` call.
    pub cleared: RefCell<Vec<String>>,
    /// Each `tag_hub_pane` call, as its pane ID and `go_to` value.
    pub tagged_hubs: RefCell<Vec<(String, String)>>,
    /// Each `show_waiting_count` call, as its pane ID and count.
    pub waiting_counts: RefCell<Vec<(String, usize)>>,
    /// Each `set_go_to` call, as its pane ID and value, `None` for a
    /// clear.
    pub go_to_writes: RefCell<Vec<(String, Option<String>)>>,
    /// The desk workspaces whose board pane already shows in the
    /// desk's place. `find_desk_board` names each desk's board pane
    /// `<workspace>:p-board`.
    pub shown_desks: Vec<String>,
    /// Each `show_desk_as_agent` call, as its pane ID and `go_to` value.
    pub desk_agents: RefCell<Vec<(String, String)>>,
    /// How many times `filter_agent_list` was called.
    pub agent_filters: Cell<usize>,
    /// The agent name of each `focus_agent` call.
    pub focused: RefCell<Vec<String>>,
    /// Each `move_workspaces` call, as the IDs moved and the ID they
    /// went before.
    pub moves: RefCell<Vec<(Vec<String>, String)>>,
    /// Each `move_pane` call, as the pane ID and its destination.
    pub moved_panes: RefCell<Vec<(String, PaneDestination)>>,
    /// Each `move_tab` call, as the tab ID and the index it goes
    /// before. `list_tabs` gives the tabs with these moves made.
    pub moved_tabs: RefCell<Vec<(String, usize)>>,
    /// Each `prompt` call, as the agent name and the text, failed ones
    /// included.
    pub prompts: RefCell<Vec<(String, String)>>,
    /// The agent name of each `press_enter` call.
    pub pressed: RefCell<Vec<String>>,
    /// Each `create_workspace` call.
    pub created_workspaces: RefCell<Vec<NewPlace>>,
    /// Each `create_tab` call, as the workspace ID and the place.
    pub created_tabs: RefCell<Vec<(String, NewPlace)>>,
    /// Each `run_in_pane` call, as the pane ID and the command.
    pub ran_in_panes: RefCell<Vec<(String, String)>>,
    /// Each `start_agent` call.
    pub started: RefCell<Vec<AgentStart>>,
    /// The tab ID of each `close_tab` call.
    pub closed_tabs: RefCell<Vec<String>>,
    /// Failures the next call of a method returns, by method name.
    pub failures: RefCell<Vec<(&'static str, HerdrError)>>,
}

impl InMemoryHerdr {
    /// Makes the next call of `method`, such as `"prompt"`, return
    /// `error`.
    pub fn fail_next(&self, method: &'static str, error: HerdrError) {
        self.failures.borrow_mut().push((method, error));
    }

    /// Returns the agents as they are now, with `after_prompt` applied
    /// once a prompt was tried, `after_start` once an agent was started,
    /// and the agents of closed tabs left out.
    fn list_current_agents(&self) -> Vec<Agent> {
        let mut agents = self.agents.clone();
        let mut changes = Vec::new();
        if !self.prompts.borrow().is_empty() {
            changes.extend(&self.after_prompt);
        }
        if !self.started.borrow().is_empty() {
            let read = self.start_reads.get();
            let mut shown: BTreeMap<&str, usize> = BTreeMap::new();
            for agent in &self.after_start {
                let index = shown.entry(agent.name.as_str()).or_default();
                if *index <= read {
                    changes.push(agent);
                }
                *index += 1;
            }
        }
        for changed in changes {
            agents.retain(|agent| agent.name != changed.name);
            agents.push(changed.clone());
        }
        let closed = self.closed_tabs.borrow();
        agents.retain(|agent| !closed.contains(&agent.tab_id));
        agents
    }

    /// Returns the failure set for `method`, once.
    fn take_failure(&self, method: &str) -> Result<(), HerdrError> {
        let mut failures = self.failures.borrow_mut();
        match failures.iter().position(|(name, _)| *name == method) {
            Some(place) => Err(failures.remove(place).1),
            None => Ok(()),
        }
    }
}

impl Herdr for InMemoryHerdr {
    fn list_agents(&self) -> Result<Vec<Agent>, HerdrError> {
        self.take_failure("list_agents")?;
        // As `HerdrCli` does, the list leaves out panes with no name.
        let mut agents = self.list_current_agents();
        agents.retain(|agent| !agent.name.is_empty());
        Ok(agents)
    }

    fn list_agent_panes(&self) -> Result<Vec<Agent>, HerdrError> {
        self.take_failure("list_agent_panes")?;
        Ok(self.list_current_agents())
    }

    fn get_agent(&self, target: &str) -> Result<Agent, HerdrError> {
        self.take_failure("get_agent")?;
        let agents = self.list_current_agents();
        if !self.started.borrow().is_empty() {
            self.start_reads.set(self.start_reads.get() + 1);
        }
        agents
            .into_iter()
            .find(|agent| agent.name == target || agent.pane_id == target)
            .ok_or_else(|| HerdrError::Refused {
                code: HerdrCode::AgentNotFound,
                message: format!("agent target {target} not found"),
            })
    }

    fn prompt(&self, name: &str, text: &str) -> Result<(), HerdrError> {
        self.prompts
            .borrow_mut()
            .push((name.to_owned(), text.to_owned()));
        if let Some(on_prompt) = &self.on_prompt {
            on_prompt();
        }
        self.take_failure("prompt")
    }

    fn read_screen(&self, name: &str) -> Result<String, HerdrError> {
        self.take_failure("read_screen")?;
        Ok(self.screens.get(name).cloned().unwrap_or_default())
    }

    fn press_enter(&self, name: &str) -> Result<(), HerdrError> {
        self.take_failure("press_enter")?;
        self.pressed.borrow_mut().push(name.to_owned());
        Ok(())
    }

    fn wait_until_started(&self, _name: &str) -> Result<(), HerdrError> {
        self.take_failure("wait_until_started")
    }

    fn mark_needs_you(&self, pane_id: &str, states: &[AgentStatus]) -> Result<(), HerdrError> {
        self.take_failure("mark_needs_you")?;
        self.marks
            .borrow_mut()
            .push((pane_id.to_owned(), states.to_vec()));
        Ok(())
    }

    fn clear_marks(&self, pane_id: &str) -> Result<(), HerdrError> {
        self.take_failure("clear_marks")?;
        self.cleared.borrow_mut().push(pane_id.to_owned());
        Ok(())
    }

    fn tag_hub_pane(&self, pane_id: &str, go_to: &str) -> Result<(), HerdrError> {
        self.take_failure("tag_hub_pane")?;
        self.tagged_hubs
            .borrow_mut()
            .push((pane_id.to_owned(), go_to.to_owned()));
        Ok(())
    }

    fn show_waiting_count(&self, pane_id: &str, count: usize) -> Result<(), HerdrError> {
        self.take_failure("show_waiting_count")?;
        self.waiting_counts
            .borrow_mut()
            .push((pane_id.to_owned(), count));
        Ok(())
    }

    fn set_go_to(&self, pane_id: &str, go_to: Option<&str>) -> Result<(), HerdrError> {
        self.take_failure("set_go_to")?;
        self.go_to_writes
            .borrow_mut()
            .push((pane_id.to_owned(), go_to.map(str::to_owned)));
        Ok(())
    }

    fn find_desk_board(&self, workspace_id: &str) -> Result<DeskBoard, HerdrError> {
        self.take_failure("find_desk_board")?;
        Ok(DeskBoard {
            pane_id: format!("{workspace_id}:p-board"),
            shown: self.shown_desks.iter().any(|shown| shown == workspace_id),
        })
    }

    fn show_desk_as_agent(&self, board_pane_id: &str, go_to: &str) -> Result<(), HerdrError> {
        self.take_failure("show_desk_as_agent")?;
        self.desk_agents
            .borrow_mut()
            .push((board_pane_id.to_owned(), go_to.to_owned()));
        Ok(())
    }

    fn filter_agent_list(&self) -> Result<(), HerdrError> {
        self.take_failure("filter_agent_list")?;
        self.agent_filters.set(self.agent_filters.get() + 1);
        Ok(())
    }

    fn list_panes(&self) -> Result<Vec<Pane>, HerdrError> {
        self.take_failure("list_panes")?;
        let added = match &self.on_list_panes {
            Some(on_list_panes) => on_list_panes(),
            None => Vec::new(),
        };
        let mut panes: Vec<Pane> = self
            .list_current_agents()
            .into_iter()
            .map(|agent| Pane {
                pane_id: agent.pane_id,
                tab_id: agent.tab_id,
                focused: agent.focused,
                has_agent: true,
            })
            .collect();
        panes.retain(|pane| !added.iter().any(|new| new.pane_id == pane.pane_id));
        panes.extend(added);
        Ok(panes)
    }

    fn list_tabs(&self, workspace_id: &str) -> Result<Vec<String>, HerdrError> {
        self.take_failure("list_tabs")?;
        let mut tabs = self.tabs.get(workspace_id).cloned().unwrap_or_default();
        tabs.retain(|tab| !self.closed_tabs.borrow().contains(tab));
        for (tab_id, insert_index) in self.moved_tabs.borrow().iter() {
            let Some(source) = tabs.iter().position(|tab| tab == tab_id) else {
                continue;
            };
            // As Herdr's `Workspace::move_tab`: the tab goes before
            // `insert_index`, which shifts down once the tab leaves.
            let target = if source < *insert_index {
                insert_index - 1
            } else {
                *insert_index
            };
            let tab = tabs.remove(source);
            tabs.insert(target.min(tabs.len()), tab);
        }
        Ok(tabs)
    }

    /// Fails for a pane in neither `quiet_shells` nor `program_in_front`.
    fn read_process_info(&self, pane_id: &str) -> Result<ProcessInfo, HerdrError> {
        let is_listed = |panes: &[String]| panes.iter().any(|listed| listed == pane_id);
        let shell_pid = std::process::id();
        let foreground_group = if is_listed(&self.quiet_shells) {
            shell_pid
        } else if is_listed(&self.program_in_front) {
            shell_pid + 1
        } else {
            return Err(HerdrError::Failed(format!("no shell in pane {pane_id}")));
        };
        Ok(ProcessInfo {
            shell_pid,
            foreground_group,
        })
    }

    fn list_workspaces(&self) -> Result<Vec<Workspace>, HerdrError> {
        self.take_failure("list_workspaces")?;
        let closed = self.closed_tabs.borrow();
        let mut workspaces = self.workspaces.clone();
        workspaces.retain(|workspace| {
            self.tabs
                .get(&workspace.workspace_id)
                .is_none_or(|tabs| tabs.is_empty() || tabs.iter().any(|tab| !closed.contains(tab)))
        });
        Ok(workspaces)
    }

    /// Answers with workspace `new` and root pane `new:p-<label>`.
    fn create_workspace(&self, place: &NewPlace) -> Result<PlaceIds, HerdrError> {
        self.take_failure("create_workspace")?;
        self.created_workspaces.borrow_mut().push(place.clone());
        Ok(PlaceIds {
            root_pane_id: format!("new:p-{}", place.label),
            workspace_id: "new".to_owned(),
        })
    }

    /// Answers with root pane `<workspace>:p-<label>`.
    fn create_tab(&self, workspace_id: &str, place: &NewPlace) -> Result<PlaceIds, HerdrError> {
        self.take_failure("create_tab")?;
        self.created_tabs
            .borrow_mut()
            .push((workspace_id.to_owned(), place.clone()));
        Ok(PlaceIds {
            root_pane_id: format!("{workspace_id}:p-{}", place.label),
            workspace_id: workspace_id.to_owned(),
        })
    }

    fn run_in_pane(&self, pane_id: &str, command: &str) -> Result<(), HerdrError> {
        self.take_failure("run_in_pane")?;
        self.ran_in_panes
            .borrow_mut()
            .push((pane_id.to_owned(), command.to_owned()));
        Ok(())
    }

    fn start_agent(&self, start: &AgentStart) -> Result<(), HerdrError> {
        self.take_failure("start_agent")?;
        self.started.borrow_mut().push(start.clone());
        Ok(())
    }

    fn close_tab(&self, tab_id: &str) -> Result<(), HerdrError> {
        self.take_failure("close_tab")?;
        self.closed_tabs.borrow_mut().push(tab_id.to_owned());
        Ok(())
    }

    fn focus_agent(&self, name: &str) -> Result<(), HerdrError> {
        self.take_failure("focus_agent")?;
        self.focused.borrow_mut().push(name.to_owned());
        Ok(())
    }

    fn move_workspaces(&self, ids: &[String], before_id: &str) -> Result<(), HerdrError> {
        self.take_failure("move_workspaces")?;
        self.moves
            .borrow_mut()
            .push((ids.to_vec(), before_id.to_owned()));
        Ok(())
    }

    fn move_pane(&self, pane_id: &str, destination: &PaneDestination) -> Result<(), HerdrError> {
        self.take_failure("move_pane")?;
        self.moved_panes
            .borrow_mut()
            .push((pane_id.to_owned(), destination.clone()));
        Ok(())
    }

    fn move_tab(&self, tab_id: &str, index: usize) -> Result<(), HerdrError> {
        self.take_failure("move_tab")?;
        self.moved_tabs
            .borrow_mut()
            .push((tab_id.to_owned(), index));
        Ok(())
    }

    /// Returns the double in `machines` for the label, so a test reads
    /// what that machine's Herdr was asked.
    fn clone_for_machine(&self, label: &str) -> Box<dyn Herdr + '_> {
        let machine = self
            .machines
            .get(label)
            .unwrap_or_else(|| panic!("no double for machine {label}"));
        Box::new(machine)
    }

    /// Names each label in `machines`.
    fn list_machines(&self) -> Result<Vec<String>, HerdrError> {
        self.take_failure("list_machines")?;
        Ok(self.machines.keys().cloned().collect())
    }
}

/// Lets `clone_for_machine` return a borrowed double. Each call goes to the
/// double itself.
impl Herdr for &InMemoryHerdr {
    fn list_agents(&self) -> Result<Vec<Agent>, HerdrError> {
        (**self).list_agents()
    }

    fn list_agent_panes(&self) -> Result<Vec<Agent>, HerdrError> {
        (**self).list_agent_panes()
    }

    fn get_agent(&self, target: &str) -> Result<Agent, HerdrError> {
        (**self).get_agent(target)
    }

    fn prompt(&self, name: &str, text: &str) -> Result<(), HerdrError> {
        (**self).prompt(name, text)
    }

    fn read_screen(&self, name: &str) -> Result<String, HerdrError> {
        (**self).read_screen(name)
    }

    fn press_enter(&self, name: &str) -> Result<(), HerdrError> {
        (**self).press_enter(name)
    }

    fn wait_until_started(&self, name: &str) -> Result<(), HerdrError> {
        (**self).wait_until_started(name)
    }

    fn mark_needs_you(&self, pane_id: &str, states: &[AgentStatus]) -> Result<(), HerdrError> {
        (**self).mark_needs_you(pane_id, states)
    }

    fn clear_marks(&self, pane_id: &str) -> Result<(), HerdrError> {
        (**self).clear_marks(pane_id)
    }

    fn tag_hub_pane(&self, pane_id: &str, go_to: &str) -> Result<(), HerdrError> {
        (**self).tag_hub_pane(pane_id, go_to)
    }

    fn show_waiting_count(&self, pane_id: &str, count: usize) -> Result<(), HerdrError> {
        (**self).show_waiting_count(pane_id, count)
    }

    fn set_go_to(&self, pane_id: &str, go_to: Option<&str>) -> Result<(), HerdrError> {
        (**self).set_go_to(pane_id, go_to)
    }

    fn find_desk_board(&self, workspace_id: &str) -> Result<DeskBoard, HerdrError> {
        (**self).find_desk_board(workspace_id)
    }

    fn show_desk_as_agent(&self, board_pane_id: &str, go_to: &str) -> Result<(), HerdrError> {
        (**self).show_desk_as_agent(board_pane_id, go_to)
    }

    fn filter_agent_list(&self) -> Result<(), HerdrError> {
        (**self).filter_agent_list()
    }

    fn list_panes(&self) -> Result<Vec<Pane>, HerdrError> {
        (**self).list_panes()
    }

    fn list_tabs(&self, workspace_id: &str) -> Result<Vec<String>, HerdrError> {
        (**self).list_tabs(workspace_id)
    }

    fn read_process_info(&self, pane_id: &str) -> Result<ProcessInfo, HerdrError> {
        (**self).read_process_info(pane_id)
    }

    fn list_workspaces(&self) -> Result<Vec<Workspace>, HerdrError> {
        (**self).list_workspaces()
    }

    fn create_workspace(&self, place: &NewPlace) -> Result<PlaceIds, HerdrError> {
        (**self).create_workspace(place)
    }

    fn create_tab(&self, workspace_id: &str, place: &NewPlace) -> Result<PlaceIds, HerdrError> {
        (**self).create_tab(workspace_id, place)
    }

    fn run_in_pane(&self, pane_id: &str, command: &str) -> Result<(), HerdrError> {
        (**self).run_in_pane(pane_id, command)
    }

    fn start_agent(&self, start: &AgentStart) -> Result<(), HerdrError> {
        (**self).start_agent(start)
    }

    fn close_tab(&self, tab_id: &str) -> Result<(), HerdrError> {
        (**self).close_tab(tab_id)
    }

    fn focus_agent(&self, name: &str) -> Result<(), HerdrError> {
        (**self).focus_agent(name)
    }

    fn move_workspaces(&self, ids: &[String], before_id: &str) -> Result<(), HerdrError> {
        (**self).move_workspaces(ids, before_id)
    }

    fn move_pane(&self, pane_id: &str, destination: &PaneDestination) -> Result<(), HerdrError> {
        (**self).move_pane(pane_id, destination)
    }

    fn move_tab(&self, tab_id: &str, index: usize) -> Result<(), HerdrError> {
        (**self).move_tab(tab_id, index)
    }

    fn clone_for_machine(&self, label: &str) -> Box<dyn Herdr + '_> {
        (**self).clone_for_machine(label)
    }

    fn list_machines(&self) -> Result<Vec<String>, HerdrError> {
        (**self).list_machines()
    }
}

/// The `Cards` trait in memory, with the board, the stages, the cards
/// ready to start and each card's note. It records each note it is
/// asked to add, each session it links, each card it moves into
/// progress, and each item it is asked to add or remove. A write does
/// not change `card_notes`.
#[derive(Default)]
pub struct InMemoryCards {
    /// Each card's stage, as `Cards::list_cards` gives it.
    pub stages: BTreeMap<String, Stage>,
    /// The keys of cards ready to start, in the order
    /// `Cards::list_todo` gives.
    pub todo: Vec<String>,
    /// What `Cards::read_card` gives, by card key. Any other card fails
    /// as `CardCli` does for a missing card. `list_cards` gives a
    /// card's sessions and open questions from here too.
    pub card_notes: BTreeMap<String, CardNote>,
    /// What `Cards::read_card` gives for a card once any item was
    /// written, as after a hand edit in Obsidian. Cards not here keep
    /// their `card_notes`.
    pub notes_after_write: BTreeMap<String, CardNote>,
    /// `None` answers as `CardCli` does without a vault.
    pub board: Option<BoardCounts>,
    /// The cards waiting on people outside the team that `read_board`
    /// gives with `board`.
    pub blocked: Vec<BlockedCard>,
    /// Each `add_note` call, as the card key and the text.
    pub notes: RefCell<Vec<(String, String)>>,
    /// When set, `add_note` fails with this message and records nothing.
    pub note_error: Option<String>,
    /// Each `link_session` call, as the card key and the agent name.
    pub links: RefCell<Vec<(String, String)>>,
    /// When set, `link_session` fails with this message and records
    /// nothing.
    pub link_error: Option<String>,
    /// Runs inside each `link_session` call that succeeds, as another
    /// process acting while the card is linked.
    pub on_link: Option<Box<dyn Fn()>>,
    /// When set, `list_cards` and `read_card` fail with this message,
    /// as `CardCli` does without a vault.
    pub read_error: Option<String>,
    /// The key of each `move_to_active` call.
    pub moves: RefCell<Vec<String>>,
    /// When set, `move_to_active` fails with this message and records
    /// nothing, as `CardCli` does past the work limit.
    pub move_error: Option<String>,
    /// Each item write, in call order, such as
    /// `add DEMO-KB-007 Open Questions: TEXT` or
    /// `remove DEMO-KB-007 Q2`.
    pub writes: RefCell<Vec<String>>,
    /// When set, `add_item` fails with this message and records
    /// nothing.
    pub add_error: Option<String>,
    /// When set, `remove_question` fails with this message and records
    /// nothing.
    pub remove_error: Option<String>,
}

impl InMemoryCards {
    /// Returns the failure `read_error` sets, if any.
    fn check_read_error(&self) -> Result<(), CardError> {
        match &self.read_error {
            Some(message) => Err(CardError::Failed(message.clone())),
            None => Ok(()),
        }
    }
}

impl Cards for InMemoryCards {
    fn list_cards(&self) -> Result<Vec<ListedCard>, CardError> {
        self.check_read_error()?;
        Ok(self
            .stages
            .iter()
            .map(|(key, stage)| {
                let note = self.card_notes.get(key).cloned().unwrap_or_default();
                ListedCard {
                    key: key.parse().unwrap(),
                    stage: *stage,
                    project: note.project,
                    title: note.title,
                    sessions: note.sessions,
                    open_questions: note.open_questions,
                }
            })
            .collect())
    }

    fn list_todo(&self) -> Result<Vec<CardKey>, CardError> {
        Ok(self.todo.iter().map(|key| key.parse().unwrap()).collect())
    }

    fn read_card(&self, key: &CardKey) -> Result<CardNote, CardError> {
        self.check_read_error()?;
        let after_write = self.notes_after_write.get(key.as_str());
        let has_written = !self.writes.borrow().is_empty();
        after_write
            .filter(|_| has_written)
            .or_else(|| self.card_notes.get(key.as_str()))
            .cloned()
            .ok_or_else(|| CardError::Failed(format!("card: no card {}", key.as_str())))
    }

    fn read_board(&self) -> Result<Board, CardError> {
        let counts = self
            .board
            .clone()
            .ok_or_else(|| CardError::Failed("card: no default vault".to_owned()))?;
        Ok(Board {
            counts,
            blocked: self.blocked.clone(),
        })
    }

    fn add_note(&self, key: &CardKey, text: &str) -> Result<(), CardError> {
        if let Some(message) = &self.note_error {
            return Err(CardError::Failed(message.clone()));
        }
        self.notes
            .borrow_mut()
            .push((key.as_str().to_owned(), text.to_owned()));
        Ok(())
    }

    fn link_session(&self, key: &CardKey, agent_name: &str) -> Result<(), CardError> {
        if let Some(message) = &self.link_error {
            return Err(CardError::Failed(message.clone()));
        }
        self.links
            .borrow_mut()
            .push((key.as_str().to_owned(), agent_name.to_owned()));
        if let Some(on_link) = &self.on_link {
            on_link();
        }
        Ok(())
    }

    fn move_to_active(&self, key: &CardKey) -> Result<(), CardError> {
        if let Some(message) = &self.move_error {
            return Err(CardError::Failed(message.clone()));
        }
        self.moves.borrow_mut().push(key.as_str().to_owned());
        Ok(())
    }

    fn add_item(&self, key: &CardKey, section: &str, text: &str) -> Result<(), CardError> {
        if let Some(message) = &self.add_error {
            return Err(CardError::Failed(message.clone()));
        }
        self.writes
            .borrow_mut()
            .push(format!("add {} {section}: {text}", key.as_str()));
        Ok(())
    }

    fn remove_question(&self, key: &CardKey, number: u32) -> Result<(), CardError> {
        if let Some(message) = &self.remove_error {
            return Err(CardError::Failed(message.clone()));
        }
        self.writes
            .borrow_mut()
            .push(format!("remove {} Q{number}", key.as_str()));
        Ok(())
    }
}

/// Every external system for a command test. It holds the doubles and a
/// temporary folder, which is also the runs folder that stands in for
/// `~/.local/state`. The folder holds the run folder `run`; the state
/// folder `state`, with its request folder `pr-write` and parked folder
/// `hub/parked`; the OpenCode database `opencode.db`; and the `prs`
/// tool's list `prs/list.json`. The request and parked folders and the
/// list do not exist until a test writes to them. A test that uses
/// it must not make another `TempFolder`. Its own `TempFolder` already
/// holds the process lock, so a second one would wait for it forever.
pub struct TestTools {
    pub herdr: InMemoryHerdr,
    pub cards: InMemoryCards,
    /// The hub's own pane, as `HERDR_PANE_ID` gives it.
    pub own_pane: Option<String>,
    /// The hub's own session, as `OPENCODE_SESSION_ID` gives it.
    pub own_session: Option<String>,
    pub pr_list: PathBuf,
    opencode: OpenCodeDb,
    requests: RequestFolder,
    parked: ParkedFolder,
    state_folder: PathBuf,
    folder: TempFolder,
}

impl TestTools {
    /// Returns empty doubles and an empty run folder, outside Herdr.
    pub fn new() -> Self {
        let folder = TempFolder::new();
        let state_folder = folder.path().join("state");
        fs::create_dir(folder.path().join("run")).unwrap();
        fs::create_dir(&state_folder).unwrap();
        TestTools {
            herdr: InMemoryHerdr::default(),
            cards: InMemoryCards::default(),
            own_pane: None,
            own_session: None,
            pr_list: folder.path().join("prs/list.json"),
            opencode: OpenCodeDb::new(folder.path().join("opencode.db")),
            requests: RequestFolder::new(state_folder.join("pr-write")),
            parked: ParkedFolder::new(state_folder.join("hub/parked")),
            state_folder,
            folder,
        }
    }

    /// Lends every system to one command call.
    pub fn tools(&self) -> Tools<'_> {
        Tools {
            herdr: &self.herdr,
            cards: &self.cards,
            opencode: &self.opencode,
            requests: &self.requests,
            parked: &self.parked,
            own_pane: self.own_pane.as_deref(),
            own_session: self.own_session.as_deref(),
            state_folder: &self.state_folder,
            runs_folder: self.folder.path(),
            pr_list: &self.pr_list,
            reopen_poll: Duration::ZERO,
        }
    }

    /// The temporary folder that holds everything else.
    pub fn path(&self) -> &Path {
        &self.folder.path
    }

    /// The run folder's path, for writing and reading its files.
    pub fn run_path(&self) -> PathBuf {
        self.folder.path().join("run")
    }

    /// The run folder to pass to a command.
    pub fn run(&self) -> RunFolder {
        RunFolder::open(&self.run_path()).unwrap()
    }

    /// Writes a stand-in program as `TempFolder::write_script` does.
    pub fn write_script(&self, name: &str, stdout: &str, stderr: &str, exit_code: i32) -> PathBuf {
        self.folder.write_script(name, stdout, stderr, exit_code)
    }

    /// The state folder that stands in for `~/.local/state/pde`.
    pub fn state_folder(&self) -> &Path {
        &self.state_folder
    }

    /// The OpenCode database file, which no test has made yet.
    pub fn build_opencode_path(&self) -> PathBuf {
        self.folder.path().join("opencode.db")
    }
}

/// One `session_message` row of the OpenCode database.
pub struct OpenCodeRow<'a> {
    pub session: &'a str,
    /// The row's `type`, such as `assistant`, `user` or `idle`.
    pub kind: &'a str,
    pub updated_ms: i64,
    pub data: Value,
}

/// A real SQLite file at `path` with OpenCode's message table and
/// these rows, oldest first.
pub fn write_opencode_db(path: &Path, rows: &[OpenCodeRow<'_>]) {
    let database = Connection::open(path).unwrap();
    database
        .execute(
            "create table session_message (id text, session_id text, type text, \
             seq integer, time_created integer, time_updated integer, data text)",
            [],
        )
        .unwrap();
    for (seq, row) in rows.iter().enumerate() {
        database
            .execute(
                "insert into session_message values (?1, ?2, ?3, ?4, ?5, ?5, ?6)",
                rusqlite::params![
                    format!("msg_{seq}"),
                    row.session,
                    row.kind,
                    seq as i64,
                    row.updated_ms,
                    row.data.to_string(),
                ],
            )
            .unwrap();
    }
}

/// Adds a row to the OpenCode database's `session_v2` table at
/// `path`, making the table first when it is missing.
pub fn write_opencode_session(path: &Path, session: &str, directory: &Path) {
    let database = open_session_table(path);
    database
        .execute(
            "insert into session_v2 (id, directory) values (?1, ?2)",
            rusqlite::params![session, directory.to_string_lossy()],
        )
        .unwrap();
}

/// Adds a `session_v2` row with the session's start and last change,
/// in milliseconds, and no folder.
pub fn write_opencode_times(path: &Path, session: &str, created_ms: i64, updated_ms: i64) {
    let database = open_session_table(path);
    database
        .execute(
            "insert into session_v2 (id, time_created, time_updated) values (?1, ?2, ?3)",
            rusqlite::params![session, created_ms, updated_ms],
        )
        .unwrap();
}

/// Opens the database at `path` with the columns of `session_v2` the
/// hub reads, making the table first when it is missing.
fn open_session_table(path: &Path) -> Connection {
    let database = Connection::open(path).unwrap();
    database
        .execute(
            "create table if not exists session_v2 (id text primary key, directory text, \
             time_created integer, time_updated integer)",
            [],
        )
        .unwrap();
    database
}

/// A fixed local time, such as `2026-11-11T22:14:00+00:00[UTC]`.
pub fn make_time(text: &str) -> Zoned {
    text.parse().unwrap()
}

/// Returns each line of the run folder's event log as JSON.
pub fn read_events(run_path: &Path) -> Vec<Value> {
    let log = fs::read_to_string(run_path.join("events.jsonl")).unwrap_or_default();
    log.lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

/// One unfocused OpenCode agent in workspace `w1`, with pane
/// `w1:<name>`, tab `w1:t-<name>`, state change 1, no session ID, no
/// mark, no folders, no machine and no `go_to` token.
pub fn make_agent(name: &str, status: AgentStatus) -> Agent {
    Agent {
        name: name.to_owned(),
        status,
        kind: AgentKind::OpenCode,
        session_id: None,
        pane_id: format!("w1:{name}"),
        tab_id: format!("w1:t-{name}"),
        workspace_id: "w1".to_owned(),
        focused: false,
        marked: false,
        state_seq: 1,
        folders: Vec::new(),
        machine: None,
        go_to: None,
    }
}

/// A Herdr refusal with this code, as Herdr gives one.
pub fn make_refusal(code: HerdrCode) -> HerdrError {
    HerdrError::Refused {
        code,
        message: "Herdr said no".to_owned(),
    }
}

/// Workspaces with these IDs and no labels, in this sidebar order.
pub fn make_workspaces(ids: &[&str]) -> Vec<Workspace> {
    ids.iter()
        .map(|id| Workspace {
            workspace_id: (*id).to_owned(),
            label: String::new(),
        })
        .collect()
}

/// Lets one test at a time write scripts and start processes, because a
/// process started while another test's script is open for writing keeps
/// that file open, and running the script then fails with "Text file busy".
static PROCESS_LOCK: Mutex<()> = Mutex::new(());

/// A temporary folder, deleted on drop. It holds `PROCESS_LOCK` until
/// then, so a test that writes a script or starts a process makes one
/// first, and never holds two at once.
pub struct TempFolder {
    path: PathBuf,
    _process_lock: MutexGuard<'static, ()>,
}

impl TempFolder {
    /// Makes an empty folder and takes the process lock.
    pub fn new() -> Self {
        // A test that panicked while holding the lock must not fail the
        // tests after it.
        let process_lock = PROCESS_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
        let output = Command::new("mktemp").arg("-d").output().unwrap();
        assert!(output.status.success());
        let path = String::from_utf8(output.stdout).unwrap();
        Self {
            path: PathBuf::from(path.trim_end_matches('\n')),
            _process_lock: process_lock,
        }
    }

    /// Returns the folder's path.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Writes a script `name` in this folder that logs its arguments,
    /// prints `stdout` and `stderr`, and exits with `exit_code`, as a
    /// stand-in for a program such as `herdr`. Returns its path.
    pub fn write_script(&self, name: &str, stdout: &str, stderr: &str, exit_code: i32) -> PathBuf {
        let script = self.path.join(name);
        fs::write(script.with_extension("out"), stdout).unwrap();
        fs::write(script.with_extension("err"), stderr).unwrap();
        // Each argument ends in a NUL byte and each call in a record
        // separator byte, so an argument may hold spaces or newlines.
        let body = format!(
            "#!/bin/sh\nprintf '%s\\0' \"$@\" >> '{log}'\nprintf '\\036' >> '{log}'\n\
             cat '{out}'\ncat '{err}' >&2\nexit {exit_code}\n",
            log = script.with_extension("log").display(),
            out = script.with_extension("out").display(),
            err = script.with_extension("err").display(),
        );
        fs::write(&script, body).unwrap();
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
        script
    }
}

impl Drop for TempFolder {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

/// Returns the arguments of each call a `TempFolder::write_script`
/// script logged, in order.
pub fn read_script_calls(script: &Path) -> Vec<Vec<String>> {
    let log = fs::read_to_string(script.with_extension("log")).unwrap_or_default();
    log.split_terminator('\u{1e}')
        .map(|call| call.split_terminator('\0').map(str::to_owned).collect())
        .collect()
}

/// Runs git in `folder` as a made-up user, with no signing.
pub fn run_git(folder: &Path, args: &[&str]) {
    let output = Command::new("git")
        .args([
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.com",
            "-c",
            "commit.gpgsign=false",
            "-C",
        ])
        .arg(folder)
        .args(args)
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
}

/// Makes a main checkout in `folder/main` with a pushed card
/// worktree beside it, `folder/card-wt` on `feat/demo-kb-001-fix`.
/// Returns the main checkout.
pub fn make_card_repo(folder: &Path) -> PathBuf {
    fs::create_dir(folder).unwrap();
    run_git(folder, &["init", "--bare", "origin.git"]);
    run_git(folder, &["clone", "origin.git", "main"]);
    let main = folder.join("main");
    run_git(&main, &["commit", "--allow-empty", "-m", "start"]);
    run_git(&main, &["push", "origin", "HEAD"]);
    run_git(
        &main,
        &[
            "worktree",
            "add",
            "-b",
            "feat/demo-kb-001-fix",
            "../card-wt",
        ],
    );
    run_git(
        &folder.join("card-wt"),
        &["push", "origin", "feat/demo-kb-001-fix"],
    );
    main
}
