mod common;

mod flag {
    use hub::commands::flag::{run_flag, run_unflag};
    use hub::outside::herdr::{Agent, AgentStatus};
    use hub::shared::{AgentName, Tools};

    use super::common::{InMemoryHerdr, TestTools, make_agent};

    /// The user flags a session to see "needs you" in the sidebar, and
    /// unflags it after. Herdr marks panes, so the agent name in place
    /// of its pane ID marks nothing. A blocked session marked on idle
    /// would keep the mark after the user answers.
    #[test]
    fn run_flag_marks_pane_and_unflag_clears() {
        let cases = [
            (
                "idle",
                AgentStatus::Idle,
                vec![AgentStatus::Idle, AgentStatus::Done],
            ),
            ("blocked", AgentStatus::Blocked, vec![AgentStatus::Blocked]),
        ];

        for (case_name, status, expected_states) in cases {
            let herdr = InMemoryHerdr {
                agents: vec![Agent {
                    pane_id: "w4:p2".to_owned(),
                    ..make_agent("docs-fix", status)
                }],
                ..InMemoryHerdr::default()
            };
            let test_tools = TestTools::new();
            let tools = Tools {
                herdr: &herdr,
                ..test_tools.tools()
            };
            let name: AgentName = "docs-fix".parse().unwrap();

            run_flag(&tools, &name).unwrap();
            run_unflag(&tools, &name).unwrap();

            assert_eq!(
                *herdr.marks.borrow(),
                [("w4:p2".to_owned(), expected_states)],
                "{case_name}"
            );
            assert_eq!(*herdr.cleared.borrow(), ["w4:p2"], "{case_name}");
        }
    }
}

mod go {
    use hub::commands::sidebar::run_go;
    use hub::outside::herdr::{Agent, AgentStatus};
    use hub::shared::Tools;

    use super::common::{InMemoryHerdr, TestTools, make_agent, make_workspaces};

    /// The user presses `ctrl+b shift+h` again and again to move
    /// between hubs in sidebar order. A wrong order or a missing wrap
    /// leaves a hub they cannot reach by pressing again. Name order
    /// differs from sidebar order here, so a sort by name fails.
    #[test]
    fn run_go_shows_hub_after_focused_one() {
        let cases = [
            ("not-on-a-hub", "demo-kb-001-impl", "kanban-hub-13"),
            ("next-hub", "kanban-hub-13", "auto-hub-1"),
            ("wraps-to-first", "auto-hub-1", "kanban-hub-13"),
        ];

        for (case_name, focused, expected) in cases {
            let agents = [
                ("auto-hub-1", "w3"),
                ("demo-kb-001-impl", "w1"),
                ("kanban-hub-13", "w2"),
            ]
            .into_iter()
            .map(|(name, workspace_id)| Agent {
                workspace_id: workspace_id.to_owned(),
                focused: name == focused,
                ..make_agent(name, AgentStatus::Idle)
            })
            .collect();
            let herdr = InMemoryHerdr {
                agents,
                workspaces: make_workspaces(&["w1", "w2", "w3"]),
                ..InMemoryHerdr::default()
            };
            let test_tools = TestTools::new();
            let tools = Tools {
                herdr: &herdr,
                ..test_tools.tools()
            };

            run_go(&tools).unwrap();

            assert_eq!(*herdr.focused.borrow(), [expected], "{case_name}");
        }
    }
}

mod gather {
    use hub::commands::sidebar::run_gather;
    use hub::outside::herdr::{Agent, AgentStatus};
    use hub::shared::{HubError, Tools};

    use super::common::{InMemoryHerdr, TestTools, make_agent, make_workspaces};

