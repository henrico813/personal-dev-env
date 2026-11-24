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
    use std::collections::BTreeMap;
    use std::path::Path;

    use hub::commands::sidebar::{run_desk, run_gather};
    use hub::external::herdr::{Agent, AgentStatus, Workspace};
    use hub::shared::Tools;

    use super::common::{InMemoryHerdr, TestTools, make_agent, make_workspaces};

    /// Runs one sidebar command and fails the test on its error.
    type RunCommand = fn(&Tools<'_>);

    /// An agent named `name` in the desk, workspace `w4`, in tab
    /// `w4:t-<name>`.
    fn make_desk_agent(name: &str) -> Agent {
        Agent {
            workspace_id: "w4".to_owned(),
            tab_id: format!("w4:t-{name}"),
            ..make_agent(name, AgentStatus::Idle)
        }
    }

    /// Each hub start gathers, and the user runs `desk`. Both must tag
    /// only the newest kanban hub as the front hub and each desk agent by
    /// its tab, clear the tokens a handed-off hub and an auto hub kept,
    /// mark a flagged worker again, then filter the agent list once.
    /// A missing tag drops a row from the user's "go to" list, a kept
    /// one leaves an old hub above the desk, and a desk sorted by name
    /// breaks the user's tab order. A token already right gets no write,
    /// since `hub watch` runs this every 5 seconds.
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
                        go_to: Some("3-hub".to_owned()),
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
                    Agent {
                        go_to: Some("1-kanban-hub".to_owned()),
                        ..make_agent("kanban-hub-12", AgentStatus::Idle)
                    },
                    make_agent("kanban-hub-13", AgentStatus::Working),
                    make_desk_agent("routine"),
                    Agent {
                        go_to: Some("2-desk-1-02".to_owned()),
                        ..make_desk_agent("notes")
                    },
                ],
                workspaces: [
                    make_workspaces(&["w1", "w2", "w3"]),
                    vec![Workspace {
                        workspace_id: "w4".to_owned(),
                        label: "desk".to_owned(),
                    }],
                ]
                .concat(),
                tabs: BTreeMap::from([(
                    "w4".to_owned(),
                    vec![
                        "w4:t-board".to_owned(),
                        "w4:t-routine".to_owned(),
                        "w4:t-notes".to_owned(),
                    ],
                )]),
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
                [("w1:kanban-hub-13".to_owned(), "1-hub-1".to_owned())],
                "{case_name}"
            );
            assert_eq!(
                *herdr.go_to_writes.borrow(),
                [
                    ("w1:auto-hub-1".to_owned(), None),
                    ("w1:kanban-hub-12".to_owned(), None),
                    ("w1:routine".to_owned(), Some("2-desk-1-01".to_owned())),
                ],
                "{case_name}"
            );
            assert_eq!(
                *herdr.desk_agents.borrow(),
                [("w4:p-board".to_owned(), "2-desk-1-00".to_owned())],
                "{case_name}"
            );
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

mod restore_sidebar {
    use std::fs;
    use std::time::Duration;

    use hub::commands::next::{NextOptions, run_next};
    use hub::commands::sidebar::run_restore_sidebar;
    use hub::external::herdr::{Agent, AgentStatus, Workspace};
    use hub::shared::Tools;
    use hub::shared::run_folder::RunFolder;

    use super::common::{InMemoryHerdr, TestTools, make_agent};

    /// A Herdr in the state its restart leaves, with a kanban hub that
    /// lost its tag, a worker, and a desk in `w4` whose board pane no
    /// longer shows as the desk agent.
    fn make_restarted_herdr() -> InMemoryHerdr {
        InMemoryHerdr {
            agents: vec![
                make_agent("kanban-hub-13", AgentStatus::Idle),
                make_agent("demo-kb-001-impl", AgentStatus::Idle),
            ],
            workspaces: vec![
                Workspace {
                    workspace_id: "w1".to_owned(),
                    label: "kanban".to_owned(),
                },
                Workspace {
                    workspace_id: "w4".to_owned(),
                    label: "desk".to_owned(),
                },
            ],
            ..InMemoryHerdr::default()
        }
    }

    /// Runs one `hub next` check in the run folder `herdr-hub.<word>` of
    /// the runs folder, as the hub whose session is `session_id`.
    fn run_hub_next(test_tools: &mut TestTools, word: &str, session_id: &str) -> RunFolder {
        let path = test_tools.path().join(format!("herdr-hub.{word}"));
        fs::create_dir_all(&path).unwrap();
        let run = RunFolder::open(&path).unwrap();
        test_tools.own_session = Some(session_id.to_owned());
        let options = NextOptions {
            timeout: Some(Duration::ZERO),
            poll: Duration::ZERO,
            json: false,
            process_id: 1,
            cwd: test_tools.path().to_owned(),
            start_reviews: false,
        };
        run_next(&test_tools.tools(), &run, &options).unwrap();
        run
    }

