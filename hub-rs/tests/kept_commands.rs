mod common;

mod flag {
    use hub::commands::flag::{run_flag, run_unflag};
    use hub::outside::herdr::{Agent, AgentStatus};
    use hub::shared::{AgentName, Tools};

    use super::common::{InMemoryHerdr, make_agent};

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
            let tools = Tools { herdr: &herdr };
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

    use super::common::{InMemoryHerdr, make_agent, make_workspaces};

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
            let tools = Tools { herdr: &herdr };

            run_go(&tools).unwrap();

            assert_eq!(*herdr.focused.borrow(), [expected], "{case_name}");
        }
    }
}

mod gather {
    use hub::commands::sidebar::run_gather;
    use hub::outside::herdr::{Agent, AgentStatus};
    use hub::shared::{HubError, Tools};

    use super::common::{InMemoryHerdr, make_agent, make_workspaces};

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
        let tools = Tools { herdr: &herdr };

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
        let tools = Tools { herdr: &herdr };

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
        let tools = Tools { herdr: &herdr };

        run_gather(&tools).unwrap();

        assert!(herdr.moves.borrow().is_empty());
    }
}
