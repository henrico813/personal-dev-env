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
const PROMPT_TIME_LIMIT_MS: &str = "30000";
/// Long enough for OpenCode to load the folder and open the session.
const START_TIME_LIMIT_MS: &str = "90000";
/// Keeps a stuck Herdr from holding `hub gather` forever.
const SOCKET_TIME_LIMIT: Duration = Duration::from_secs(10);

/// Calls Herdr. `HerdrCli` runs the real command; tests use
/// `InMemoryHerdr` from `tests/common`.
pub trait Herdr {
    /// Returns every named agent, the hub's own included.
    fn list_agents(&self) -> Result<Vec<Agent>, HerdrError>;
    /// Returns one agent by name or pane ID.
    fn get_agent(&self, target: &str) -> Result<Agent, HerdrError>;
    /// Submits text and waits up to 30 seconds until the agent works
    /// or blocks.
    fn prompt(&self, name: &str, text: &str) -> Result<(), HerdrError>;
    /// Returns the visible part of the agent's terminal.
    fn read_screen(&self, name: &str) -> Result<String, HerdrError>;
    /// Sends one Enter key press to the agent.
    fn press_enter(&self, name: &str) -> Result<(), HerdrError>;
    /// Waits up to 30 seconds for the agent to start working or block.
    fn wait_until_started(&self, name: &str) -> Result<(), HerdrError>;
    /// Shows "needs you" in place of each given state in the sidebar.
    fn mark_needs_you(&self, pane_id: &str, states: &[AgentStatus]) -> Result<(), HerdrError>;
    /// Removes the hub's state labels from a pane.
    fn clear_marks(&self, pane_id: &str) -> Result<(), HerdrError>;
    /// Returns workspaces in sidebar order, top first.
    fn list_workspaces(&self) -> Result<Vec<Workspace>, HerdrError>;
    /// Makes an unfocused tab in a workspace and returns its root pane.
    fn create_tab(&self, workspace_id: &str, place: &NewPlace) -> Result<PlaceIds, HerdrError>;
    /// Starts an OpenCode agent at a pane's shell prompt.
    fn start_agent(&self, start: &AgentStart) -> Result<(), HerdrError>;
    /// Shows the agent's pane in every terminal attached to Herdr.
    fn focus_agent(&self, name: &str) -> Result<(), HerdrError>;
    /// Moves workspaces into one block before another, through
    /// Herdr's socket.
    fn move_workspaces(&self, ids: &[String], before_id: &str) -> Result<(), HerdrError>;
    /// Returns a Herdr for a saved machine, for a held message sent
    /// with `--machine`.
    fn on_machine(&self, label: &str) -> Box<dyn Herdr + '_>;
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
        // Herdr prints its JSON error on stderr, so both outputs are read.
        let refusal = [&output.stdout, &output.stderr]
            .into_iter()
            .find_map(|printed| {
                let answer: Value = serde_json::from_slice(printed).ok()?;
                read_refusal(&answer)
            });
        if let Some(refusal) = refusal {
            return Err(refusal);
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
        parse_agent_list(&read_result(&printed)?, self.machine.as_deref())
    }

    fn get_agent(&self, target: &str) -> Result<Agent, HerdrError> {
        let printed = self.run_herdr(&["agent", "get", target])?;
        parse_agent(&read_result(&printed)?["agent"], self.machine.as_deref())
    }

    fn prompt(&self, name: &str, text: &str) -> Result<(), HerdrError> {
        let args = [
            "agent",
            "prompt",
            name,
            text,
            "--wait",
            "--until",
            "working",
            "--until",
            "blocked",
            "--timeout",
            PROMPT_TIME_LIMIT_MS,
        ];
        self.run_herdr(&args).map(drop)
    }

    fn read_screen(&self, name: &str) -> Result<String, HerdrError> {
        let printed = self.run_herdr(&["agent", "read", name, "--source", "visible"])?;
        Ok(String::from_utf8_lossy(&printed).into_owned())
    }

    fn press_enter(&self, name: &str) -> Result<(), HerdrError> {
        self.run_herdr(&["agent", "send-keys", name, "Enter"])
            .map(drop)
    }

