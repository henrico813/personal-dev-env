mod common;

mod herdr_cli {
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::net::UnixListener;
    use std::path::Path;
    use std::thread::{self, JoinHandle};

    use hub::external::herdr::{
        AgentStart, AgentStatus, Herdr, HerdrCli, HerdrCode, HerdrError, HubKind, NewPlace,
    };
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
    /// there. The agent filter finds hubs and flagged idle sessions
    /// only by these tokens, so a wrong token hides them from the user.
    /// The hub token's value sets the row's place in the list.
    /// A blocked session gets no token, since its mark goes away once it
    /// works again.
    #[test]
    fn herdr_cli_sends_pane_marks_and_tokens() {
        let folder = TempFolder::new();
        let script = folder.write_script("herdr", OK_ANSWER, "", 0);
        let herdr = HerdrCli::new(script.clone(), None, folder.path().join("herdr.sock"));

        herdr
            .mark_needs_you("w1:p7", &[AgentStatus::Idle, AgentStatus::Done])
            .unwrap();
        herdr
            .mark_needs_you("w1:p7", &[AgentStatus::Blocked])
            .unwrap();
        herdr.clear_marks("w1:p7").unwrap();
        herdr.tag_hub_pane("w1:p7", HubKind::Kanban).unwrap();
        herdr.tag_hub_pane("w1:p7", HubKind::Other).unwrap();

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
                        "--token",
                        "needs_you=yes",
                    ],
                ]
                .concat(),
                [&pane[..], &["--state-label", "blocked=needs you"]].concat(),
                [
                    &pane[..],
                    &["--clear-state-labels", "--clear-token", "needs_you"],
                ]
                .concat(),
                [
                    &pane[..],
                    &[
                        "--token",
                        "go_to=1-kanban-hub",
                        "--state-label",
                        "working=hub busy",
                        "--state-label",
                        "idle=hub free",
                        "--state-label",
                        "done=hub free",
                    ],
                ]
                .concat(),
                [
                    &pane[..],
                    &[
                        "--token",
                        "go_to=3-hub",
                        "--state-label",
                        "working=hub busy",
                        "--state-label",
                        "idle=hub free",
                        "--state-label",
                        "done=hub free",
                    ],
                ]
                .concat(),
            ]
        );
    }

    /// A remote hub's `hub --machine laptop flag NAME` must look the
    /// session up on that machine. Herdr reads `--machine` only before
    /// the command, and without it a local session with the same name
    /// gets the mark. A wrong command word breaks `go`, `gather` or
    /// `flag` outright. Each agent must carry the machine, since
    /// `hub status` keeps a remote session open only by it.
    #[test]
    fn herdr_cli_sends_each_command_to_machine() {
        let folder = TempFolder::new();
        let agent = json!({
            "name": "docs-fix",
            "agent": "opencode",
            "agent_status": "idle",
            "pane_id": "w4:p2",
            "workspace_id": "w4",
            "tab_id": "w4:t2",
            "focused": false,
            "state_change_seq": 1288,
        });
        let answer = json!({"result": {"agents": [agent], "workspaces": [], "agent": agent}});
        let script = folder.write_script("herdr", &answer.to_string(), "", 0);
        let herdr = HerdrCli::new(
            script.clone(),
            Some("laptop".to_owned()),
            folder.path().join("herdr.sock"),
        );

        let listed = herdr.list_agents().unwrap();
        let found = herdr.get_agent("docs-fix").unwrap();
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
        assert_eq!(listed[0].machine.as_deref(), Some("laptop"));
        assert_eq!(found.machine.as_deref(), Some("laptop"));
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
        let tab_answer = r#"{"result":{"type":"tab_created","root_pane":{"pane_id":"w1:p9","workspace_id":"w1"},"tab":{"tab_id":"w1:t9"}}}"#;
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

    /// Reopening a parked session whose workspace is gone makes a new
    /// workspace with `HUB_RUN` set, and the tidy pass closes tabs by
    /// ID. A flag in the wrong place starts the session without its run
    /// folder, and a wrong tab ID closes another tab.
    #[test]
    fn herdr_cli_creates_workspace_and_closes_tab() {
        let folder = TempFolder::new();
        let answer = r#"{"result":{"type":"workspace_created","root_pane":{"pane_id":"w4:p1","workspace_id":"w4"},"tab":{"tab_id":"w4:t1"},"workspace":{"workspace_id":"w4"}}}"#;
        let script = folder.write_script("herdr", answer, "", 0);
        let herdr = HerdrCli::new(script.clone(), None, folder.path().join("herdr.sock"));
        let place = NewPlace {
            label: "DEMO-KB-007 impl".to_owned(),
            cwd: "/home/u/wt/demo-kb-007".into(),
            env: vec![("HUB_RUN".to_owned(), "/home/u/run".to_owned())],
        };

        let ids = herdr.create_workspace(&place).unwrap();
        herdr.close_tab("w1:t3").unwrap();

        assert_eq!(ids.root_pane_id, "w4:p1");
        assert_eq!(ids.workspace_id, "w4");
        assert_eq!(
            read_script_calls(&script),
            [
                vec![
                    "workspace",
                    "create",
                    "--no-focus",
                    "--label",
                    "DEMO-KB-007 impl",
                    "--cwd",
                    "/home/u/wt/demo-kb-007",
                    "--env",
                    "HUB_RUN=/home/u/run",
                ],
                vec!["tab", "close", "w1:t3"],
            ]
        );
    }

    /// `hub desk` starts the board by typing `hub board` into the desk's
    /// first pane. Herdr takes the pane before the command, so a swapped
    /// order leaves the desk without its board.
    #[test]
    fn herdr_cli_runs_command_in_pane() {
        let folder = TempFolder::new();
        let script = folder.write_script("herdr", OK_ANSWER, "", 0);
        let herdr = HerdrCli::new(script.clone(), None, folder.path().join("herdr.sock"));

        herdr.run_in_pane("w4:p1", "hub board").unwrap();

        assert_eq!(
            read_script_calls(&script),
            [["pane", "run", "w4:p1", "hub board"]]
        );
    }

    /// The tidy pass closes a quiet shell only with its finished agent's
    /// tab, and it knows the shell is quiet only from these two numbers.
    /// A field read from the wrong key keeps every such tab open, and
    /// the leak of finished tabs comes back.
    #[test]
    fn herdr_cli_reads_shell_process_info() {
        let folder = TempFolder::new();
        let answer = r#"{"result":{"type":"pane_process_info","process_info":{"pane_id":"w1:p8","shell_pid":4100,"foreground_process_group_id":4200,"foreground_processes":[]}}}"#;
        let script = folder.write_script("herdr", answer, "", 0);
        let herdr = HerdrCli::new(script.clone(), None, folder.path().join("herdr.sock"));

        let info = herdr.read_process_info("w1:p8").unwrap();

        assert_eq!((info.shell_pid, info.foreground_group), (4100, 4200));
        assert_eq!(
            read_script_calls(&script),
            [["pane", "process-info", "--pane", "w1:p8"]]
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

    /// Whether a failed message is unsure or not sent comes from Herdr's
    /// error code, which Herdr prints on stderr with stdout empty. A
    /// code misread turns a message that may have arrived into one the
    /// hub sends again, or the reverse.
    #[test]
    fn herdr_cli_reads_refusal_code() {
        let cases = [
            (
                "stalled",
                "agent_prompt_stalled",
                HerdrCode::AgentPromptStalled,
            ),
            ("timeout", "timeout", HerdrCode::Timeout),
            ("not-found", "agent_not_found", HerdrCode::AgentNotFound),
            (
                "blocked",
                "agent_blocked",
                HerdrCode::Other("agent_blocked".to_owned()),
            ),
        ];

        let folder = TempFolder::new();
        for (case_name, code, expected) in cases {
            let answer = format!(r#"{{"error":{{"code":"{code}","message":"Herdr said no"}}}}"#);
            let script = folder.write_script(&format!("herdr-{case_name}"), "", &answer, 1);
            let herdr = HerdrCli::new(script, None, folder.path().join("herdr.sock"));

            let error = herdr.prompt("docs-fix", "Rebase on main.").unwrap_err();

            let HerdrError::Refused { code, message } = error else {
                panic!("{case_name}: {error:?}");
            };
            assert_eq!(code, expected, "{case_name}");
            assert_eq!(message, "Herdr said no", "{case_name}");
        }
    }

    /// `hub next` sends a message held with `--machine` through that
    /// machine's Herdr, including the screen read, Enter press and wait
    /// after a stall. A call without `--machine LABEL` first reaches a
    /// session on this machine instead.
    #[test]
    fn herdr_cli_sends_paste_steps_to_machine() {
        let folder = TempFolder::new();
        let screen = "  ┃  [Pasted ~1 lines]\n";
        let script = folder.write_script("herdr", screen, "", 0);
        let herdr = HerdrCli::new(script.clone(), None, folder.path().join("herdr.sock"));
        let laptop = herdr.on_machine("laptop");

        let shown = laptop.read_screen("docs-fix").unwrap();
        laptop.press_enter("docs-fix").unwrap();
        laptop.wait_until_started("docs-fix").unwrap();

        assert_eq!(shown, screen);
        assert_eq!(
            read_script_calls(&script),
            [
                vec![
                    "--machine",
                    "laptop",
                    "agent",
                    "read",
                    "docs-fix",
                    "--source",
                    "visible",
                ],
                vec![
                    "--machine",
                    "laptop",
                    "agent",
                    "send-keys",
                    "docs-fix",
                    "Enter",
                ],
                vec![
                    "--machine",
                    "laptop",
                    "agent",
                    "wait",
                    "docs-fix",
                    "--until",
                    "working",
                    "--until",
                    "blocked",
                    "--timeout",
                    "30000",
                ],
            ]
        );
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

    /// The desk's board pane must become an agent with the `go_to`
    /// token, or the "go to" list has no desk row. Herdr lists panes in
    /// tab order, so the first pane listed is the board's, not the
    /// shell's.
    #[test]
    fn show_desk_as_agent_reports_first_pane() {
        let folder = TempFolder::new();
        let answer = json!({"id": "cli", "result": {"panes": [
            {"pane_id": "w4:p1", "tab_id": "w4:t1", "focused": false, "agent": null},
            {"pane_id": "w4:p2", "tab_id": "w4:t2", "focused": true, "agent": null},
        ]}});
        let script = folder.write_script("herdr", &answer.to_string(), "", 0);
        let herdr = HerdrCli::new(script.clone(), None, folder.path().join("herdr.sock"));

        herdr.show_desk_as_agent("w4").unwrap();

        assert_eq!(
            read_script_calls(&script),
            [
                vec!["pane", "list", "--workspace", "w4"],
                vec![
                    "pane",
                    "report-agent",
                    "w4:p1",
                    "--source",
                    "pde-hub",
                    "--agent",
                    "desk",
                    "--state",
                    "idle",
                ],
                vec![
                    "pane",
                    "report-metadata",
                    "w4:p1",
                    "--source",
                    "pde-hub",
                    "--token",
                    "go_to=2-desk",
                    "--state-label",
                    "idle=your desk",
                    "--state-label",
                    "done=your desk",
                ],
            ]
        );
    }

    /// Each hub start filters Herdr's agent list through the socket.
    /// Herdr refuses a wrong method or field name, and a wrong token
    /// name, state or sort hides the desk, a hub or a waiting session
    /// from the user or moves it below the others.
    #[test]
    fn filter_agent_list_sends_agent_view_request() {
        let folder = TempFolder::new();
        let socket = folder.path().join("herdr.sock");
        let server = answer_one_request(&socket, json!({"id": "hub", "result": {}}));
        let herdr = HerdrCli::new(folder.path().join("herdr"), None, socket);

        herdr.filter_agent_list().unwrap();

        assert_eq!(
            server.join().unwrap(),
            json!({
                "id": "hub",
                "method": "agent.view.set",
                "params": {
                    "source": "pde-hub",
                    "label": "go to",
                    "filter": {"op": "any", "filters": [
                        {"op": "exists", "field": {"token": "go_to"}},
                        {"op": "eq", "field": "status", "value": "blocked"},
                        {"op": "all", "filters": [
                            {"op": "eq", "field": {"token": "needs_you"}, "value": "yes"},
                            {"op": "in", "field": "status", "values": ["idle", "done"]},
                        ]},
                    ]},
                    "sort": [
                        {"field": {"token": "go_to"}, "order": "asc"},
                        {"field": "attention", "order": "desc"},
                        {"field": "state_change_seq", "order": "desc"},
                    ],
                },
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
    use hub::external::cards::{CardCli, CardKey, Cards};

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

    /// A message to a worker that handed off goes to the newest live
    /// session in its card's `sessions` list. A list misread sends it to
    /// an older session or to none.
    #[test]
    fn card_cli_reads_sessions_from_card_show() {
        let folder = TempFolder::new();
        let card_show = "---\n\
                         stage: active\n\
                         sessions:\n  \
                           - \"opencode ses_0a1b2c3d4e5f\"\n  \
                           - \"opencode ses_9f8e7d6c5b4a\"\n\
                         project: DevEnv\n\
                         ---\n\
                         \n\
                         # DEMO-KB-007 Retry failed uploads\n\
                         \n\
                         - opencode ses_not_a_session\n";
        let script = folder.write_script("card", card_show, "", 0);
        let cards = CardCli::new(script.clone());

        let note = cards.read_card(&"DEMO-KB-007".parse().unwrap()).unwrap();

        assert_eq!(note.sessions, ["ses_0a1b2c3d4e5f", "ses_9f8e7d6c5b4a"]);
        assert_eq!(read_script_calls(&script), [["show", "DEMO-KB-007"]]);
    }

    /// `hub auto ready` and `hub auto check` work from this list. A
    /// draft card in it could be run without the user's say, and a
    /// ready card left out never reaches the user's briefing.
    #[test]
    fn card_cli_lists_ready_keys_in_order() {
        let folder = TempFolder::new();
        let card_list = "DEMO-KB-007 ready task Retry failed uploads\n\
                         DEMO-KB-003 ready task Fix the docs\n\
                         2 cards\n";
        let script = folder.write_script("card", card_list, "", 0);
        let cards = CardCli::new(script.clone());

        let keys = cards.list_ready().unwrap();

        let keys: Vec<&str> = keys.iter().map(CardKey::as_str).collect();
        assert_eq!(keys, ["DEMO-KB-007", "DEMO-KB-003"]);
        assert_eq!(read_script_calls(&script), [["list", "--stage", "ready"]]);
    }

    /// Parking adds the session to its card before closing the tab, so
    /// the card keeps a way back to the reviewed work. The agent name
    /// read as a plan note or a PR link would link nothing.
    #[test]
    fn card_cli_passes_link_arguments_in_order() {
        let folder = TempFolder::new();
        let script = folder.write_script("card", "", "", 0);
        let cards = CardCli::new(script.clone());

        cards
            .link_session(&"DEMO-KB-007".parse().unwrap(), "demo-kb-007-impl")
            .unwrap();

        assert_eq!(
            read_script_calls(&script),
            [["link", "DEMO-KB-007", "--session", "demo-kb-007-impl"]]
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

mod git {
    use std::fs;
    use std::path::Path;

    use hub::external::git::{Unsaved, check_unsaved, find_card_worktrees};

    use super::common::{TempFolder, make_card_repo, run_git};

    /// A finished worker's tab may close only when its worktree holds
    /// nothing that exists nowhere else. Missing uncommitted files or
    /// unpushed commits would close the last sign of that work. The
    /// agent works from the main checkout here, so only the branch
    /// name ties the worktree to its card. The repository hides new
    /// files from a plain `git status`, as large repositories often do,
    /// and the check must still find them.
    #[test]
    fn check_unsaved_finds_files_and_commits() {
        struct Case {
            name: &'static str,
            leave_work: fn(&Path),
            expected: fn(&Path) -> Unsaved,
        }
        let cases = [
            Case {
                name: "clean",
                leave_work: |_| {},
                expected: |_| Unsaved::Clean,
            },
            Case {
                name: "files",
                leave_work: |worktree| fs::write(worktree.join("notes.txt"), "x").unwrap(),
                expected: |worktree| Unsaved::Files(worktree.to_owned()),
            },
            Case {
                name: "commits",
                leave_work: |worktree| run_git(worktree, &["commit", "--allow-empty", "-m", "fix"]),
                expected: |worktree| Unsaved::Commits(worktree.to_owned()),
            },
        ];
        let folder = TempFolder::new();

        for case in cases {
            let main = make_card_repo(&folder.path().join(case.name));
            run_git(&main, &["config", "status.showUntrackedFiles", "no"]);
            let worktree = folder
                .path()
                .join(case.name)
                .join("card-wt")
                .canonicalize()
                .unwrap();
            (case.leave_work)(&worktree);

            let found =
                find_card_worktrees(&[main], Some(&"DEMO-KB-001".parse().unwrap())).unwrap();
            let unsaved = check_unsaved(&found[0]).unwrap();

            assert_eq!(unsaved, (case.expected)(&worktree), "{}", case.name);
            assert_eq!(found, [worktree], "{}", case.name);
        }
    }
}

mod requests {
    use std::fs;

    use hub::external::requests::RequestFolder;
    use serde_json::json;

    use super::common::TempFolder;

    /// A request that already ran must not keep a session open forever.
    /// An approved one has not run `gh` yet, so its session must stay.
    /// One file that is not JSON must not break `hub status`; it is
    /// counted for one problem line instead.
    #[test]
    fn list_open_requests_skips_ran_and_unreadable() {
        let folder = TempFolder::new();
        let write = |name: &str, text: String| fs::write(folder.path().join(name), text).unwrap();
        write(
            "a.json",
            json!({"state": "pending", "pane_id": "w1:p7"}).to_string(),
        );
        write(
            "b.json",
            json!({"state": "running", "session_id": "ses_b"}).to_string(),
        );
        write(
            "c.json",
            json!({"state": "ran", "pane_id": "w1:p8"}).to_string(),
        );
        write(
            "f.json",
            json!({"state": "approved", "pane_id": "w1:p6"}).to_string(),
        );
        write(
            "d.json",
            json!({"state": "declined", "pane_id": "w1:p9"}).to_string(),
        );
        write("e.json", "{cut off".to_owned());
        write("notes.txt", "not a request".to_owned());

        let open = RequestFolder::new(folder.path().to_owned())
            .list_open_requests()
            .unwrap();

        let mut found: Vec<(Option<String>, Option<String>)> = open
            .requests
            .into_iter()
            .map(|request| (request.pane_id, request.session_id))
            .collect();
        found.sort();
        assert_eq!(
            found,
            [
                (None, Some("ses_b".to_owned())),
                (Some("w1:p6".to_owned()), None),
                (Some("w1:p7".to_owned()), None),
            ]
        );
        assert_eq!(open.bad_files, 1);
    }
}

mod opencode {
    use std::path::Path;

    use hub::external::opencode::OpenCodeDb;

    use super::common::{TempFolder, write_opencode_session};

    /// Reopening starts OpenCode in the session's own folder, and
    /// `--session` with an ID the database lacks starts an empty session.
    /// A wrong folder, or a missing session read as present, sends the
    /// review comments to a session that knows nothing of the work.
    #[test]
    fn find_session_returns_folder_or_none() {
        let folder = TempFolder::new();
        let database = folder.path().join("opencode.db");
        write_opencode_session(&database, "ses_007", Path::new("/home/u/wt/demo-kb-007"));
        let opencode = OpenCodeDb::new(database);

        let found = opencode.find_session("ses_007").unwrap();
        let missing = opencode.find_session("ses_gone").unwrap();

        assert_eq!(found.as_deref(), Some(Path::new("/home/u/wt/demo-kb-007")));
        assert_eq!(missing, None);
    }
}

mod processes {
    use std::io::{BufRead, BufReader};
    use std::process::{Child, Command, Stdio};

    use hub::external::processes::has_shell_jobs;

    use super::common::TempFolder;

    /// A stand-in shell that starts `job` in the background and waits
    /// for its input to close. It prints the job's process ID once the
    /// job is ready.
    fn start_shell(job: &str) -> (Child, String) {
        let script = format!("{job} & read line");
        let mut shell = Command::new("sh")
            .args(["-c", &script])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let mut job_pid = String::new();
        BufReader::new(shell.stdout.take().unwrap())
            .read_line(&mut job_pid)
            .unwrap();
        (shell, job_pid.trim().to_owned())
    }

    /// A tab whose shell holds an editor stopped with Ctrl-Z looks idle
    /// to Herdr, and closing it would throw away the user's open file.
    /// The stopped job sits in a process group of its own, as a shell
    /// with job control puts it. A child in the shell's own group is
    /// no job, or every quiet shell would keep its tab.
    #[test]
    fn has_shell_jobs_finds_stopped_job() {
        let _folder = TempFolder::new();
        // `setsid` gives the job a group of its own before it stops.
        let (stopped, stopped_job) = start_shell("setsid sh -c 'echo $$; kill -STOP $$'");
        let (plain, plain_job) = start_shell("sh -c 'echo $$; exec sleep 60'");

        let stopped_found = has_shell_jobs(stopped.id()).unwrap();
        let plain_found = has_shell_jobs(plain.id()).unwrap();

        // End both shells and their jobs, then check.
        for (mut shell, job) in [(stopped, stopped_job), (plain, plain_job)] {
            Command::new("kill").args(["-KILL", &job]).status().unwrap();
            drop(shell.stdin.take());
            shell.wait().unwrap();
        }
        assert!(stopped_found);
        assert!(!plain_found);
    }
}
