//! Calls Herdr, the terminal program that runs the sessions, through
//! the `herdr` command and its socket. Read the `Herdr` trait first.
//! Tests use an in-memory double in its place.

use std::fmt;
use std::io::{self, BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use serde_json::{Value, json};

const LABEL_SOURCE: &str = "pde-hub";
const NEEDS_YOU: &str = "needs you";
/// Keeps a stuck Herdr from holding `hub gather` forever.
const SOCKET_TIME_LIMIT: Duration = Duration::from_secs(10);

/// Calls Herdr. `HerdrCli` runs the real command; tests use
/// `InMemoryHerdr` from `tests/common`.
pub trait Herdr {
    /// Returns every named agent, the hub's own included.
    fn list_agents(&self) -> Result<Vec<Agent>, HerdrError>;
    /// Returns one agent by name or pane ID.
    fn get_agent(&self, target: &str) -> Result<Agent, HerdrError>;
    /// Shows "needs you" in place of each given state in the sidebar.
    fn mark_needs_you(&self, pane_id: &str, states: &[AgentStatus]) -> Result<(), HerdrError>;
    /// Removes the hub's state labels from a pane.
    fn clear_marks(&self, pane_id: &str) -> Result<(), HerdrError>;
    /// Returns workspaces in sidebar order, top first.
    fn list_workspaces(&self) -> Result<Vec<Workspace>, HerdrError>;
    /// Shows the agent's pane in every terminal attached to Herdr.
    fn focus_agent(&self, name: &str) -> Result<(), HerdrError>;
    /// Moves workspaces into one block before another, through
    /// Herdr's socket.
    fn move_workspaces(&self, ids: &[String], before_id: &str) -> Result<(), HerdrError>;
}

/// Runs the `herdr` command, on this machine or a saved one.
pub struct HerdrCli {
    program: PathBuf,
    machine: Option<String>,
    socket: PathBuf,
}

impl HerdrCli {
    /// Runs `program`, which `main` gives as `herdr` and adapter tests
    /// give as a script path, so no test changes `PATH`.
    pub fn new(program: PathBuf, machine: Option<String>, socket: PathBuf) -> HerdrCli {
        HerdrCli {
            program,
            machine,
            socket,
        }
    }

    /// Runs `herdr` with `args` and returns what it printed.
    fn run_herdr(&self, args: &[&str]) -> Result<Vec<u8>, HerdrError> {
        let mut command = Command::new(&self.program);
        if let Some(machine) = &self.machine {
            command.arg("--machine").arg(machine);
        }
        let output = command.args(args).output().map_err(|error| {
            HerdrError::Failed(format!("cannot run {}: {error}", self.program.display()))
        })?;
        if output.status.success() {
            return Ok(output.stdout);
        }
        let answer: Option<Value> = serde_json::from_slice(&output.stdout).ok();
        let refusal = answer
            .as_ref()
            .and_then(|answer| answer.pointer("/error/message"))
            .and_then(Value::as_str);
        if let Some(message) = refusal {
            return Err(HerdrError::Refused(message.to_owned()));
        }
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        if stderr.is_empty() {
            Err(HerdrError::Failed(format!(
                "herdr ended with {}",
                output.status
            )))
        } else {
            Err(HerdrError::Failed(stderr))
        }
    }

    /// Sends pane label options to Herdr for one pane.
    fn report_pane_labels(&self, pane_id: &str, options: &[&str]) -> Result<(), HerdrError> {
        // Herdr takes the pane ID before --source and refuses it after.
        let mut args = vec!["pane", "report-metadata", pane_id, "--source", LABEL_SOURCE];
        args.extend_from_slice(options);
        self.run_herdr(&args).map(drop)
    }
}

impl Herdr for HerdrCli {
    fn list_agents(&self) -> Result<Vec<Agent>, HerdrError> {
        let printed = self.run_herdr(&["agent", "list"])?;
        parse_agent_list(&read_result(&printed)?)
    }

    fn get_agent(&self, target: &str) -> Result<Agent, HerdrError> {
        let printed = self.run_herdr(&["agent", "get", target])?;
        parse_agent(&read_result(&printed)?["agent"])
    }

    fn mark_needs_you(&self, pane_id: &str, states: &[AgentStatus]) -> Result<(), HerdrError> {
        let labels: Vec<String> = states
            .iter()
            .map(|state| format!("{}={NEEDS_YOU}", state.as_str()))
            .collect();
        let mut options = Vec::new();
        for label in &labels {
            options.push("--state-label");
            options.push(label.as_str());
        }
        self.report_pane_labels(pane_id, &options)
    }

    fn clear_marks(&self, pane_id: &str) -> Result<(), HerdrError> {
        self.report_pane_labels(pane_id, &["--clear-state-labels"])
    }

    fn list_workspaces(&self) -> Result<Vec<Workspace>, HerdrError> {
        let printed = self.run_herdr(&["workspace", "list"])?;
        let result = read_result(&printed)?;
        let raw_workspaces = result["workspaces"].as_array().ok_or_else(|| {
            HerdrError::Failed("Herdr's workspace list has no workspaces".to_owned())
        })?;
        raw_workspaces
            .iter()
            .map(|raw| {
                Ok(Workspace {
                    workspace_id: read_text(raw, "workspace_id")?,
                })
            })
            .collect()
    }

    fn focus_agent(&self, name: &str) -> Result<(), HerdrError> {
        self.run_herdr(&["agent", "focus", name]).map(drop)
    }

    fn move_workspaces(&self, ids: &[String], before_id: &str) -> Result<(), HerdrError> {
        // The herdr command cannot reorder workspaces, so this one call
        // goes through the socket.
        let params = json!({"workspace_ids": ids, "before_workspace_id": before_id});
        call_socket(&self.socket, "workspace.move_block", params)
    }
}

/// One agent as Herdr lists it.
#[derive(Clone, Debug)]
pub struct Agent {
    pub name: String,
    pub status: AgentStatus,
    pub pane_id: String,
    pub workspace_id: String,
    pub focused: bool,
}

/// Herdr's state for an agent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AgentStatus {
    Working,
    Idle,
    Done,
    Blocked,
    /// Herdr's own `unknown`, or a state this hub does not know yet.
    Unknown,
}

