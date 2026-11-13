mod common;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use hub::commands::launch::{LaunchRequest, run_launch};
use hub::external::cards::Stage;
use hub::external::herdr::{AgentStatus, HerdrCode, Workspace};
use hub::shared::HubError;

use common::{TestTools, make_agent, make_refusal, make_time};

/// The model a hub passes for its workers.
const MODEL: &str = "github-copilot/claude-sonnet-5#high";

/// A hub `kanban-hub` in workspace `w1`, labelled `kanban`, a workspace
/// `w2` labelled `DEMO-KB-007 approvals`, the ready card DEMO-KB-007,
/// the active card DEMO-KB-009 and a handoff file, whose path it
/// returns.
fn make_launch_tools() -> (TestTools, PathBuf) {
    let mut test_tools = TestTools::new();
    test_tools.herdr.agents = vec![make_agent("kanban-hub", AgentStatus::Working)];
    test_tools.own_pane = Some("w1:kanban-hub".to_owned());
    test_tools.herdr.workspaces = vec![
        Workspace {
            workspace_id: "w1".to_owned(),
            label: "kanban".to_owned(),
        },
        Workspace {
            workspace_id: "w2".to_owned(),
            label: "DEMO-KB-007 approvals".to_owned(),
        },
    ];
    test_tools.cards.stages = BTreeMap::from([
        ("DEMO-KB-007".to_owned(), Stage::Ready),
        ("DEMO-KB-009".to_owned(), Stage::Active),
    ]);
    let handoff = test_tools.path().join("impl.md");
    fs::write(&handoff, "# Handoff: build it\n").unwrap();
    (test_tools, handoff)
}

/// Runs `hub launch WORKER HANDOFF --model MODEL`, with
/// `--workspace LABEL` when one is given.
fn launch_worker(
    test_tools: &TestTools,
    worker: &str,
    handoff: &Path,
    workspace: Option<&str>,
) -> Result<Vec<String>, HubError> {
    let request = LaunchRequest {
        name: worker.parse().unwrap(),
        handoff: handoff.to_owned(),
        model: MODEL.to_owned(),
        workspace: workspace.map(str::to_owned),
        cwd: test_tools.path().to_owned(),
    };
    run_launch(
        &test_tools.tools(),
        &test_tools.run(),
        &request,
        &make_time("2026-11-11T22:14:00+00:00[UTC]"),
    )
    .map(|outcome| outcome.lines)
}

/// A card's worker opens in its card's workspace, else in a new one
/// named after the card, and work with no card in the hub's own, as the
/// herdr-hub skill's sidebar rule says. A worker in the wrong place is
/// lost in the user's sidebar.
#[test]
fn run_launch_opens_worker_by_sidebar_rule() {
    /// Where the worker opened.
    #[derive(Debug, PartialEq)]
    enum Opened {
        /// A new tab in the workspace with this ID.
        Tab(String),
        /// A new workspace with this label.
        Workspace(String),
    }
    struct Case {
        name: &'static str,
        worker: &'static str,
        workspace: Option<&'static str>,
        opened: Opened,
    }
    let cases = [
        Case {
            name: "card-workspace",
            worker: "demo-kb-007-impl",
            workspace: None,
            opened: Opened::Tab("w2".to_owned()),
        },
        Case {
            name: "new-card-workspace",
            worker: "demo-kb-009-plan-2",
            workspace: None,
            opened: Opened::Workspace("demo-kb-009 plan-2".to_owned()),
        },
        Case {
            name: "given-workspace",
            worker: "demo-kb-009-plan-2",
            workspace: Some("Kanban"),
            opened: Opened::Tab("w1".to_owned()),
        },
        Case {
            name: "given-new-workspace",
            worker: "demo-kb-009-plan-2",
            workspace: Some("docs"),
            opened: Opened::Workspace("docs".to_owned()),
        },
        Case {
            name: "no-card",
            worker: "docs-fix",
            workspace: None,
            opened: Opened::Tab("w1".to_owned()),
        },
    ];

    for case in cases {
        let (test_tools, handoff) = make_launch_tools();

        launch_worker(&test_tools, case.worker, &handoff, case.workspace).unwrap();

        let tabs = test_tools.herdr.created_tabs.borrow();
        let workspaces = test_tools.herdr.created_workspaces.borrow();
        let opened = match (tabs.as_slice(), workspaces.as_slice()) {
            ([(workspace_id, _)], []) => Opened::Tab(workspace_id.clone()),
            ([], [place]) => Opened::Workspace(place.label.clone()),
            _ => panic!("{}: expected one tab or one workspace", case.name),
        };
        assert_eq!(opened, case.opened, "{}", case.name);
    }
}

/// A ready card moves to active before its worker starts, so the board
/// counts the work. An active card stays, since `card` refuses active
/// to active and every second worker on a card would fail.
#[test]
fn run_launch_activates_only_ready_cards() {
    let cases = [
        ("ready", "demo-kb-007-impl", vec!["DEMO-KB-007"]),
        ("active", "demo-kb-009-impl", vec![]),
    ];

    for (case_name, worker, moves) in cases {
        let (test_tools, handoff) = make_launch_tools();

        launch_worker(&test_tools, worker, &handoff, None).unwrap();

        assert_eq!(*test_tools.cards.moves.borrow(), moves, "{case_name}");
    }
}

