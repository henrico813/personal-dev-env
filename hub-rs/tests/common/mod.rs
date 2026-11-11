// Each test file uses only some of these helpers.
#![allow(dead_code)]

use std::cell::RefCell;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, MutexGuard, PoisonError};

use hub::outside::herdr::{Agent, AgentStatus, Herdr, HerdrError, Workspace};

/// Herdr, as the `Herdr` trait, with its agents and workspaces in
/// memory. It records each mark, clear, focus and workspace move it
/// is asked for.
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
}

impl Herdr for InMemoryHerdr {
    fn list_agents(&self) -> Result<Vec<Agent>, HerdrError> {
        Ok(self.agents.clone())
    }

    fn get_agent(&self, target: &str) -> Result<Agent, HerdrError> {
        self.agents
            .iter()
            .find(|agent| agent.name == target || agent.pane_id == target)
            .cloned()
            .ok_or_else(|| HerdrError::Refused(format!("agent target {target} not found")))
    }

    fn mark_needs_you(&self, pane_id: &str, states: &[AgentStatus]) -> Result<(), HerdrError> {
        self.marks
            .borrow_mut()
            .push((pane_id.to_owned(), states.to_vec()));
        Ok(())
    }

    fn clear_marks(&self, pane_id: &str) -> Result<(), HerdrError> {
        self.cleared.borrow_mut().push(pane_id.to_owned());
        Ok(())
    }

    fn list_workspaces(&self) -> Result<Vec<Workspace>, HerdrError> {
        Ok(self.workspaces.clone())
    }

    fn focus_agent(&self, name: &str) -> Result<(), HerdrError> {
        self.focused.borrow_mut().push(name.to_owned());
        Ok(())
    }

    fn move_workspaces(&self, ids: &[String], before_id: &str) -> Result<(), HerdrError> {
        self.moves
            .borrow_mut()
            .push((ids.to_vec(), before_id.to_owned()));
        Ok(())
    }
}

/// One unfocused agent in workspace `w1`, with pane `w1:<name>`.
pub fn make_agent(name: &str, status: AgentStatus) -> Agent {
    Agent {
        name: name.to_owned(),
        status,
        pane_id: format!("w1:{name}"),
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