    fn wait_until_started(&self, name: &str) -> Result<(), HerdrError> {
        let args = [
            "agent",
            "wait",
            name,
            "--until",
            "working",
            "--until",
            "blocked",
            "--timeout",
            PROMPT_TIME_LIMIT_MS,
        ];
        self.run_herdr(&args).map(drop)
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

    fn create_tab(&self, workspace_id: &str, place: &NewPlace) -> Result<PlaceIds, HerdrError> {
        let cwd = place.cwd.to_string_lossy();
        let env: Vec<String> = place
            .env
            .iter()
            .map(|(key, value)| format!("{key}={value}"))
            .collect();
        let mut args = vec![
            "tab",
            "create",
            "--no-focus",
            "--workspace",
            workspace_id,
            "--label",
            place.label.as_str(),
            "--cwd",
            cwd.as_ref(),
        ];
        for pair in &env {
            args.push("--env");
            args.push(pair);
        }
        let printed = self.run_herdr(&args)?;
        let result = read_result(&printed)?;
        Ok(PlaceIds {
            root_pane_id: read_text(&result["root_pane"], "pane_id")?,
        })
    }

    fn start_agent(&self, start: &AgentStart) -> Result<(), HerdrError> {
        let mut args = vec![
            "agent",
            "start",
            start.name.as_str(),
            "--kind",
            "opencode",
            "--pane",
            start.pane_id.as_str(),
            "--timeout",
            START_TIME_LIMIT_MS,
            "--",
        ];
        args.extend(start.args.iter().map(String::as_str));
        self.run_herdr(&args).map(drop)
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

    fn on_machine(&self, label: &str) -> Box<dyn Herdr + '_> {
        Box::new(HerdrCli {
            program: self.program.clone(),
            machine: Some(label.to_owned()),
            socket: self.socket.clone(),
        })
    }
}

/// One agent as Herdr lists it.
#[derive(Clone, Debug)]
pub struct Agent {
    pub name: String,
    pub status: AgentStatus,
    pub kind: AgentKind,
    /// The OpenCode or Claude Code session ID, once Herdr has seen it.
    pub session_id: Option<String>,
    pub pane_id: String,
    pub tab_id: String,
    pub workspace_id: String,
    pub focused: bool,
    /// The pane shows "needs you" for its present state.
    pub marked: bool,
    /// Herdr's count of state changes, which tells one blocked state
    /// from the next.
    pub state_seq: u64,
    /// Its working folder and its foreground process's folder, as
    /// Herdr gives them.
    pub folders: Vec<PathBuf>,
    /// The saved machine it runs on, or `None` for this one.
    pub machine: Option<String>,
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

/// Which agent program runs in the pane.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AgentKind {
    OpenCode,
    Claude,
    /// Any other, such as `codex`, by Herdr's name for it. Empty while
    /// Herdr has not yet detected the program.
    Other(String),
}

/// One workspace in sidebar order.
#[derive(Clone, Debug)]
pub struct Workspace {
    pub workspace_id: String,
}

/// A new tab to make.
#[derive(Clone, Debug)]
pub struct NewPlace {
    pub label: String,
    pub cwd: PathBuf,
    /// Set in the pane's environment, such as `HUB_RUN`.
    pub env: Vec<(String, String)>,
}

/// The IDs Herdr gives a new tab.
#[derive(Clone, Debug)]
pub struct PlaceIds {
    pub root_pane_id: String,
}

/// An OpenCode agent to start in a pane.
#[derive(Clone, Debug)]
pub struct AgentStart {
    pub name: String,
    pub pane_id: String,
    /// The agent's own arguments, after `--`.
    pub args: Vec<String>,
}

/// A Herdr call that failed. Each variant holds the words `main` prints
/// after `hub: `.
#[derive(Debug)]
pub enum HerdrError {
    /// Herdr answered with an error code and message.
    Refused { code: HerdrCode, message: String },
    /// `herdr` could not run or exited without a JSON error, or the
    /// socket gave no usable answer.
    Failed(String),
}

/// The Herdr error codes the hub acts on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HerdrCode {
    AgentPromptStalled,
    Timeout,
    AgentNotFound,
    /// Any other code, such as `agent_blocked`, or none.
    Other(String),
}