/// The started worker gets a prompt to read its handoff, the card gets
/// the handoff note and the worker's session, and `notes.md` gets a
/// line. Without them the worker starts without its brief, and the hub
/// cannot find the work again after it hands off.
#[test]
fn run_launch_prompts_worker_and_records_handoff() {
    let (test_tools, handoff) = make_launch_tools();

    launch_worker(&test_tools, "demo-kb-007-impl", &handoff, None).unwrap();

    let prompts = test_tools.herdr.prompts.borrow();
    assert!(prompts[0].1.contains(&handoff.display().to_string()));
    let note = format!("Handoff {}", handoff.display());
    assert_eq!(
        *test_tools.cards.notes.borrow(),
        [("DEMO-KB-007".to_owned(), note)]
    );
    assert_eq!(
        *test_tools.cards.links.borrow(),
        [("DEMO-KB-007".to_owned(), "demo-kb-007-impl".to_owned())]
    );
    let notes = fs::read_to_string(test_tools.run_path().join("notes.md")).unwrap();
    assert!(notes.contains("started demo-kb-007-impl ("));
}

/// Each refusal comes before any tab, workspace, agent, card change or
/// `notes.md` line, so the hub fixes the cause and runs the same command
/// again with nothing to clean up. A rerun for a worker that already
/// runs must not start a second one.
#[test]
fn run_launch_changes_nothing_when_check_fails() {
    struct Case {
        name: &'static str,
        worker: &'static str,
        /// The handoff file name, where only `impl.md` exists.
        handoff: &'static str,
        live: &'static [&'static str],
        move_error: Option<&'static str>,
        in_hub_pane: bool,
    }
    let cases = [
        Case {
            name: "missing-handoff",
            worker: "demo-kb-007-impl",
            handoff: "gone.md",
            live: &[],
            move_error: None,
            in_hub_pane: true,
        },
        Case {
            name: "live-worker",
            worker: "demo-kb-007-impl",
            handoff: "impl.md",
            live: &["demo-kb-007-impl"],
            move_error: None,
            in_hub_pane: true,
        },
        Case {
            name: "missing-card",
            worker: "demo-kb-404-impl",
            handoff: "impl.md",
            live: &[],
            move_error: None,
            in_hub_pane: true,
        },
        Case {
            name: "refused-move",
            worker: "demo-kb-007-impl",
            handoff: "impl.md",
            live: &[],
            move_error: Some("the limit is 8"),
            in_hub_pane: true,
        },
        Case {
            name: "no-card-outside-hub-pane",
            worker: "docs-fix",
            handoff: "impl.md",
            live: &[],
            move_error: None,
            in_hub_pane: false,
        },
    ];

    for case in cases {
        let (mut test_tools, _) = make_launch_tools();
        let live = case
            .live
            .iter()
            .map(|name| make_agent(name, AgentStatus::Idle));
        test_tools.herdr.agents.extend(live);
        test_tools.cards.move_error = case.move_error.map(str::to_owned);
        if !case.in_hub_pane {
            test_tools.own_pane = None;
        }
        let handoff = test_tools.path().join(case.handoff);

        launch_worker(&test_tools, case.worker, &handoff, None).unwrap_err();

        assert!(
            test_tools.herdr.created_tabs.borrow().is_empty(),
            "{}",
            case.name
        );
        assert!(
            test_tools.herdr.created_workspaces.borrow().is_empty(),
            "{}",
            case.name
        );
        assert!(
            test_tools.herdr.started.borrow().is_empty(),
            "{}",
            case.name
        );
        assert!(test_tools.cards.moves.borrow().is_empty(), "{}", case.name);
        assert!(test_tools.cards.notes.borrow().is_empty(), "{}", case.name);
        assert!(test_tools.cards.links.borrow().is_empty(), "{}", case.name);
        assert!(
            !test_tools.run_path().join("notes.md").exists(),
            "{}",
            case.name
        );
    }
}

/// A failure after the card moved or the worker started names each step
/// that finished, so the hub redoes only the rest by hand. A rerun
/// refuses, since the worker already runs, so this text is the hub's
/// only guide.
#[test]
fn run_launch_error_names_finished_steps() {
    struct Case {
        name: &'static str,
        herdr_failure: Option<&'static str>,
        note_error: Option<&'static str>,
        link_error: Option<&'static str>,
        done: &'static str,
    }
    let cases = [
        Case {
            name: "start-fails-after-move",
            herdr_failure: Some("start_agent"),
            note_error: None,
            link_error: None,
            done: "moved DEMO-KB-007 to active",
        },
        Case {
            name: "card-note-fails",
            herdr_failure: None,
            note_error: Some("card: vault is offline"),
            link_error: None,
            done: "started demo-kb-007-impl, sent the handoff prompt",
        },
        Case {
            name: "session-link-fails",
            herdr_failure: None,
            note_error: None,
            link_error: Some("card: no session yet"),
            done: "started demo-kb-007-impl, sent the handoff prompt, added the card note",
        },
    ];

    for case in cases {
        let (mut test_tools, handoff) = make_launch_tools();
        if let Some(method) = case.herdr_failure {
            let refusal = make_refusal(HerdrCode::Other(String::new()));
            test_tools.herdr.fail_next(method, refusal);
        }
        test_tools.cards.note_error = case.note_error.map(str::to_owned);
        test_tools.cards.link_error = case.link_error.map(str::to_owned);

        let error = launch_worker(&test_tools, "demo-kb-007-impl", &handoff, None).unwrap_err();

        let HubError::PartlyDone { done, .. } = &error else {
            panic!("{}: expected a partly done error, got {error}", case.name);
        };
        assert_eq!(done, case.done, "{}", case.name);
    }
}
