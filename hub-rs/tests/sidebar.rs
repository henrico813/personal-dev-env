mod common;

mod go {
    use hub::commands::sidebar::run_go;
    use hub::external::herdr::{Agent, AgentStatus};
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
    use hub::external::herdr::{Agent, AgentStatus, Workspace};
    use hub::shared::{HubError, Tools};

    use super::common::{InMemoryHerdr, TestTools, make_agent, make_workspaces};

    /// `hub handoff`, `hub adopt` and `hub auto now` gather, so the
    /// hubs sit together at the top of the sidebar. Herdr refuses a
    /// block that names a workspace twice or goes before one of its
    /// own members, and that refusal is what would leave the hubs
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
    /// nothing. Otherwise it reports "at the top" while no hub
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

    /// The user finds the kanban hub at the very top and the desk right
    /// below it, on the phone as on the laptop. The other hubs follow.
    /// A desk or hub left among the workers, or out of this order, sends
    /// the user scrolling again.
    #[test]
    fn run_gather_orders_kanban_hubs_desk_others() {
        let herdr = InMemoryHerdr {
            agents: vec![
                Agent {
                    workspace_id: "w2".to_owned(),
                    ..make_agent("demo-kb-001-impl", AgentStatus::Idle)
                },
                Agent {
                    workspace_id: "w3".to_owned(),
                    ..make_agent("auto-hub-1", AgentStatus::Done)
                },
                Agent {
                    workspace_id: "w4".to_owned(),
                    ..make_agent("kanban-hub-13", AgentStatus::Working)
                },
            ],
            workspaces: vec![
                Workspace {
                    workspace_id: "w1".to_owned(),
                    label: "desk".to_owned(),
                },
                Workspace {
                    workspace_id: "w2".to_owned(),
                    label: "DEMO-KB-001 impl".to_owned(),
                },
                Workspace {
                    workspace_id: "w3".to_owned(),
                    label: "auto".to_owned(),
                },
                Workspace {
                    workspace_id: "w4".to_owned(),
                    label: "kanban".to_owned(),
                },
            ],
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
            [(
                vec!["w4".to_owned(), "w1".to_owned(), "w3".to_owned()],
                "w2".to_owned()
            )]
        );
    }

    /// A hub that starts alone gathers while every workspace holds a
    /// hub. Herdr refuses a block placed before one of its own
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

mod desk {
    use std::path::Path;

    use hub::commands::sidebar::run_desk;
    use hub::external::herdr::{Agent, AgentStatus, Workspace};
    use hub::shared::Tools;

    use super::common::{InMemoryHerdr, TestTools, make_agent, make_workspaces};

    /// The desk's first tab shows on the phone, so the board must run
    /// there and the shell must come second. A board in the shell's
    /// place leaves the user with a bare prompt on the phone.
    #[test]
    fn run_desk_makes_board_then_shell() {
        let herdr = InMemoryHerdr {
            workspaces: make_workspaces(&["w1"]),
            ..InMemoryHerdr::default()
        };
        let test_tools = TestTools::new();
        let tools = Tools {
            herdr: &herdr,
            ..test_tools.tools()
        };

        run_desk(&tools, Path::new("/home/u")).unwrap();

        let created = herdr.created_workspaces.borrow();
        assert_eq!(created.len(), 1);
        assert_eq!(created[0].label, "desk");
        assert_eq!(
            *herdr.ran_in_panes.borrow(),
            [("new:p-desk".to_owned(), "hub board".to_owned())]
        );
        let tabs = herdr.created_tabs.borrow();
        assert_eq!(tabs.len(), 1);
        assert_eq!(
            (tabs[0].0.as_str(), tabs[0].1.label.as_str()),
            ("new", "shell")
        );
    }