    /// A hub runs `hub gather` from its own pane at each start, so the
    /// hubs sit together at the top of the sidebar. Herdr refuses a
    /// block that names a workspace twice or goes before one of its own
    /// members. Without that refusal, a bad block would leave the hubs
    /// below the workers.
    #[test]
    fn run_gather_moves_hub_workspaces_to_top() {
        let herdr = InMemoryHerdr {
            agents: vec![
                Agent {
                    workspace_id: "w3".to_owned(),
                    ..make_agent("auto-hub-1", AgentStatus::Done)
                },
                Agent {
                    workspace_id: "w3".to_owned(),
                    ..make_agent("auto-hub-2", AgentStatus::Done)
                },
                Agent {
                    workspace_id: "w2".to_owned(),
                    ..make_agent("demo-kb-001-impl", AgentStatus::Idle)
                },
                Agent {
                    workspace_id: "w1".to_owned(),
                    ..make_agent("kanban-hub-13", AgentStatus::Working)
                },
            ],
            workspaces: make_workspaces(&["w1", "w2", "w3"]),
            ..InMemoryHerdr::default()
        };
        let test_tools = TestTools::new();
        let tools = Tools {
            herdr: &herdr,
            ..test_tools.tools()
        };

        run_gather(&tools).unwrap();

        assert_eq!(
            *herdr.moves.borrow(),
            [(vec!["w1".to_owned(), "w3".to_owned()], "w2".to_owned())]
        );
    }

    /// With no hub running, `gather` must stop with exit 3 and move
    /// nothing. Otherwise it reports "hubs at the top" while no hub
    /// moved.
    #[test]
    fn run_gather_stops_when_no_hub_runs() {
        let herdr = InMemoryHerdr {
            agents: vec![make_agent("demo-kb-001-impl", AgentStatus::Idle)],
            workspaces: make_workspaces(&["w1", "w2"]),
            ..InMemoryHerdr::default()
        };
        let test_tools = TestTools::new();
        let tools = Tools {
            herdr: &herdr,
            ..test_tools.tools()
        };

        let error = run_gather(&tools).unwrap_err();

        assert!(matches!(error, HubError::NotDone(_)), "{error:?}");
        assert!(herdr.moves.borrow().is_empty());
    }

    /// A hub that starts alone runs `hub gather` while every workspace
    /// holds a hub. Herdr refuses a block placed before one of its own
    /// members, so a move then would fail each such start.
    #[test]
    fn run_gather_leaves_hub_only_sidebar_alone() {
        let herdr = InMemoryHerdr {
            agents: vec![
                make_agent("kanban-hub-13", AgentStatus::Idle),
                Agent {
                    workspace_id: "w2".to_owned(),
                    ..make_agent("auto-hub-1", AgentStatus::Done)
                },
            ],
            workspaces: make_workspaces(&["w1", "w2"]),
            ..InMemoryHerdr::default()
        };
        let test_tools = TestTools::new();
        let tools = Tools {
            herdr: &herdr,
            ..test_tools.tools()
        };

        run_gather(&tools).unwrap();

        assert!(herdr.moves.borrow().is_empty());
    }
}

mod brief {
    use std::fs;

    use hub::commands::brief::run_brief;
    use hub::outside::cards::BoardCounts;
    use hub::shared::HubError;

    use super::common::TestTools;

    /// A whole `decisions.md` entry, as the hub writes one.
    const DECISION: &str = "## New model overnight\n\
                            \n\
                            Deploy the new model tonight so you can test it in the morning?\n\
                            \n\
                            - A. Deploy from the branch, with the undo path ready.\n\
                            - B. Wait until you return.\n\
                            \n\
                            Recommended: A, since you want to test it in the morning.\n\
                            If no reply: A.\n";

    /// The user decides from this one message, often away from the
    /// desk. Hubs once named questions by number alone and the user
    /// had to ask what they were, so every line of each entry prints,
    /// after the board line and the pull requests.
    #[test]
    fn run_brief_prints_board_reviews_then_decisions() {
        let mut test_tools = TestTools::new();
        test_tools.cards.board = Some(BoardCounts {
            review: 19,
            busy: 24,
            limit: 8,
        });
        fs::write(
            test_tools.run_path().join("reviews.md"),
            "# PRs\n\n| #1 | Fix |\n",
        )
        .unwrap();
        fs::write(test_tools.run_path().join("decisions.md"), DECISION).unwrap();

        let outcome = run_brief(&test_tools.tools(), &test_tools.run()).unwrap();

        assert_eq!(
            outcome.lines,
            [
                "board 24/8, review 19; 1 decision",
                "",
                "# PRs",
                "",
                "| #1 | Fix |",
                "",
                "Decision: New model overnight",
                "Deploy the new model tonight so you can test it in the morning?",
                "- A. Deploy from the branch, with the undo path ready.",
                "- B. Wait until you return.",
                "Recommended: A, since you want to test it in the morning.",
                "If no reply: A.",
            ]
        );
    }

