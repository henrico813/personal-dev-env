mod common;

mod herdr_cli {
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::net::UnixListener;
    use std::path::Path;
    use std::thread::{self, JoinHandle};

    use hub::outside::herdr::{AgentStatus, Herdr, HerdrCli};
    use serde_json::{Value, json};

    use super::common::{TempFolder, read_script_calls, write_script};

    /// Herdr's answer to a call that changes something.
    const OK_ANSWER: &str = r#"{"id":"cli","result":{"type":"ok"}}"#;

    /// Starts a made-up Herdr socket at `socket` that gives one request
    /// `answer`, and returns the request it got.
    fn answer_one_request(socket: &Path, answer: Value) -> JoinHandle<Value> {
        let listener = UnixListener::bind(socket).unwrap();
        thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut line = String::new();
            BufReader::new(&stream).read_line(&mut line).unwrap();
            writeln!(&stream, "{answer}").unwrap();
            serde_json::from_str(&line).unwrap()
        })
    }

    /// Herdr refuses the pane ID after `--source`, so a wrong order
    /// leaves the sidebar without the mark while `hub flag` says it is
    /// there.
    #[test]
    fn herdr_cli_puts_pane_before_source() {
        let folder = TempFolder::new();
        let script = write_script(folder.path(), "herdr", OK_ANSWER, "", 0);
        let herdr = HerdrCli::new(script.clone(), None, folder.path().join("herdr.sock"));

        herdr
            .mark_needs_you("w1:p7", &[AgentStatus::Idle, AgentStatus::Done])
            .unwrap();
        herdr.clear_marks("w1:p7").unwrap();

        let pane = ["pane", "report-metadata", "w1:p7", "--source", "pde-hub"];
        assert_eq!(
            read_script_calls(&script),
            [
                [
                    &pane[..],
                    &[
                        "--state-label",
                        "idle=needs you",
                        "--state-label",
                        "done=needs you",
                    ],
                ]
                .concat(),
                [&pane[..], &["--clear-state-labels"]].concat(),
            ]
        );
    }

    /// A remote hub's `hub --machine laptop flag NAME` must look the
    /// session up on that machine. Herdr reads `--machine` only before
    /// the command, and without it a local session with the same name
    /// gets the mark. A wrong command word breaks `go`, `gather` or
    /// `flag` outright.
    #[test]
    fn herdr_cli_sends_each_command_to_machine() {
        let folder = TempFolder::new();
        let answer = r#"{"result":{"agents":[],"workspaces":[],"agent":{
            "name":"docs-fix","agent_status":"idle","pane_id":"w4:p2",
            "workspace_id":"w4","focused":false}}}"#;
        let script = write_script(folder.path(), "herdr", answer, "", 0);
        let herdr = HerdrCli::new(
            script.clone(),
            Some("laptop".to_owned()),
            folder.path().join("herdr.sock"),
        );

        herdr.list_agents().unwrap();
        herdr.get_agent("docs-fix").unwrap();
        herdr.list_workspaces().unwrap();
        herdr.focus_agent("kanban-hub-1").unwrap();

        assert_eq!(
            read_script_calls(&script),
            [
                vec!["--machine", "laptop", "agent", "list"],
                vec!["--machine", "laptop", "agent", "get", "docs-fix"],
                vec!["--machine", "laptop", "workspace", "list"],
                vec!["--machine", "laptop", "agent", "focus", "kanban-hub-1"],
            ]
        );
    }

    /// When Herdr prints no JSON error, its standard error is the only
    /// reason the hub can show, such as a server that is not running.
    #[test]
    fn herdr_cli_uses_stderr_without_json() {
        let folder = TempFolder::new();
        let script = write_script(
            folder.path(),
            "herdr",
            "",
            "herdr: no server at /run/herdr.sock\n",
            1,
        );
        let herdr = HerdrCli::new(script, None, folder.path().join("herdr.sock"));

        let error = herdr.list_agents().unwrap_err();

        assert_eq!(error.to_string(), "herdr: no server at /run/herdr.sock");
    }

    /// `hub gather` reorders the sidebar through Herdr's socket, which
    /// refuses a request whose method or field names are wrong.
    #[test]
    fn move_workspaces_sends_move_block_request() {
        let folder = TempFolder::new();
        let socket = folder.path().join("herdr.sock");
        let server = answer_one_request(&socket, json!({"id": "hub", "result": {}}));
        let herdr = HerdrCli::new(folder.path().join("herdr"), None, socket);

        herdr
            .move_workspaces(&["w1".to_owned(), "w3".to_owned()], "w2")
            .unwrap();

        assert_eq!(
            server.join().unwrap(),
            json!({
                "id": "hub",
                "method": "workspace.move_block",
                "params": {"workspace_ids": ["w1", "w3"], "before_workspace_id": "w2"},
            })
        );
    }

    /// Hubs act on `hub gather`'s exit code, so a refused move must come
    /// back as an error with Herdr's own words, never as success.
    #[test]
    fn move_workspaces_keeps_herdr_refusal_message() {
        let folder = TempFolder::new();
        let socket = folder.path().join("herdr.sock");
        let refusal = json!({
            "id": "hub",
            "error": {"code": "invalid_params", "message": "bad block"},
        });
        let server = answer_one_request(&socket, refusal);
        let herdr = HerdrCli::new(folder.path().join("herdr"), None, socket);

        let error = herdr.move_workspaces(&["w1".to_owned()], "w2").unwrap_err();
        server.join().unwrap();

        assert_eq!(error.to_string(), "bad block");
    }
}