    /// The user runs `hub desk` to open the desk or bring it back, so
    /// the desk must land at the top, right below the kanban hubs,
    /// whether it was just made or already open, and with no hub
    /// running. A desk made twice, or left where Herdr put it, sends the
    /// user scrolling again.
    #[test]
    fn run_desk_puts_desk_below_kanban_hubs() {
        let cases = [
            (
                "made-below-kanban-hub",
                vec![
                    Agent {
                        workspace_id: "w1".to_owned(),
                        ..make_agent("kanban-hub-13", AgentStatus::Working)
                    },
                    Agent {
                        workspace_id: "w2".to_owned(),
                        ..make_agent("demo-kb-001-impl", AgentStatus::Idle)
                    },
                ],
                vec![("w1", "kanban"), ("w2", "DEMO-KB-001 impl")],
                (vec!["w1", "new"], "w2"),
            ),
            (
                "open-desk-top-without-hub",
                vec![Agent {
                    workspace_id: "w1".to_owned(),
                    ..make_agent("demo-kb-001-impl", AgentStatus::Idle)
                }],
                vec![("w1", "DEMO-KB-001 impl"), ("w2", "desk")],
                (vec!["w2"], "w1"),
            ),
        ];

        for (case_name, agents, workspaces, (block, before)) in cases {
            let herdr = InMemoryHerdr {
                agents,
                workspaces: workspaces
                    .into_iter()
                    .map(|(id, label)| Workspace {
                        workspace_id: id.to_owned(),
                        label: label.to_owned(),
                    })
                    .collect(),
                ..InMemoryHerdr::default()
            };
            let test_tools = TestTools::new();
            let tools = Tools {
                herdr: &herdr,
                ..test_tools.tools()
            };

            run_desk(&tools, Path::new("/home/u")).unwrap();

            let block: Vec<String> = block.into_iter().map(str::to_owned).collect();
            assert_eq!(
                *herdr.moves.borrow(),
                [(block, before.to_owned())],
                "{case_name}"
            );
        }
    }
}

mod agent_filter {
    use std::path::Path;

    use hub::commands::sidebar::{run_desk, run_gather};
    use hub::external::herdr::{Agent, AgentStatus, HubKind, Workspace};
    use hub::shared::Tools;

    use super::common::{InMemoryHerdr, TestTools, make_agent, make_workspaces};

    /// Runs one sidebar command and fails the test on its error.
    type RunCommand = fn(&Tools<'_>);

    /// Each hub start gathers, and the user runs `desk`. Both must
    /// tag every hub and the desk, mark a flagged worker again and leave
    /// a plain worker alone, then filter the agent list once. A missing
    /// tag or mark drops that row from the user's "go to" list. A wrong
    /// hub kind puts that hub's row in the wrong place.
    #[test]
    fn gather_and_desk_fill_go_to_list() {
        let cases: [(&str, RunCommand); 2] = [
            ("gather", |tools| {
                run_gather(tools).unwrap();
            }),
            ("desk", |tools| {
                run_desk(tools, Path::new("/home/u")).unwrap();
            }),
        ];

        for (case_name, run) in cases {
            let herdr = InMemoryHerdr {
                agents: vec![
                    Agent {
                        workspace_id: "w3".to_owned(),
                        ..make_agent("auto-hub-1", AgentStatus::Done)
                    },
                    Agent {
                        workspace_id: "w2".to_owned(),
                        ..make_agent("demo-kb-001-impl", AgentStatus::Idle)
                    },
                    Agent {
                        workspace_id: "w2".to_owned(),
                        marked: true,
                        ..make_agent("demo-kb-002-review", AgentStatus::Done)
                    },
                    make_agent("kanban-hub-13", AgentStatus::Working),
                ],
                workspaces: [
                    make_workspaces(&["w1", "w2", "w3"]),
                    vec![Workspace {
                        workspace_id: "w4".to_owned(),
                        label: "desk".to_owned(),
                    }],
                ]
                .concat(),
                ..InMemoryHerdr::default()
            };
            let test_tools = TestTools::new();
            let tools = Tools {
                herdr: &herdr,
                ..test_tools.tools()
            };

            run(&tools);

            assert_eq!(
                *herdr.tagged_hubs.borrow(),
                [
                    ("w1:kanban-hub-13".to_owned(), HubKind::Kanban),
                    ("w1:auto-hub-1".to_owned(), HubKind::Other),
                ],
                "{case_name}"
            );
            assert_eq!(*herdr.desk_agents.borrow(), ["w4"], "{case_name}");
            assert_eq!(
                *herdr.marks.borrow(),
                [(
                    "w1:demo-kb-002-review".to_owned(),
                    vec![AgentStatus::Idle, AgentStatus::Done]
                )],
                "{case_name}"
            );
            assert_eq!(herdr.agent_filters.get(), 1, "{case_name}");
        }
    }
}