    /// Herdr runs this when it starts, after a restart dropped every
    /// tag, the desk row and the filter. A hub or desk left out stays
    /// missing from the top of the user's list until someone acts.
    #[test]
    fn run_restore_sidebar_restores_hubs_and_desk() {
        let herdr = make_restarted_herdr();
        let test_tools = TestTools::new();
        let tools = Tools {
            herdr: &herdr,
            ..test_tools.tools()
        };

        let outcome = run_restore_sidebar(&tools, false).unwrap();

        assert_eq!(outcome.lines, ["changed: kanban-hub-13 desk"]);
        assert_eq!(
            *herdr.tagged_hubs.borrow(),
            [("w1:kanban-hub-13".to_owned(), "1-hub-1".to_owned())]
        );
        assert_eq!(
            *herdr.desk_agents.borrow(),
            [("w4:p-board".to_owned(), "2-desk-1-00".to_owned())]
        );
        assert_eq!(herdr.agent_filters.get(), 1);
    }

    /// The front hub is the one that last ran `hub next` in a run of
    /// the user's own, even when Herdr lists it with no name and an
    /// older named hub of the same kind stays open. An auto run's hub,
    /// also with no name, that ran `hub next` later must not take its
    /// place, or the user's main hub drops below the desk.
    #[test]
    fn run_restore_sidebar_takes_recorded_front_hub() {
        let mut test_tools = TestTools::new();
        test_tools.herdr.agents = vec![
            Agent {
                session_id: Some("ses_old".to_owned()),
                go_to: Some("1-hub-1".to_owned()),
                ..make_agent("kanban-hub-29", AgentStatus::Idle)
            },
            Agent {
                name: String::new(),
                pane_id: "w2:p1".to_owned(),
                session_id: Some("ses_front".to_owned()),
                ..make_agent("", AgentStatus::Working)
            },
            Agent {
                name: String::new(),
                pane_id: "w3:p1".to_owned(),
                session_id: Some("ses_auto".to_owned()),
                ..make_agent("", AgentStatus::Working)
            },
        ];
        run_hub_next(&mut test_tools, "kanban", "ses_front");
        let auto_run = run_hub_next(&mut test_tools, "auto", "ses_auto");
        fs::write(auto_run.path().join("approved.md"), "Approved: run 2\n").unwrap();

        run_restore_sidebar(&test_tools.tools(), false).unwrap();

        assert_eq!(
            *test_tools.herdr.tagged_hubs.borrow(),
            [("w2:p1".to_owned(), "1-hub-1".to_owned())]
        );
        assert_eq!(
            *test_tools.herdr.go_to_writes.borrow(),
            [("w1:kanban-hub-29".to_owned(), None)]
        );
    }

    /// The user can run a second hub of their own beside the main one.
    /// Each must keep a row at the top, the older run's first however
    /// the two take turns running `hub next`, or one hub drops out of
    /// the user's list.
    #[test]
    fn run_restore_sidebar_tags_each_runs_hub() {
        let mut test_tools = TestTools::new();
        test_tools.herdr.agents = vec![
            Agent {
                session_id: Some("ses_main".to_owned()),
                ..make_agent("kanban-hub-7", AgentStatus::Working)
            },
            Agent {
                session_id: Some("ses_side".to_owned()),
                ..make_agent("side-hub-3", AgentStatus::Idle)
            },
        ];
        run_hub_next(&mut test_tools, "main", "ses_main");
        run_hub_next(&mut test_tools, "side", "ses_side");

        run_restore_sidebar(&test_tools.tools(), false).unwrap();

        assert_eq!(
            *test_tools.herdr.tagged_hubs.borrow(),
            [
                ("w1:kanban-hub-7".to_owned(), "1-hub-1".to_owned()),
                ("w1:side-hub-3".to_owned(), "1-hub-1-01".to_owned()),
            ]
        );
    }

    /// A worker may loop on `hub next` in a run folder of its own, as
    /// a quiet-hours script does. It must not take the front hub's row
    /// from the hub the user works with.
    #[test]
    fn run_restore_sidebar_skips_recorded_worker() {
        let mut test_tools = TestTools::new();
        test_tools.herdr.agents = vec![
            make_agent("kanban-hub-29", AgentStatus::Idle),
            Agent {
                session_id: Some("ses_worker".to_owned()),
                ..make_agent("demo-kb-172-quiet-2", AgentStatus::Working)
            },
        ];
        run_hub_next(&mut test_tools, "quiet", "ses_worker");

        run_restore_sidebar(&test_tools.tools(), false).unwrap();

        assert_eq!(
            *test_tools.herdr.tagged_hubs.borrow(),
            [("w1:kanban-hub-29".to_owned(), "1-hub-1".to_owned())]
        );
    }

