mod common;

mod herdr_cli {
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::net::UnixListener;
    use std::path::Path;
    use std::thread::{self, JoinHandle};

    use hub::external::herdr::{AgentStart, AgentStatus, Herdr, HerdrCli, NewPlace};
    use serde_json::{Value, json};

    use super::common::{TempFolder, read_script_calls};

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
        let script = folder.write_script("herdr", OK_ANSWER, "", 0);
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
            "workspace_id":"w4","tab_id":"w4:t2","focused":false}}}"#;
        let script = folder.write_script("herdr", answer, "", 0);
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

    /// A message with spaces, quotes and a newline must reach the agent
    /// word for word. The wait flags make `hub handoff` wait until the
    /// next hub has taken the message.
    #[test]
    fn herdr_cli_passes_prompt_as_one_argument() {
        let folder = TempFolder::new();
        let script = folder.write_script("herdr", OK_ANSWER, "", 0);
        let herdr = HerdrCli::new(script.clone(), None, folder.path().join("herdr.sock"));
        let text = "Read the \"handoff\" at /tmp/a b.md\nand follow it.";

        herdr.prompt("kanban-hub-2", text).unwrap();

        assert_eq!(
            read_script_calls(&script),
            [[
                "agent",
                "prompt",
                "kanban-hub-2",
                text,
                "--wait",
                "--until",
                "working",
                "--until",
                "blocked",
                "--timeout",
                "30000",
            ]]
        );
    }

    /// `hub handoff` makes a tab with `HUB_RUN` set and starts the next
    /// hub in that tab's pane. A pane ID read from the wrong field, or
    /// the model passed before `--`, starts nothing or the wrong model.
    #[test]
    fn herdr_cli_creates_tab_then_starts_agent() {
        let folder = TempFolder::new();
        let tab_answer = r#"{"result":{"type":"tab_created","root_pane":{"pane_id":"w1:p9"},"tab":{"tab_id":"w1:t9"}}}"#;
        let script = folder.write_script("herdr", tab_answer, "", 0);
        let herdr = HerdrCli::new(script.clone(), None, folder.path().join("herdr.sock"));
        let place = NewPlace {
            label: "kanban-hub-2".to_owned(),
            cwd: "/home/u/notes".into(),
            env: vec![("HUB_RUN".to_owned(), "/home/u/run".to_owned())],
        };

        let ids = herdr.create_tab("w1", &place).unwrap();
        herdr
            .start_agent(&AgentStart {
                name: "kanban-hub-2".to_owned(),
                pane_id: ids.root_pane_id,
                args: vec!["--model".to_owned(), "m#medium".to_owned()],
            })
            .unwrap();

        assert_eq!(
            read_script_calls(&script),
            [
                vec![
                    "tab",
                    "create",
                    "--no-focus",
                    "--workspace",
                    "w1",
                    "--label",
                    "kanban-hub-2",
                    "--cwd",
                    "/home/u/notes",
                    "--env",
                    "HUB_RUN=/home/u/run",
                ],
                vec![
                    "agent",
                    "start",
                    "kanban-hub-2",
                    "--kind",
                    "opencode",
                    "--pane",
                    "w1:p9",
                    "--timeout",
                    "90000",
                    "--",
                    "--model",
                    "m#medium",
                ],
            ]
        );
    }

    /// When Herdr prints no JSON error, its standard error is the only
    /// reason the hub can show, such as a server that is not running.
    #[test]
    fn herdr_cli_uses_stderr_without_json() {
        let folder = TempFolder::new();
        let script = folder.write_script("herdr", "", "herdr: no server at /run/herdr.sock\n", 1);
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

mod card_cli {
    use hub::external::cards::{CardCli, Cards};

    use super::common::{TempFolder, read_script_calls};

    /// `hub read --card` adds the note to the card named first. Key and
    /// text swapped would put the note on no card, or fail. The `--`
    /// keeps a note such as `-h` from being read as an option of `card`.
    #[test]
    fn card_cli_passes_note_key_then_text() {
        let folder = TempFolder::new();
        let script = folder.write_script("card", "", "", 0);
        let cards = CardCli::new(script.clone());

        cards
            .add_note(&"DEMO-KB-007".parse().unwrap(), "merged #9")
            .unwrap();

        assert_eq!(
            read_script_calls(&script),
            [["note", "--", "DEMO-KB-007", "merged #9"]]
        );
    }

    /// `hub brief` shows card's own reason on its board line, such as a
    /// vault it cannot find, so the user can fix it.
    #[test]
    fn card_cli_error_keeps_card_stderr() {
        let folder = TempFolder::new();
        let script = folder.write_script("card", "", "card: no default vault\n", 1);
        let cards = CardCli::new(script);

        let error = cards.read_board().unwrap_err();

        assert_eq!(error.to_string(), "card: no default vault");
    }
}