impl AgentStatus {
    /// Returns Herdr's word for the state, as its labels take it.
    fn as_str(self) -> &'static str {
        match self {
            AgentStatus::Working => "working",
            AgentStatus::Idle => "idle",
            AgentStatus::Done => "done",
            AgentStatus::Blocked => "blocked",
            AgentStatus::Unknown => "unknown",
        }
    }
}

/// One workspace in sidebar order.
#[derive(Clone, Debug)]
pub struct Workspace {
    pub workspace_id: String,
}

/// A Herdr call that failed. Each variant holds the words `main` prints
/// after `hub: `.
#[derive(Debug)]
pub enum HerdrError {
    /// Herdr answered with an error message.
    Refused(String),
    /// `herdr` could not run or exited without a JSON error, or the
    /// socket gave no usable answer.
    Failed(String),
}

/// Writes Herdr's own message.
impl fmt::Display for HerdrError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HerdrError::Refused(message) | HerdrError::Failed(message) => {
                formatter.write_str(message)
            }
        }
    }
}

/// Lets `HubError::source` return it.
impl std::error::Error for HerdrError {}

/// Returns the `result` of an answer `herdr` printed.
fn read_result(printed: &[u8]) -> Result<Value, HerdrError> {
    let mut answer: Value = serde_json::from_slice(printed)
        .map_err(|error| HerdrError::Failed(format!("Herdr's answer is not JSON: {error}")))?;
    match answer.get_mut("result") {
        Some(result) => Ok(result.take()),
        None => Err(HerdrError::Failed(
            "Herdr's answer has no result".to_owned(),
        )),
    }
}

/// Turns `agent list`'s result into agents, leaving out panes where
/// Herdr found an agent nobody named.
fn parse_agent_list(result: &Value) -> Result<Vec<Agent>, HerdrError> {
    let raw_agents = result["agents"]
        .as_array()
        .ok_or_else(|| HerdrError::Failed("Herdr's agent list has no agents".to_owned()))?;
    raw_agents
        .iter()
        .filter(|raw| raw["name"].as_str().is_some_and(|name| !name.is_empty()))
        .map(parse_agent)
        .collect()
}