    /// Herdr restores the desk with a new shell, so the board must
    /// start there again. When the shell runs a program, such as an
    /// editor, typing `hub board` would land in that program.
    #[test]
    fn run_restore_sidebar_starts_board_when_quiet() {
        let cases = [
            ("quiet-shell", true, vec![("w4:p-board", "hub board")]),
            ("program-in-front", false, vec![]),
        ];

        for (case_name, quiet, expected_runs) in cases {
            let mut herdr = make_restarted_herdr();
            let board = vec!["w4:p-board".to_owned()];
            if quiet {
                herdr.quiet_shells = board;
            } else {
                herdr.program_in_front = board;
            }
            let test_tools = TestTools::new();
            let tools = Tools {
                herdr: &herdr,
                ..test_tools.tools()
            };

            run_restore_sidebar(&tools, true).unwrap();

            let expected: Vec<(String, String)> = expected_runs
                .into_iter()
                .map(|(pane, command)| (pane.to_owned(), command.to_owned()))
                .collect();
            assert_eq!(*herdr.ran_in_panes.borrow(), expected, "{case_name}");
        }
    }
}

mod desk_steps {
    use std::collections::BTreeMap;

    use hub::commands::sidebar::run_desk_move;
    use hub::external::herdr::{Agent, AgentStatus, Herdr, Workspace};
    use hub::shared::{AgentName, Tools};

    use super::common::{InMemoryHerdr, TestTools, make_agent, make_workspaces};

    /// Herdr with a worker workspace `w2` and the desk, `w4`, holding
    /// `agents` in `desk_tabs` order.
    fn make_desk_herdr(agents: Vec<Agent>, desk_tabs: &[&str]) -> InMemoryHerdr {
        InMemoryHerdr {
            agents,
            workspaces: [
                make_workspaces(&["w2"]),
                vec![Workspace {
                    workspace_id: "w4".to_owned(),
                    label: "desk".to_owned(),
                }],
            ]
            .concat(),
            tabs: BTreeMap::from([(
                "w4".to_owned(),
                desk_tabs.iter().map(|tab| (*tab).to_owned()).collect(),
            )]),
            ..InMemoryHerdr::default()
        }
    }

    /// An agent named `name` in workspace `workspace_id`, in tab
    /// `<workspace_id>:t-<name>`.
    fn make_agent_in(name: &str, workspace_id: &str) -> Agent {
        Agent {
            workspace_id: workspace_id.to_owned(),
            tab_id: format!("{workspace_id}:t-{name}"),
            ..make_agent(name, AgentStatus::Idle)
        }
    }

    /// The user's agent name, parsed as `hub` parses it.
    fn parse_name(text: &str) -> AgentName {
        text.parse().unwrap()
    }

    /// The user counts places among the desk's agents, but Herdr counts
    /// every tab, the board's and the shell's too, and puts a moved tab
    /// before the index it is given. Each move must land at the place
    /// asked for, or the agent goes above the board or one place off.
    #[test]
    fn run_desk_move_puts_agent_at_place() {
        let cases = [
            ("up", "desk-c", 1, ["desk-c", "desk-a", "desk-b"]),
            ("down", "desk-a", 3, ["desk-b", "desk-c", "desk-a"]),
        ];

        for (case_name, name, place, expected_order) in cases {
            let herdr = make_desk_herdr(
                vec![
                    make_agent_in("desk-a", "w4"),
                    make_agent_in("desk-b", "w4"),
                    make_agent_in("desk-c", "w4"),
                ],
                &[
                    "w4:t-board",
                    "w4:t-shell",
                    "w4:t-desk-a",
                    "w4:t-desk-b",
                    "w4:t-desk-c",
                ],
            );
            let test_tools = TestTools::new();
            let tools = Tools {
                herdr: &herdr,
                ..test_tools.tools()
            };

            run_desk_move(&tools, &parse_name(name), place).unwrap();

            let expected_tabs: Vec<String> = ["board", "shell"]
                .into_iter()
                .chain(expected_order)
                .map(|tab| format!("w4:t-{tab}"))
                .collect();
            assert_eq!(herdr.list_tabs("w4").unwrap(), expected_tabs, "{case_name}");
        }
    }
}