impl HerdrError {
    /// Tells whether Herdr may have taken the text before it failed.
    /// Herdr reports a stall or a timeout only after the text went in.
    pub(crate) fn may_have_arrived(&self) -> bool {
        matches!(
            self,
            HerdrError::Refused {
                code: HerdrCode::AgentPromptStalled | HerdrCode::Timeout,
                ..
            }
        )
    }

    /// Tells whether no live agent has the name.
    pub(crate) fn is_agent_not_found(&self) -> bool {
        matches!(
            self,
            HerdrError::Refused {
                code: HerdrCode::AgentNotFound,
                ..
            }
        )
    }
}

/// Writes Herdr's own message.
impl fmt::Display for HerdrError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HerdrError::Refused { message, .. } | HerdrError::Failed(message) => {
                formatter.write_str(message)
            }
        }
    }
}

/// Lets `HubError::source` return it.
impl std::error::Error for HerdrError {}

/// Returns the refusal in an answer's `error` object, or `None` when
/// the answer holds no error message.
fn read_refusal(answer: &Value) -> Option<HerdrError> {
    let message = answer.pointer("/error/message")?.as_str()?;
    let code = match answer.pointer("/error/code").and_then(Value::as_str) {
        Some("agent_prompt_stalled") => HerdrCode::AgentPromptStalled,
        Some("timeout") => HerdrCode::Timeout,
        Some("agent_not_found") => HerdrCode::AgentNotFound,
        other => HerdrCode::Other(other.unwrap_or_default().to_owned()),
    };
    Some(HerdrError::Refused {
        code,
        message: message.to_owned(),
    })
}

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
fn parse_agent_list(result: &Value, machine: Option<&str>) -> Result<Vec<Agent>, HerdrError> {
    let raw_agents = result["agents"]
        .as_array()
        .ok_or_else(|| HerdrError::Failed("Herdr's agent list has no agents".to_owned()))?;
    raw_agents
        .iter()
        .filter(|raw| raw["name"].as_str().is_some_and(|name| !name.is_empty()))
        .map(|raw| parse_agent(raw, machine))
        .collect()
}