    /// The Python hub printed `Nothing waits on the user.` when nothing
    /// waits, and the user reads it as the all-clear. Without it the
    /// brief is a bare board line that looks cut off.
    #[test]
    fn run_brief_says_when_nothing_waits() {
        let mut test_tools = TestTools::new();
        test_tools.cards.board = Some(BoardCounts {
            review: 0,
            busy: 3,
            limit: 8,
        });

        let outcome = run_brief(&test_tools.tools(), &test_tools.run()).unwrap();

        assert_eq!(
            outcome.lines,
            [
                "board 3/8, review 0; 0 decisions",
                "",
                "Nothing waits on the user.",
            ]
        );
    }

    /// An entry without a recommendation or a no-reply default cannot
    /// go to a user who decides from the message alone. The command
    /// names each missing part, so the hub can fix the entry.
    #[test]
    fn run_brief_stops_on_entry_missing_parts() {
        let test_tools = TestTools::new();
        fs::write(
            test_tools.run_path().join("decisions.md"),
            "## Pause cards\n\nPause stale cards?\n\n- A. Yes\n- B. No\n",
        )
        .unwrap();

        let error = run_brief(&test_tools.tools(), &test_tools.run()).unwrap_err();

        let HubError::NotDone(message) = error else {
            panic!("expected NotDone, got {error:?}");
        };
        assert_eq!(
            message,
            "decisions.md entry 'Pause cards' lacks the Recommended: line, \
             the If no reply: line"
        );
    }
}

mod alive {
    use hub::commands::alive::run_alive;
    use hub::outside::opencode::OpenCodeError;
    use hub::shared::HubError;
    use serde_json::json;

    use super::common::{OpenCodeRow, TestTools, make_time, write_opencode_db};

    /// The hub reads this line to tell the user whether a session still
    /// works. The newest row decides the time, and the last tool comes
    /// from the newest step that called one, even when newer rows called
    /// none.
    #[test]
    fn run_alive_prints_time_and_last_tool() {
        let test_tools = TestTools::new();
        let now = make_time("2026-11-11T22:14:00+00:00[UTC]");
        let now_ms = now.timestamp().as_millisecond();
        write_opencode_db(
            &test_tools.opencode_path(),
            &[
                OpenCodeRow {
                    session: "ses_a",
                    kind: "assistant",
                    updated_ms: now_ms - 20 * 60 * 1000,
                    data: json!({"content": [{"type": "tool", "name": "shell"}]}),
                },
                OpenCodeRow {
                    session: "ses_a",
                    kind: "user",
                    updated_ms: now_ms - 30 * 1000,
                    data: json!({}),
                },
            ],
        );

        let outcome = run_alive(&test_tools.tools(), "ses_a", &now).unwrap();

        assert_eq!(
            outcome.lines,
            ["2 messages, newest 22:13:30, last tool shell, working"]
        );
    }

    /// A mistyped session ID must not read as a stopped session, or the
    /// hub would tell the user a working session died.
    #[test]
    fn run_alive_unknown_session_fails() {
        let test_tools = TestTools::new();
        let now = make_time("2026-11-11T22:14:00+00:00[UTC]");
        write_opencode_db(
            &test_tools.opencode_path(),
            &[OpenCodeRow {
                session: "ses_a",
                kind: "user",
                updated_ms: 1,
                data: json!({}),
            }],
        );

        let error = run_alive(&test_tools.tools(), "ses_b", &now).unwrap_err();

        assert!(
            matches!(error, HubError::OpenCode(OpenCodeError::NoSession(_))),
            "{error:?}"
        );
    }
}
