mod common;

use std::fs;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::process::Stdio;

use common::{APPROVED, STORE, TempFolder, add_decision, make_program_command, make_repo};
use glimmer::steps::make_feedback_request_list::{ChangeRequest, make_feedback_request_list};
use serde_json::{Value, json};

/// Starts the MCP server in `dir`, greets it as an MCP client does, sends it `messages`, and returns each response after the greeting's.
fn run_mcp_server(dir: &Path, messages: &[String]) -> Vec<Value> {
    let greeting = [
        json!({"jsonrpc": "2.0", "id": 0, "method": "initialize", "params": {"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "test", "version": "1"}}}).to_string(),
        json!({"jsonrpc": "2.0", "method": "notifications/initialized"}).to_string(),
    ];
    let mut server = make_program_command(dir)
        .arg("mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut input = server.stdin.take().unwrap();
    for message in greeting.iter().chain(messages) {
        writeln!(input, "{message}").unwrap();
    }
    // Closing stdin ends the server once it has answered every message.
    drop(input);
    let output = server.wait_with_output().unwrap();
    let mut responses: Vec<Value> = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(responses.remove(0)["id"], json!(0));
    responses
}

/// Returns a `tools/call` request with `id` for the tool `name`.
fn make_tool_call(id: u32, name: &str, arguments: Value) -> String {
    json!({"jsonrpc": "2.0", "id": id, "method": "tools/call", "params": {"name": name, "arguments": arguments}}).to_string()
}

/// Returns the reply a tool gives through the MCP server, read from the text of its result.
fn call_mcp_tool(dir: &Path, name: &str, arguments: Value) -> Value {
    let responses = run_mcp_server(dir, &[make_tool_call(1, name, arguments)]);
    let text = responses[0]["result"]["content"][0]["text"]
        .as_str()
        .unwrap();
    serde_json::from_str(text).unwrap()
}

/// Returns the reply the CLI prints for `args` in `dir`.
fn run_cli(dir: &Path, args: &[&str]) -> Value {
    let output = make_program_command(dir).args(args).output().unwrap();
    serde_json::from_slice(&output.stdout).unwrap()
}

/// Removes the Review Session, so the next call starts from the same state as the one before.
fn remove_review_session(dir: &Path) {
    match fs::remove_file(dir.join(".git/pde/review.json")) {
        Err(error) if error.kind() != ErrorKind::NotFound => panic!("{error}"),
        _ => {}
    }
}

/// `call_tool` must give the same reply as the CLI command for each tool, including step 3 with `base` and the CLI's input file.
/// Otherwise an agent would get a different reply through MCP than through the CLI for the same call.
#[test]
fn mcp_reply_matches_cli_reply() {
    let repo = make_repo();
    let dir = repo.path();
    let dir_text = dir.to_str().unwrap();
    add_decision(dir, STORE, "SaveAnswer", APPROVED);
    // The input files live outside the checkout, so no step lists them as changed files.
    let files = TempFolder::new();
    let input_file = files.path().join("input.json");
    let change_requests = json!([{"file": STORE, "name": "SaveAnswer"}]);
    fs::write(
        &input_file,
        json!({"change_requests": change_requests}).to_string(),
    )
    .unwrap();
    let list_file = files.path().join("list.json");
    let list = make_list_with_keep_suggestion(dir);
    fs::write(&list_file, list.to_string()).unwrap();
    let input_path = input_file.to_str().unwrap();
    let list_path = list_file.to_str().unwrap();
    let cases = [
        (
            vec!["check-comments", "--base", "main"],
            "check_comments",
            json!({"dir": dir_text, "base": "main"}),
        ),
        (
            vec![
                "make-feedback-request-list",
                "--base",
                "main",
                "--input",
                input_path,
            ],
            "make_feedback_request_list",
            json!({"dir": dir_text, "base": "main", "change_requests": change_requests}),
        ),
        (
            vec!["send-agent-suggestions", list_path],
            "send_agent_suggestions",
            json!({"dir": dir_text, "list": list}),
        ),
        (vec!["show-status"], "show_status", json!({"dir": dir_text})),
    ];

    for (args, tool, arguments) in cases {
        // Each call may start a Review Session, so the MCP call starts from the state the CLI call had.
        let cli_reply = run_cli(dir, &args);
        remove_review_session(dir);
        let mcp_reply = call_mcp_tool(dir, tool, arguments);
        remove_review_session(dir);

        assert_eq!(mcp_reply, cli_reply, "{tool}");
    }
}

/// Returns step 3's list for a change request to the approved comment, with a keep Agent Suggestion, as JSON.
fn make_list_with_keep_suggestion(dir: &Path) -> Value {
    let change_request = ChangeRequest {
        file: PathBuf::from(STORE),
        name: "SaveAnswer".to_owned(),
    };
    let list = make_feedback_request_list(dir, None, &[change_request])
        .unwrap()
        .result
        .list;
    let mut list = serde_json::to_value(list).unwrap();
    list["requests"][0]["suggestion"] =
        json!({"kind": "keep", "reasoning": "Says what it does.", "also_at": []});
    list
}

/// `serve_mcp` must answer a message it cannot read with a JSON-RPC error that says what was wrong, then answer the next call.
/// Otherwise an agent that sends one broken message would learn nothing, and the server would stop serving its later calls.
#[test]
fn unreadable_message_gets_mcp_error() {
    let repo = make_repo();
    let dir = repo.path();
    let messages = [
        "{not json".to_owned(),
        make_tool_call(2, "show_status", json!({"dir": dir.to_str().unwrap()})),
    ];

    let responses = run_mcp_server(dir, &messages);

    assert_eq!(responses[0]["id"], json!(null));
    assert_eq!(responses[0]["error"]["code"], json!(-32700));
    assert!(
        responses[0]["error"]["message"]
            .as_str()
            .unwrap()
            .contains("JSON")
    );
    assert_eq!(responses[1]["id"], json!(2));
    assert_eq!(responses[1]["result"]["isError"], json!(false));
}
