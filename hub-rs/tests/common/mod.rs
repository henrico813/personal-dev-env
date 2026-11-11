// Each test file uses only some of these helpers.
#![allow(dead_code)]

use std::cell::RefCell;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, MutexGuard, PoisonError};

use hub::external::cards::{BoardCounts, CardError, CardKey, Cards};
use hub::external::herdr::{
    Agent, AgentStart, AgentStatus, Herdr, HerdrError, NewPlace, PlaceIds, Workspace,
};
use hub::external::opencode::OpenCodeDb;
use hub::shared::Tools;
use hub::shared::run_folder::RunFolder;
use jiff::Zoned;
use rusqlite::Connection;
use serde_json::Value;

/// Herdr, as the `Herdr` trait, with its agents and workspaces in
/// memory. It records each prompt, mark, clear, new tab, started agent,
/// focus and workspace move it is asked for.
#[derive(Default)]
pub struct InMemoryHerdr {
    pub agents: Vec<Agent>,
    /// The workspaces in sidebar order, top first.
    pub workspaces: Vec<Workspace>,
    /// Each `mark_needs_you` call, as its pane ID and states.
    pub marks: RefCell<Vec<(String, Vec<AgentStatus>)>>,
    /// The pane ID of each `clear_marks` call.
    pub cleared: RefCell<Vec<String>>,
    /// The agent name of each `focus_agent` call.
    pub focused: RefCell<Vec<String>>,
    /// Each `move_workspaces` call, as the IDs moved and the ID they
    /// went before.
    pub moves: RefCell<Vec<(Vec<String>, String)>>,
    /// Each `prompt` call, as the agent name and the text.
    pub prompts: RefCell<Vec<(String, String)>>,
    /// Each `create_tab` call, as the workspace ID and the place.
    pub created_tabs: RefCell<Vec<(String, NewPlace)>>,
    /// Each `start_agent` call.
    pub started: RefCell<Vec<AgentStart>>,
    /// Failures the next call of a method returns, by method name.
    pub failures: RefCell<Vec<(&'static str, HerdrError)>>,
}

impl InMemoryHerdr {
    /// Makes the next call of `method`, such as `"prompt"`, return
    /// `error`.
    pub fn fail_next(&self, method: &'static str, error: HerdrError) {
        self.failures.borrow_mut().push((method, error));
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
        Ok(self.agents.clone())
    }

    fn get_agent(&self, target: &str) -> Result<Agent, HerdrError> {
        self.take_failure("get_agent")?;
        self.agents
            .iter()
            .find(|agent| agent.name == target || agent.pane_id == target)
            .cloned()
            .ok_or_else(|| HerdrError::Refused(format!("agent target {target} not found")))
    }

    fn prompt(&self, name: &str, text: &str) -> Result<(), HerdrError> {
        self.take_failure("prompt")?;
        self.prompts
            .borrow_mut()
            .push((name.to_owned(), text.to_owned()));
        Ok(())
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

    fn list_workspaces(&self) -> Result<Vec<Workspace>, HerdrError> {
        self.take_failure("list_workspaces")?;
        Ok(self.workspaces.clone())
    }

    /// Answers with root pane `<workspace>:p-<label>`.
    fn create_tab(&self, workspace_id: &str, place: &NewPlace) -> Result<PlaceIds, HerdrError> {
        self.take_failure("create_tab")?;
        self.created_tabs
            .borrow_mut()
            .push((workspace_id.to_owned(), place.clone()));
        Ok(PlaceIds {
            root_pane_id: format!("{workspace_id}:p-{}", place.label),
        })
    }

    fn start_agent(&self, start: &AgentStart) -> Result<(), HerdrError> {
        self.take_failure("start_agent")?;
        self.started.borrow_mut().push(start.clone());
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
}

/// The `card` command, as the `Cards` trait, with the board in
/// memory. It records each note it is asked to add.
#[derive(Default)]
pub struct InMemoryCards {
    /// `None` answers as `card` does without a vault.
    pub board: Option<BoardCounts>,
    /// Each `add_note` call, as the card key and the text.
    pub notes: RefCell<Vec<(String, String)>>,
    /// When set, `add_note` fails with this message and records nothing.
    pub note_error: Option<String>,
}

impl Cards for InMemoryCards {
    fn read_board(&self) -> Result<BoardCounts, CardError> {
        self.board
            .clone()
            .ok_or_else(|| CardError::Failed("card: no default vault".to_owned()))
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
}

/// Every external system for a command test. It holds the doubles and a
/// temporary folder that holds the run folder `run`, the state folder
/// `state` and the OpenCode database `opencode.db`. A test that uses
/// it must not make another `TempFolder`. Its own `TempFolder` already
/// holds the process lock, so a second one would wait for it forever.
pub struct TestTools {
    pub herdr: InMemoryHerdr,
    pub cards: InMemoryCards,
    /// The hub's own pane, as `HERDR_PANE_ID` gives it.
    pub own_pane: Option<String>,
    opencode: OpenCodeDb,
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
            opencode: OpenCodeDb::new(folder.path().join("opencode.db")),
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
            own_pane: self.own_pane.as_deref(),
            state_folder: &self.state_folder,
        }
    }

    /// The temporary folder that holds everything else.
    pub fn path(&self) -> &Path {
        self.folder.path()
    }

    /// The run folder's path, for writing and reading its files.
    pub fn run_path(&self) -> PathBuf {
        self.folder.path().join("run")
    }

    /// The run folder to pass to a command.
    pub fn run(&self) -> RunFolder {
        RunFolder::open(&self.run_path()).unwrap()
    }

    /// The state folder that stands in for `~/.local/state/pde`.
    pub fn state_folder(&self) -> &Path {
        &self.state_folder
    }

    /// The OpenCode database file, which no test has made yet.
    pub fn opencode_path(&self) -> PathBuf {
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

/// One unfocused agent in workspace `w1`, with pane `w1:<name>`, tab
/// `w1:t-<name>` and no session ID.
pub fn make_agent(name: &str, status: AgentStatus) -> Agent {
    Agent {
        name: name.to_owned(),
        status,
        session_id: None,
        pane_id: format!("w1:{name}"),
        tab_id: format!("w1:t-{name}"),
        workspace_id: "w1".to_owned(),
        focused: false,
    }
}

/// Workspaces with these IDs, in this sidebar order.
pub fn make_workspaces(ids: &[&str]) -> Vec<Workspace> {
    ids.iter()
        .map(|id| Workspace {
            workspace_id: (*id).to_owned(),
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