/// Turns one agent from Herdr's JSON into an `Agent`.
fn parse_agent(raw: &Value) -> Result<Agent, HerdrError> {
    let status = match raw["agent_status"].as_str() {
        Some("working") => AgentStatus::Working,
        Some("idle") => AgentStatus::Idle,
        Some("done") => AgentStatus::Done,
        Some("blocked") => AgentStatus::Blocked,
        Some(_) => AgentStatus::Unknown,
        None => return Err(make_missing_field_error("agent_status")),
    };
    Ok(Agent {
        name: read_text(raw, "name")?,
        status,
        pane_id: read_text(raw, "pane_id")?,
        workspace_id: read_text(raw, "workspace_id")?,
        focused: raw["focused"]
            .as_bool()
            .ok_or_else(|| make_missing_field_error("focused"))?,
    })
}

/// Returns one text field of Herdr's JSON.
fn read_text(raw: &Value, field: &str) -> Result<String, HerdrError> {
    raw[field]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| make_missing_field_error(field))
}

/// Returns the failure for an answer that lacks a field, as after a
/// Herdr upgrade renames one.
fn make_missing_field_error(field: &str) -> HerdrError {
    HerdrError::Failed(format!("Herdr's answer has no {field}"))
}

/// Sends one request to Herdr's socket and checks its answer.
fn call_socket(socket: &Path, method: &str, params: Value) -> Result<(), HerdrError> {
    let unreachable = |error: io::Error| {
        HerdrError::Failed(format!(
            "cannot reach Herdr at {}: {error}",
            socket.display()
        ))
    };
    let mut stream = UnixStream::connect(socket).map_err(unreachable)?;
    stream
        .set_read_timeout(Some(SOCKET_TIME_LIMIT))
        .map_err(unreachable)?;
    stream
        .set_write_timeout(Some(SOCKET_TIME_LIMIT))
        .map_err(unreachable)?;
    let request = json!({"id": "hub", "method": method, "params": params});
    writeln!(stream, "{request}").map_err(unreachable)?;
    let mut answer = String::new();
    BufReader::new(stream)
        .read_line(&mut answer)
        .map_err(unreachable)?;
    let no_usable_answer =
        || HerdrError::Failed(format!("Herdr gave no usable answer: {answer:?}"));
    let reply: Value = serde_json::from_str(&answer).map_err(|_| no_usable_answer())?;
    if let Some(message) = reply.pointer("/error/message").and_then(Value::as_str) {
        return Err(HerdrError::Refused(message.to_owned()));
    }
    if reply.get("result").is_none() {
        return Err(no_usable_answer());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An `agent list` answer copied from Herdr 0.9.1's schema, with
    /// one named agent and one pane where Herdr found an agent nobody
    /// named.
    const AGENT_LIST: &str = include_str!("../../tests/samples/agent-list.json");

    /// Every command reads agents through this parser. A field Herdr
    /// renamed, or one read from the wrong key, breaks every command
    /// at once.
    #[test]
    fn parse_agent_reads_herdr_fields() {
        let answer: Value = serde_json::from_str(AGENT_LIST).unwrap();

        let agent = parse_agent(&answer["result"]["agents"][0]).unwrap();

        assert_eq!(agent.name, "demo-kb-007-impl");
        assert_eq!(agent.status, AgentStatus::Idle);
        assert_eq!(agent.pane_id, "w1:p7");
        assert_eq!(agent.workspace_id, "w1");
        assert!(!agent.focused);
    }

    /// Herdr lists a pane with no name when nobody started its agent by
    /// name. Reading that pane as a session would break `hub go` and
    /// `hub gather` whenever such a pane is open.
    #[test]
    fn parse_agent_list_skips_unnamed_agents() {
        let answer: Value = serde_json::from_str(AGENT_LIST).unwrap();

        let agents = parse_agent_list(&answer["result"]).unwrap();

        let names: Vec<&str> = agents.iter().map(|agent| agent.name.as_str()).collect();
        assert_eq!(names, ["demo-kb-007-impl"]);
    }
}