/// Turns one agent from Herdr's JSON into an `Agent` on `machine`.
fn parse_agent(raw: &Value, machine: Option<&str>) -> Result<Agent, HerdrError> {
    let status = match raw["agent_status"].as_str() {
        Some("working") => AgentStatus::Working,
        Some("idle") => AgentStatus::Idle,
        Some("done") => AgentStatus::Done,
        Some("blocked") => AgentStatus::Blocked,
        Some(_) => AgentStatus::Unknown,
        None => return Err(make_missing_field_error("agent_status")),
    };
    // Herdr lists a starting agent with `"agent": null` until it detects
    // the program. Its state is then unknown, so a message for it waits.
    let (kind, status) = match raw.get("agent") {
        Some(Value::Null) => (AgentKind::Other(String::new()), AgentStatus::Unknown),
        _ => {
            let kind = match read_text(raw, "agent")?.as_str() {
                "opencode" => AgentKind::OpenCode,
                "claude" => AgentKind::Claude,
                other => AgentKind::Other(other.to_owned()),
            };
            (kind, status)
        }
    };
    let status_word = raw["agent_status"].as_str().unwrap_or_default();
    let mut folders: Vec<PathBuf> = ["cwd", "foreground_cwd"]
        .into_iter()
        .filter_map(|field| raw[field].as_str())
        .map(PathBuf::from)
        .collect();
    folders.dedup();
    Ok(Agent {
        name: read_text(raw, "name")?,
        status,
        kind,
        session_id: raw["agent_session"]["value"].as_str().map(str::to_owned),
        pane_id: read_text(raw, "pane_id")?,
        tab_id: read_text(raw, "tab_id")?,
        workspace_id: read_text(raw, "workspace_id")?,
        focused: raw["focused"]
            .as_bool()
            .ok_or_else(|| make_missing_field_error("focused"))?,
        marked: raw["state_labels"][status_word].as_str() == Some(NEEDS_YOU),
        state_seq: raw["state_change_seq"]
            .as_u64()
            .ok_or_else(|| make_missing_field_error("state_change_seq"))?,
        folders,
        machine: machine.map(str::to_owned),
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
    if let Some(refusal) = read_refusal(&reply) {
        return Err(refusal);
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
    /// at once. `hub handoff` names the old hub's session and tab from
    /// it, so a misread one sends the session review to the wrong place.
    /// `hub status` reads the mark, folders and machine to decide what
    /// may close.
    #[test]
    fn parse_agent_reads_herdr_fields() {
        let answer: Value = serde_json::from_str(AGENT_LIST).unwrap();

        let agent = parse_agent(&answer["result"]["agents"][0], Some("laptop")).unwrap();

        assert_eq!(agent.name, "demo-kb-007-impl");
        assert_eq!(agent.status, AgentStatus::Idle);
        assert_eq!(agent.kind, AgentKind::OpenCode);
        assert_eq!(agent.session_id.as_deref(), Some("ses_0a1b2c3d4e5f"));
        assert_eq!(agent.pane_id, "w1:p7");
        assert_eq!(agent.tab_id, "w1:t3");
        assert_eq!(agent.workspace_id, "w1");
        assert!(!agent.focused);
        assert!(agent.marked);
        assert_eq!(agent.state_seq, 12);
        assert_eq!(agent.folders, [PathBuf::from("/home/u/wt/demo-kb-007")]);
        assert_eq!(agent.machine.as_deref(), Some("laptop"));
    }

    /// Herdr keeps a label for each state, and a pane shows only the
    /// label for its present state. Reading a label set for another
    /// state would show a working agent as `you`, so the hub would send
    /// the user to a session that needs nothing.
    #[test]
    fn parse_agent_marks_only_its_present_state() {
        let answer: Value = serde_json::from_str(AGENT_LIST).unwrap();
        // The sample labels only `idle` as needs you.
        let mut raw = answer["result"]["agents"][0].clone();
        raw["agent_status"] = Value::from("working");

        let agent = parse_agent(&raw, None).unwrap();

        assert!(!agent.marked);
    }

    /// Herdr lists a pane with no name when nobody started its agent by
    /// name. Reading that pane as a session would break `hub go` and
    /// `hub gather` whenever such a pane is open.
    #[test]
    fn parse_agent_list_skips_unnamed_agents() {
        let answer: Value = serde_json::from_str(AGENT_LIST).unwrap();

        let agents = parse_agent_list(&answer["result"], None).unwrap();

        let names: Vec<&str> = agents.iter().map(|agent| agent.name.as_str()).collect();
        assert_eq!(names, ["demo-kb-007-impl"]);
    }

    /// Herdr lists a new agent by name before it detects which program
    /// runs there. That entry must stay in the list with an unknown
    /// state, whatever state Herdr gives, so a message for it waits.
    /// Left out, the session looks gone, and its message is marked not
    /// sent or moved to an older session of the card.
    #[test]
    fn parse_agent_list_keeps_undetected_agents_unknown() {
        let result = json!({"agents": [
            {
                "name": "docs-fix",
                "agent": null,
                "agent_status": "idle",
                "pane_id": "w4:p2",
                "workspace_id": "w4",
                "tab_id": "w4:t2",
                "focused": false,
                "state_change_seq": 1,
            },
            {
                "name": "notes-fix",
                "agent": "opencode",
                "agent_status": "idle",
                "pane_id": "w4:p3",
                "workspace_id": "w4",
                "tab_id": "w4:t3",
                "focused": false,
                "state_change_seq": 1288,
            },
        ]});

        let agents = parse_agent_list(&result, None).unwrap();

        let states: Vec<(&str, AgentStatus)> = agents
            .iter()
            .map(|agent| (agent.name.as_str(), agent.status))
            .collect();
        assert_eq!(
            states,
            [
                ("docs-fix", AgentStatus::Unknown),
                ("notes-fix", AgentStatus::Idle),
            ]
        );
    }
}
