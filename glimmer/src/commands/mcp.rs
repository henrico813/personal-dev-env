//! This module serves the four MCP tools over stdin and stdout, one JSON-RPC
//! message per line, and reads the review files fresh on each call. Read the
//! `serve_mcp()` and `call_tool(name, arguments)` functions to understand the
//! module.

use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use super::{Reply, list_commands, resolve_base};
use crate::shared::review_session::FeedbackRequestList;
use crate::shared::{Error, ErrorCode};
use crate::steps::check_comments::check_comments;
use crate::steps::make_feedback_request_list::{ChangeRequest, make_feedback_request_list};
use crate::steps::send_agent_suggestions::send_agent_suggestions;
use crate::steps::show_status::show_status;

/// Lists the MCP versions this server speaks, newest first. Their tool calls
/// all have the same shape.
const PROTOCOL_VERSIONS: [&str; 4] = ["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];
const PARSE_ERROR: i64 = -32700;
/// Marks JSON that is not a request, such as a list of requests.
const INVALID_REQUEST: i64 = -32600;
const METHOD_NOT_FOUND: i64 = -32601;
const INVALID_PARAMS: i64 = -32602;

/// Serves the four MCP tools over stdin and stdout, one call at a time, until
/// stdin closes. Answers a message it cannot read with a JSON-RPC error and
/// keeps serving.
pub fn serve_mcp() -> io::Result<()> {
    let mut output = io::stdout().lock();
    for line in io::stdin().lock().split(b'\n') {
        let line = line?;
        if line.trim_ascii().is_empty() {
            continue;
        }
        let response = match serde_json::from_slice::<Value>(&line) {
            Ok(message) => answer_message(&message),
            Err(error) => Some(refuse_unreadable_message(&error)),
        };
        // A notification gets no response, as JSON-RPC says.
        if let Some(response) = response {
            serde_json::to_writer(&mut output, &response)?;
            output.write_all(b"\n")?;
            output.flush()?;
        }
    }
    Ok(())
}

/// Runs one MCP tool, giving the same reply as its CLI command. `dir` must be a
/// full path, since the server may run in another folder.
pub fn call_tool(name: &str, arguments: Value) -> Reply {
    match name {
        "check_comments" => {
            Reply::from_outcome(read_arguments(arguments).and_then(|input: CheckInput| {
                let dir = refuse_relative_dir(&input.dir)?;
                check_comments(dir, resolve_base(dir, input.base.as_deref())?)
            }))
        }
        "make_feedback_request_list" => {
            Reply::from_outcome(read_arguments(arguments).and_then(|input: ListInput| {
                let dir = refuse_relative_dir(&input.dir)?;
                let base = resolve_base(dir, input.base.as_deref())?;
                make_feedback_request_list(dir, base, &input.change_requests)
            }))
        }
        "send_agent_suggestions" => {
            Reply::from_outcome(read_arguments(arguments).and_then(|input: SendInput| {
                send_agent_suggestions(refuse_relative_dir(&input.dir)?, input.list)
            }))
        }
        "show_status" => Reply::from_outcome(
            read_arguments(arguments)
                .and_then(|input: DirInput| show_status(refuse_relative_dir(&input.dir)?)),
        ),
        _ => Reply::from_outcome::<()>(Err(Error {
            code: ErrorCode::BadInput,
            message: format!("No tool is named {name}."),
        })),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CheckInput {
    dir: PathBuf,
    base: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ListInput {
    dir: PathBuf,
    base: Option<String>,
    #[serde(default)]
    change_requests: Vec<ChangeRequest>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SendInput {
    dir: PathBuf,
    list: FeedbackRequestList,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DirInput {
    dir: PathBuf,
}

/// Returns the JSON-RPC response to one message, or `None` for a
/// notification.
fn answer_message(message: &Value) -> Option<Value> {
    let id = message.get("id").cloned();
    let Some(method) = message.get("method").and_then(Value::as_str) else {
        return Some(refuse_message(
            id.unwrap_or(Value::Null),
            INVALID_REQUEST,
            "Send one JSON-RPC request object with a method.".to_owned(),
        ));
    };
    // A message without an id is a notification, such as `notifications/initialized`.
    let id = id?;
    let params = message.get("params").cloned().unwrap_or(Value::Null);
    let result = match method {
        "initialize" => Ok(describe_server(&params)),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(list_tools()),
        "tools/call" => answer_tool_call(params),
        _ => Err((METHOD_NOT_FOUND, format!("No method is named {method}."))),
    };
    Some(match result {
        Ok(result) => json!({"jsonrpc": "2.0", "id": id, "result": result}),
        Err((code, message)) => refuse_message(id, code, message),
    })
}

/// Returns the `initialize` result, in the client's MCP version when this
/// server speaks it, else in the newest one it speaks.
fn describe_server(params: &Value) -> Value {
    let asked = params.get("protocolVersion").and_then(Value::as_str);
    let version = PROTOCOL_VERSIONS
        .into_iter()
        .find(|version| Some(*version) == asked)
        .unwrap_or(PROTOCOL_VERSIONS[0]);
    json!({
        "protocolVersion": version,
        "capabilities": {"tools": {}},
        "serverInfo": {"name": "glimmer", "version": env!("CARGO_PKG_VERSION")},
    })
}

/// Returns each tool with its description from `list_commands` and its inputs.
fn list_tools() -> Value {
    let tools: Vec<Value> = list_commands()
        .iter()
        .filter_map(|command| {
            let name = command.mcp_name?;
            Some(json!({
                "name": name,
                "description": command.description,
                "inputSchema": describe_tool_inputs(name),
            }))
        })
        .collect();
    json!({"tools": tools})
}

fn describe_tool_inputs(name: &str) -> Value {
    let mut properties = json!({
        "dir": {"type": "string", "description": "The worktree folder, as a full path."},
    });
    let base = json!({"type": "string", "description": "The task start, a commit or branch. Defaults to the branch start."});
    match name {
        "check_comments" => properties["base"] = base,
        "make_feedback_request_list" => {
            properties["base"] = base;
            properties["change_requests"] = json!({
                "type": "array",
                "description": "Approved comments the agent wants reworded.",
                "items": {
                    "type": "object",
                    "properties": {"file": {"type": "string"}, "name": {"type": "string"}},
                    "required": ["file", "name"],
                    "additionalProperties": false,
                },
            });
        }
        "send_agent_suggestions" => {
            properties["list"] = json!({
                "type": "object",
                "description": "Step 3's list, with an Agent Suggestion in each Feedback Request.",
            });
        }
        _ => {}
    }
    let mut required = vec!["dir"];
    if name == "send_agent_suggestions" {
        required.push("list");
    }
    json!({
        "type": "object",
        "properties": properties,
        "required": required,
        "additionalProperties": false,
    })
}

/// Returns the `tools/call` result, which carries the tool's reply as text.
fn answer_tool_call(params: Value) -> Result<Value, (i64, String)> {
    let Some(name) = params.get("name").and_then(Value::as_str) else {
        return Err((INVALID_PARAMS, "Name the tool to call.".to_owned()));
    };
    let arguments = params.get("arguments").cloned().unwrap_or(json!({}));
    let reply = call_tool(name, arguments);
    let text = serde_json::to_string_pretty(&reply)
        .expect("a reply holds only JSON values, strings and unit variants");
    Ok(json!({
        "content": [{"type": "text", "text": text}],
        "isError": reply.error.is_some(),
    }))
}

/// Returns the JSON-RPC error response to a message that is not JSON, with no
/// ID, since the server cannot read one.
fn refuse_unreadable_message(error: &serde_json::Error) -> Value {
    refuse_message(
        Value::Null,
        PARSE_ERROR,
        format!("Cannot read the message as JSON. {error}"),
    )
}

fn refuse_message(id: Value, code: i64, message: String) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
}

/// Reads a tool's arguments, refusing unknown or missing ones with `BadInput`.
fn read_arguments<T: DeserializeOwned>(arguments: Value) -> Result<T, Error> {
    serde_json::from_value(arguments).map_err(|error| Error {
        code: ErrorCode::BadInput,
        message: format!("Fix the tool's arguments. {error}"),
    })
}

/// Returns `dir`, or refuses with `DirNotAbsolute` when it is not a full path.
fn refuse_relative_dir(dir: &Path) -> Result<&Path, Error> {
    if dir.is_absolute() {
        return Ok(dir);
    }
    Err(Error {
        code: ErrorCode::DirNotAbsolute,
        message: format!(
            "Pass dir as a full path. {} is relative, and the server may run in another folder.",
            dir.display()
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `call_tool` must refuse a relative `dir` with `dir_not_absolute` before it runs a step.
    /// Otherwise a server started in another folder would review that folder's branch instead of the agent's.
    #[test]
    fn call_tool_refuses_relative_dir() {
        let reply = call_tool("show_status", json!({"dir": "no-such-worktree"}));

        assert_eq!(
            reply.error.map(|error| error.code),
            Some(ErrorCode::DirNotAbsolute)
        );
    }

    /// `answer_tool_call` must set `isError` to true when the tool refuses.
    /// Otherwise a harness would show a refused call as a success, and the agent could miss that the step never ran.
    #[test]
    fn answer_tool_call_marks_refusal_as_error() {
        let params = json!({"name": "show_status", "arguments": {"dir": "no-such-worktree"}});

        let result = answer_tool_call(params).unwrap();

        assert_eq!(result["isError"], json!(true));
    }
}
