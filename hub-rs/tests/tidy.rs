mod common;

use std::cell::Cell;
use std::fs;
use std::path::Path;

use hub::commands::Outcome;
use hub::commands::clean::{run_clean, run_clean_listed, run_park};
use hub::external::cards::{CardNote, Stage};
use hub::external::herdr::{Agent, AgentStatus, HerdrError, Pane, Workspace};
use hub::shared::HubError;
use jiff::{SignedDuration, Timestamp};
use serde_json::{Value, json};

use common::{TestTools, make_agent, make_time, read_events};

/// Adds an idle worker whose report was read, on a card at `stage`,
/// working from a folder outside any repository, so git finds nothing
/// unsaved.
fn add_finished_worker(test_tools: &mut TestTools, name: &str, key: &str, stage: Stage) {
    let folder = test_tools.path().to_owned();
    test_tools.herdr.agents.push(Agent {
        session_id: Some(format!("ses_{name}")),
        folders: vec![folder],
        ..make_agent(name, AgentStatus::Idle)
    });
    test_tools.cards.stages.insert(key.to_owned(), stage);
    test_tools
        .cards
        .card_notes
        .insert(key.to_owned(), CardNote::default());
    let archive = format!("report-{name}.2214.read");
    fs::write(test_tools.run_path().join(archive), "PR opened.\n").unwrap();
}

/// Runs `run_clean`, as a dry run when `dry_run` is set, at `22:14`.
fn clean(test_tools: &TestTools, dry_run: bool) -> Outcome {
    let now = make_time("2026-11-11T22:14:00+00:00[UTC]");
    run_clean(&test_tools.tools(), &test_tools.run(), dry_run, &now).unwrap()
}

/// Runs `run_park` for NAME at `22:14`.
fn park(test_tools: &TestTools, name: &str) -> Result<Outcome, HubError> {
    let now = make_time("2026-11-11T22:14:00+00:00[UTC]");
    run_park(
        &test_tools.tools(),
        &test_tools.run(),
        &name.parse().unwrap(),
        &now,
    )
}

/// Returns NAME's parked record as JSON, or `None` when it has none.
fn read_parked_record(test_tools: &TestTools, name: &str) -> Option<Value> {
    let path = test_tools
        .state_folder()
        .join(format!("hub/parked/{name}.json"));
    let text = fs::read_to_string(path).ok()?;
    Some(serde_json::from_str(&text).unwrap())
}

/// Returns the run folder's `notes.md`, or empty text.
fn read_notes(run_path: &Path) -> String {
    fs::read_to_string(run_path.join("notes.md")).unwrap_or_default()
}

/// A finished session on a card whose work is over loses its tab,
/// and the line says which and why. A close with no line, or a line
/// with no close, tells the user the wrong thing. `notes.md` and the
/// event log are the only record of what the hub closed on its own.
#[test]
fn run_clean_closes_tab_and_says_why() {
    let mut test_tools = TestTools::new();
    add_finished_worker(
        &mut test_tools,
        "demo-kb-001-impl",
        "DEMO-KB-001",
        Stage::Done,
    );

    let outcome = clean(&test_tools, false);

    assert_eq!(outcome.lines, ["closed demo-kb-001-impl: DEMO-KB-001 done"]);
    assert_eq!(
        *test_tools.herdr.closed_tabs.borrow(),
        ["w1:t-demo-kb-001-impl"]
    );
    assert!(
        read_notes(&test_tools.run_path())
            .contains("- 2214 closed demo-kb-001-impl (DEMO-KB-001 done)\n")
    );
    let events = read_events(&test_tools.run_path());
    assert_eq!(events.last().unwrap()["event"], "closed");
    assert_eq!(events.last().unwrap()["tab"], "w1:t-demo-kb-001-impl");
}

/// The user reads a dry run of `run_clean` on real tabs before the
/// first real run, so it must close, park, link and write nothing while it
/// names what it would do.
#[test]
fn run_clean_dry_run_changes_nothing() {
    let mut test_tools = TestTools::new();
    add_finished_worker(
        &mut test_tools,
        "demo-kb-001-impl",
        "DEMO-KB-001",
        Stage::Done,
    );
    add_finished_worker(
        &mut test_tools,
        "demo-kb-007-impl",
        "DEMO-KB-007",
        Stage::InReview,
    );

    let outcome = clean(&test_tools, true);

    assert_eq!(
        outcome.lines,
        [
            "would close demo-kb-001-impl: DEMO-KB-001 done",
            "would park demo-kb-007-impl: DEMO-KB-007 in-review",
        ]
    );
    assert!(test_tools.herdr.closed_tabs.borrow().is_empty());
    assert!(test_tools.cards.links.borrow().is_empty());
    assert!(test_tools.cards.notes.borrow().is_empty());
    assert_eq!(read_parked_record(&test_tools, "demo-kb-007-impl"), None);
    assert!(read_events(&test_tools.run_path()).is_empty());
}

/// The board's y cleans only the sessions its dialog listed. A session
/// that became safe to close after the list was made must stay open,
/// or y would close a tab the user never saw named. The listed session
/// reported to another hub's run, since the dialog lists every hub's.
/// An unlisted session in the listed one's tab keeps that tab too.
#[test]
fn run_clean_listed_keeps_unlisted_sessions_open() {
    let cases = [
        (
            "own-tab",
            "w1:t-demo-kb-002-impl",
            vec!["closed demo-kb-001-impl: DEMO-KB-001 done"],
            vec!["w1:t-demo-kb-001-impl"],
        ),
        (
            "shared-tab",
            "w1:t-demo-kb-001-impl",
            vec![
                "kept demo-kb-001-impl: its tab holds other work",
                "nothing to close",
            ],
            Vec::new(),
        ),
    ];

    for (case_name, unlisted_tab, expected_lines, expected_closed) in cases {
        let mut test_tools = TestTools::new();
        add_finished_worker(
            &mut test_tools,
            "demo-kb-001-impl",
            "DEMO-KB-001",
            Stage::Done,
        );
        add_finished_worker(
            &mut test_tools,
            "demo-kb-002-impl",
            "DEMO-KB-002",
            Stage::Done,
        );
        test_tools.herdr.agents[1].tab_id = unlisted_tab.to_owned();
        let other_run = test_tools.path().join("other-run");
        fs::create_dir(&other_run).unwrap();
        let archive = "report-demo-kb-001-impl.2214.read";
        fs::rename(test_tools.run_path().join(archive), other_run.join(archive)).unwrap();
        let now = make_time("2026-11-11T22:14:00+00:00[UTC]");

        let outcome = run_clean_listed(
            &test_tools.tools(),
            &test_tools.run(),
            &["demo-kb-001-impl".to_owned()],
            &now,
        )
        .unwrap();

        assert_eq!(outcome.lines, expected_lines, "{case_name}");
        assert_eq!(
            *test_tools.herdr.closed_tabs.borrow(),
            expected_closed,
            "{case_name}"
        );
    }
}

/// Every hub on the machine sees every Herdr agent, so `run_clean`
/// must never touch a tab with none of its run's sessions. Closing
/// another hub's finished worker, such as an auto run's, ends a session
/// that hub still tracks, and the user loses its tab with no word from
/// its hub.
#[test]
fn run_clean_leaves_other_runs_workers_open() {
    let mut test_tools = TestTools::new();
    add_finished_worker(
        &mut test_tools,
        "demo-kb-001-impl",
        "DEMO-KB-001",
        Stage::Done,
    );
    add_finished_worker(
        &mut test_tools,
        "demo-kb-002-auto",
        "DEMO-KB-002",
        Stage::Done,
    );
    // The second worker reported to another hub's run folder.
    let other_run = test_tools.path().join("other-run");
    fs::create_dir(&other_run).unwrap();
    let archive = "report-demo-kb-002-auto.2214.read";
    fs::rename(test_tools.run_path().join(archive), other_run.join(archive)).unwrap();

    let outcome = clean(&test_tools, false);

    assert_eq!(outcome.lines, ["closed demo-kb-001-impl: DEMO-KB-001 done"]);
    assert_eq!(
        *test_tools.herdr.closed_tabs.borrow(),
        ["w1:t-demo-kb-001-impl"]
    );
}

/// The tidy pass gets every session on the machine, but `run_clean`
/// must leave another run's parked record to that run, even when its
/// card is done. It must also print no `kept` line for another hub, or
/// every clean lists every hub on the machine.
#[test]
fn run_clean_skips_sessions_outside_its_run() {
    let mut test_tools = TestTools::new();
    // The run's own done worker closes, so the lines show the pass ran.
    add_finished_worker(
        &mut test_tools,
        "demo-kb-001-impl",
        "DEMO-KB-001",
        Stage::Done,
    );
    test_tools
        .herdr
        .agents
        .push(make_agent("kanban-hub-2", AgentStatus::Idle));
    test_tools
        .cards
        .stages
        .insert("DEMO-KB-003".to_owned(), Stage::Done);
    let parked = test_tools.state_folder().join("hub/parked");
    fs::create_dir_all(&parked).unwrap();
    let record = json!({
        "name": "demo-kb-003-auto",
        "session_id": "ses_003",
        "card": "DEMO-KB-003",
        "workspace_label": "DEMO-KB-003 auto",
        "parked_at": "2026-11-10T22:41:00Z",
    });
    fs::write(parked.join("demo-kb-003-auto.json"), record.to_string()).unwrap();

    let outcome = clean(&test_tools, false);

    assert_eq!(outcome.lines, ["closed demo-kb-001-impl: DEMO-KB-001 done"]);
    assert!(read_parked_record(&test_tools, "demo-kb-003-auto").is_some());
}

/// A run also knows a worker by the `notes.md` line `run_launch`
/// wrote, before it ever reports, and an old hub by the `handoff`
/// event that replaced it. Missing either leaves finished tabs open
/// for the user to close by hand.
#[test]
fn run_clean_closes_sessions_this_run_knows() {
    struct Case {
        name: &'static str,
        session: &'static str,
        file: &'static str,
        text: &'static str,
        line: &'static str,
    }
    let cases = [
        Case {
            name: "launched-worker",
            session: "demo-kb-001-impl",
            file: "notes.md",
            text: "- 2200 started demo-kb-001-impl (w1:demo-kb-001-impl) from demo.md\n",
            line: "closed demo-kb-001-impl: DEMO-KB-001 done",
        },
        Case {
            name: "handed-off-hub",
            session: "kanban-hub-3",
            file: "events.jsonl",
            text: r#"{"at":"2026-11-11T22:00:00Z","event":"handoff","old":"kanban-hub-3","tab":"w1:t-kanban-hub-3","new":"kanban-hub-4"}"#,
            line: "closed kanban-hub-3: handed off",
        },
    ];

    for case in cases {
        let mut test_tools = TestTools::new();
        let folder = test_tools.path().to_owned();
        test_tools.herdr.agents.push(Agent {
            session_id: Some(format!("ses_{}", case.session)),
            folders: vec![folder],
            ..make_agent(case.session, AgentStatus::Idle)
        });
        test_tools
            .cards
            .stages
            .insert("DEMO-KB-001".to_owned(), Stage::Done);
        test_tools
            .cards
            .card_notes
            .insert("DEMO-KB-001".to_owned(), CardNote::default());
        fs::write(test_tools.run_path().join(case.file), case.text).unwrap();

        let outcome = clean(&test_tools, false);

        assert_eq!(outcome.lines, [case.line], "{}", case.name);
    }
}

/// An old hub's tab can hold an agent an older run left there, which no
/// current run knows. A done one must close with the old hub, or the tab
/// stays open for good. A working one must keep the tab, or its turn is
/// cut off. When the user's pane keeps the tab, the done one gets no
/// `kept` line, since this run does not know it.
#[test]
fn run_clean_judges_agents_sharing_hub_tab() {
    let cases = [
        (
            "done-worker",
            AgentStatus::Idle,
            false,
            vec!["closed kanban-hub-3, demo-kb-002-auto: handed off, DEMO-KB-002 done"],
            vec!["w1:t-kanban-hub-3"],
        ),
        (
            "working-worker",
            AgentStatus::Working,
            false,
            vec![
                "kept kanban-hub-3: its tab holds other work",
                "nothing to close",
            ],
            Vec::new(),
        ),
        (
            "focused-shell",
            AgentStatus::Idle,
            true,
            vec![
                "kept kanban-hub-3: its tab holds other work",
                "nothing to close",
            ],
            Vec::new(),
        ),
    ];

    for (case_name, worker_status, focused_shell, expected_lines, expected_closed) in cases {
        let mut test_tools = TestTools::new();
        let folder = test_tools.path().to_owned();
        test_tools.herdr.agents.push(Agent {
            session_id: Some("ses_kanban-hub-3".to_owned()),
            folders: vec![folder.clone()],
            ..make_agent("kanban-hub-3", AgentStatus::Idle)
        });
        // The worker shares the old hub's tab and never reported to this run.
        test_tools.herdr.agents.push(Agent {
            session_id: Some("ses_demo-kb-002-auto".to_owned()),
            tab_id: "w1:t-kanban-hub-3".to_owned(),
            folders: vec![folder],
            ..make_agent("demo-kb-002-auto", worker_status)
        });
        test_tools
            .cards
            .stages
            .insert("DEMO-KB-002".to_owned(), Stage::Done);
        test_tools
            .cards
            .card_notes
            .insert("DEMO-KB-002".to_owned(), CardNote::default());
        if focused_shell {
            test_tools.herdr.on_list_panes = Some(Box::new(|| {
                vec![Pane {
                    pane_id: "w1:p-user".to_owned(),
                    tab_id: "w1:t-kanban-hub-3".to_owned(),
                    focused: true,
                    has_agent: false,
                }]
            }));
        }
        let handoff = r#"{"at":"2026-11-11T22:00:00Z","event":"handoff","old":"kanban-hub-3","tab":"w1:t-kanban-hub-3","new":"kanban-hub-4"}"#;
        fs::write(test_tools.run_path().join("events.jsonl"), handoff).unwrap();

        let outcome = clean(&test_tools, false);

        assert_eq!(outcome.lines, expected_lines, "{case_name}");
        assert_eq!(
            *test_tools.herdr.closed_tabs.borrow(),
            expected_closed,
            "{case_name}"
        );
    }
}

/// Sets up auto run 21 after `kanban-hub-4`, this hub, adopted it.
/// This hub's pane sits in `own_workspace`. Workspace `w2`, "auto run
/// 21", holds the idle old hub `auto-hub-21`. Workspace `w3`,
/// "demo-kb-002 auto", holds the worker `demo-kb-002-auto` on done card
/// DEMO-KB-002 and tab `w3:t-shell` with plain shell pane `w3:p-shell`.
/// The auto run's folder says it started the worker, and the adopt's
/// `handoff` event names that folder.
fn add_adopted_auto_run(
    test_tools: &mut TestTools,
    worker_status: AgentStatus,
    own_workspace: &str,
) {
    let folder = test_tools.path().to_owned();
    let own_pane = format!("{own_workspace}:kanban-hub-4");
    let own_tab = format!("{own_workspace}:t-kanban-hub-4");
    test_tools.own_pane = Some(own_pane.clone());
    test_tools.herdr.agents.push(Agent {
        pane_id: own_pane,
        tab_id: own_tab.clone(),
        workspace_id: own_workspace.to_owned(),
        ..make_agent("kanban-hub-4", AgentStatus::Working)
    });
    for (name, status, workspace) in [
        ("auto-hub-21", AgentStatus::Idle, "w2"),
        ("demo-kb-002-auto", worker_status, "w3"),
    ] {
        test_tools.herdr.agents.push(Agent {
            session_id: Some(format!("ses_{name}")),
            pane_id: format!("{workspace}:{name}"),
            tab_id: format!("{workspace}:t-{name}"),
            workspace_id: workspace.to_owned(),
            folders: vec![folder.clone()],
            ..make_agent(name, status)
        });
    }
    test_tools.herdr.on_list_panes = Some(Box::new(|| {
        vec![Pane {
            pane_id: "w3:p-shell".to_owned(),
            tab_id: "w3:t-shell".to_owned(),
            focused: false,
            has_agent: false,
        }]
    }));
    test_tools.herdr.workspaces = [
        ("w1", "kanban"),
        ("w2", "auto run 21"),
        ("w3", "demo-kb-002 auto"),
    ]
    .map(|(workspace_id, label)| Workspace {
        workspace_id: workspace_id.to_owned(),
        label: label.to_owned(),
    })
    .to_vec();
    let tabs = &mut test_tools.herdr.tabs;
    tabs.insert("w2".to_owned(), vec!["w2:t-auto-hub-21".to_owned()]);
    let worker_tabs = ["w3:t-demo-kb-002-auto", "w3:t-shell"];
    tabs.insert("w3".to_owned(), worker_tabs.map(str::to_owned).to_vec());
    tabs.entry(own_workspace.to_owned())
        .or_default()
        .push(own_tab);
    test_tools
        .cards
        .stages
        .insert("DEMO-KB-002".to_owned(), Stage::Done);
    test_tools
        .cards
        .card_notes
        .insert("DEMO-KB-002".to_owned(), CardNote::default());
    let auto_run = folder.join("herdr-hub.auto21");
    fs::create_dir(&auto_run).unwrap();
    let started = "- 1047 started demo-kb-002-auto (w3:demo-kb-002-auto) from /h.md\n";
    fs::write(auto_run.join("notes.md"), started).unwrap();
    let adopt = json!({
        "at": "2026-11-11T22:00:00Z",
        "event": "handoff",
        "old": "auto-hub-21",
        "tab": "w2:t-auto-hub-21",
        "new": "kanban-hub-4",
        "run": auto_run,
    });
    fs::write(
        test_tools.run_path().join("events.jsonl"),
        adopt.to_string(),
    )
    .unwrap();
}

/// Once an adopted auto hub is done, the hub's tab, a done worker's
/// tab and a tab of a shell at its prompt must close, and so must their
/// workspaces. Otherwise every finished run stays in the user's
/// sidebar.
#[test]
fn run_clean_retires_finished_auto_run_workspaces() {
    let mut test_tools = TestTools::new();
    add_adopted_auto_run(&mut test_tools, AgentStatus::Idle, "w1");
    test_tools.herdr.quiet_shells = vec!["w3:p-shell".to_owned()];

    let outcome = clean(&test_tools, false);

    assert_eq!(
        outcome.lines,
        [
            "closed auto-hub-21: handed off",
            "closed demo-kb-002-auto: DEMO-KB-002 done",
            "closed pane w3:p-shell: run over",
            "closed workspace auto run 21: run over",
            "closed workspace demo-kb-002 auto: run over",
        ]
    );
    assert_eq!(
        *test_tools.herdr.closed_tabs.borrow(),
        ["w2:t-auto-hub-21", "w3:t-demo-kb-002-auto", "w3:t-shell"]
    );
}

/// Retiring starts only once the adopted auto hub is `to-close`. While
/// it still works, the user may still use its run, so no tab in the
/// run's workspaces may close.
#[test]
fn run_clean_keeps_workspaces_while_hub_works() {
    let mut test_tools = TestTools::new();
    add_adopted_auto_run(&mut test_tools, AgentStatus::Idle, "w1");
    test_tools.herdr.quiet_shells = vec!["w3:p-shell".to_owned()];
    let agents = &mut test_tools.herdr.agents;
    let old_hub = agents.iter_mut().find(|agent| agent.name == "auto-hub-21");
    old_hub.unwrap().status = AgentStatus::Working;

    clean(&test_tools, false);

    assert!(test_tools.herdr.closed_tabs.borrow().is_empty());
}

/// A finished run's worker workspace can still hold work, such as a
/// worker mid-turn or a shell running a program. Their tabs must stay, and
/// each must get a `kept` line, or the user cannot tell why the
/// workspace stays open.
#[test]
fn run_clean_names_work_keeping_retired_workspace() {
    let mut test_tools = TestTools::new();
    add_adopted_auto_run(&mut test_tools, AgentStatus::Working, "w1");
    test_tools.herdr.program_in_front = vec!["w3:p-shell".to_owned()];

    let outcome = clean(&test_tools, false);

    assert_eq!(
        outcome.lines,
        [
            "closed auto-hub-21: handed off",
            "kept pane w3:p-shell: shell in use",
            "kept demo-kb-002-auto: working",
            "closed workspace auto run 21: run over",
        ]
    );
    assert_eq!(*test_tools.herdr.closed_tabs.borrow(), ["w2:t-auto-hub-21"]);
}

/// A finished worker of the run can stay for a reason, such as work git
/// cannot vouch for. Its `kept` line must name it even though this hub
/// did not start it, or the user cannot tell why the workspace stays.
#[test]
fn run_clean_names_kept_worker_holding_workspace() {
    let mut test_tools = TestTools::new();
    add_adopted_auto_run(&mut test_tools, AgentStatus::Idle, "w1");
    test_tools.herdr.quiet_shells = vec!["w3:p-shell".to_owned()];
    let agents = &mut test_tools.herdr.agents;
    let worker = agents
        .iter_mut()
        .find(|agent| agent.name == "demo-kb-002-auto");
    worker.unwrap().folders.clear();

    let outcome = clean(&test_tools, false);

    assert_eq!(
        outcome.lines,
        [
            "closed auto-hub-21: handed off",
            "closed pane w3:p-shell: run over",
            "kept demo-kb-002-auto: git failed",
            "closed workspace auto run 21: run over",
        ]
    );
}

/// A workspace that holds this hub's pane is the user's, not the run's,
/// even when a worker of the run sits there too. Retiring it would
/// close the user's shells, so only the old hub's workspace closes.
#[test]
fn run_clean_keeps_workspace_holding_own_hub() {
    let mut test_tools = TestTools::new();
    add_adopted_auto_run(&mut test_tools, AgentStatus::Idle, "w3");
    test_tools.herdr.quiet_shells = vec!["w3:p-shell".to_owned()];

    let outcome = clean(&test_tools, false);

    assert_eq!(
        outcome.lines,
        [
            "closed auto-hub-21: handed off",
            "closed workspace auto run 21: run over",
        ]
    );
    assert_eq!(*test_tools.herdr.closed_tabs.borrow(), ["w2:t-auto-hub-21"]);
}

/// The desk holds the board and the user's own shell, even after
/// `hub desk add` moved a worker of the run there. Retiring it would
/// close the user's shell tab, and then Herdr would drop the desk.
#[test]
fn run_clean_never_retires_desk_workspace() {
    let mut test_tools = TestTools::new();
    add_adopted_auto_run(&mut test_tools, AgentStatus::Idle, "w1");
    test_tools.herdr.quiet_shells = vec!["w3:p-shell".to_owned()];
    // `w3`, which holds the worker and a shell, is the user's desk.
    test_tools.herdr.workspaces[2].label = "desk".to_owned();

    clean(&test_tools, false);

    assert_eq!(*test_tools.herdr.closed_tabs.borrow(), ["w2:t-auto-hub-21"]);
}

/// When `HERDR_PANE_ID` names the wrong pane, a hub the user still
/// works in can sit in a run's worker workspace. Retiring that
/// workspace would close the user's tabs there, so a workspace that
/// holds a hub that stays open must keep its tabs.
#[test]
fn run_clean_keeps_workspace_holding_open_hub() {
    let mut test_tools = TestTools::new();
    add_adopted_auto_run(&mut test_tools, AgentStatus::Idle, "w1");
    test_tools.herdr.quiet_shells = vec!["w3:p-shell".to_owned()];
    test_tools.herdr.agents.push(Agent {
        pane_id: "w3:kanban-hub-3".to_owned(),
        tab_id: "w3:t-kanban-hub-3".to_owned(),
        workspace_id: "w3".to_owned(),
        ..make_agent("kanban-hub-3", AgentStatus::Working)
    });

    clean(&test_tools, false);

    assert_eq!(*test_tools.herdr.closed_tabs.borrow(), ["w2:t-auto-hub-21"]);
}

/// Without `HERDR_PANE_ID`, clean cannot tell which workspace holds
/// the hub the user works in, so it must retire nothing rather than
/// risk the user's own tabs, and say why. The old hub's tab still
/// closes.
#[test]
fn run_clean_retires_nothing_without_own_pane() {
    let mut test_tools = TestTools::new();
    add_adopted_auto_run(&mut test_tools, AgentStatus::Idle, "w1");
    test_tools.herdr.quiet_shells = vec!["w3:p-shell".to_owned()];
    test_tools.own_pane = None;

    let outcome = clean(&test_tools, false);

    assert_eq!(
        outcome.lines,
        [
            "closed auto-hub-21: handed off",
            "kept workspaces of auto-hub-21: this hub's pane is unknown",
        ]
    );
    assert_eq!(*test_tools.herdr.closed_tabs.borrow(), ["w2:t-auto-hub-21"]);
}

/// A failed Herdr list while clean looks for a finished run's
/// workspaces must not stop the clean. The old hub's tab still closes,
/// and a `kept` line says why no workspace closed.
#[test]
fn run_clean_closes_hub_despite_tab_failure() {
    let mut test_tools = TestTools::new();
    add_adopted_auto_run(&mut test_tools, AgentStatus::Idle, "w1");
    test_tools.herdr.quiet_shells = vec!["w3:p-shell".to_owned()];
    let not_running = HerdrError::Failed("herdr is not running".to_owned());
    test_tools.herdr.fail_next("list_tabs", not_running);

    let outcome = clean(&test_tools, false);

    assert_eq!(
        outcome.lines,
        [
            "closed auto-hub-21: handed off",
            "kept workspaces of auto-hub-21: herdr is not running",
        ]
    );
    assert_eq!(*test_tools.herdr.closed_tabs.borrow(), ["w2:t-auto-hub-21"]);
}

/// Only the workspaces of sessions the auto run started may retire. The
/// user's quiet shell beside a worker the run did not start must stay,
/// or every clean would close the shells in each live session's
/// workspace.
#[test]
fn run_clean_keeps_unstarted_workers_workspace() {
    let mut test_tools = TestTools::new();
    add_adopted_auto_run(&mut test_tools, AgentStatus::Idle, "w1");
    test_tools.herdr.agents.push(Agent {
        pane_id: "w4:demo-kb-009-impl".to_owned(),
        tab_id: "w4:t-demo-kb-009-impl".to_owned(),
        workspace_id: "w4".to_owned(),
        ..make_agent("demo-kb-009-impl", AgentStatus::Idle)
    });
    // Workspace `w4` holds that worker and the user's quiet shell.
    test_tools.herdr.workspaces.push(Workspace {
        workspace_id: "w4".to_owned(),
        label: "demo-kb-009 impl".to_owned(),
    });
    let w4_tabs = ["w4:t-demo-kb-009-impl", "w4:t-shell"];
    test_tools
        .herdr
        .tabs
        .insert("w4".to_owned(), w4_tabs.map(str::to_owned).to_vec());
    test_tools.herdr.on_list_panes = Some(Box::new(|| {
        vec![
            Pane {
                pane_id: "w3:p-shell".to_owned(),
                tab_id: "w3:t-shell".to_owned(),
                focused: false,
                has_agent: false,
            },
            Pane {
                pane_id: "w4:p-shell".to_owned(),
                tab_id: "w4:t-shell".to_owned(),
                focused: false,
                has_agent: false,
            },
        ]
    }));
    test_tools.herdr.quiet_shells = vec!["w3:p-shell".to_owned(), "w4:p-shell".to_owned()];
    test_tools
        .cards
        .stages
        .insert("DEMO-KB-009".to_owned(), Stage::Done);
    test_tools
        .cards
        .card_notes
        .insert("DEMO-KB-009".to_owned(), CardNote::default());

    clean(&test_tools, false);

    assert_eq!(
        *test_tools.herdr.closed_tabs.borrow(),
        ["w2:t-auto-hub-21", "w3:t-demo-kb-002-auto", "w3:t-shell"]
    );
}

/// The hub runs `run_send` as a process of its own, beside `run_clean`.
/// So a message can come in between the verdict and the close or park.
/// Closing then would leave the message for a closed tab. The state is
/// worked out again first.
#[test]
fn run_clean_skips_session_messaged_meanwhile() {
    let mut test_tools = TestTools::new();
    add_finished_worker(
        &mut test_tools,
        "demo-kb-001-impl",
        "DEMO-KB-001",
        Stage::Done,
    );
    add_finished_worker(
        &mut test_tools,
        "demo-kb-007-impl",
        "DEMO-KB-007",
        Stage::InReview,
    );
    let sends = test_tools.run_path().join("sends");
    test_tools.herdr.on_list_panes = Some(Box::new(move || {
        fs::create_dir_all(&sends).unwrap();
        for name in ["demo-kb-001-impl", "demo-kb-007-impl"] {
            fs::write(sends.join(format!("{name}.20261111-221400.1.held")), "{}").unwrap();
        }
        Vec::new()
    }));

    let outcome = clean(&test_tools, false);

    assert_eq!(
        outcome.lines,
        [
            "kept demo-kb-001-impl: hub",
            "kept demo-kb-007-impl: hub",
            "nothing to close",
        ]
    );
    assert!(test_tools.herdr.closed_tabs.borrow().is_empty());
    assert_eq!(read_parked_record(&test_tools, "demo-kb-007-impl"), None);
}

/// The user can focus a finished agent's pane while the pass parks or
/// closes others, which takes seconds of card calls. The tab keeps the
/// same panes, so only the focus shows the user is there. The tab must
/// stay, or the session the user turned to closes in front of them.
#[test]
fn run_clean_keeps_tab_focused_meanwhile() {
    let mut test_tools = TestTools::new();
    add_finished_worker(
        &mut test_tools,
        "demo-kb-001-impl",
        "DEMO-KB-001",
        Stage::Done,
    );
    add_finished_worker(
        &mut test_tools,
        "demo-kb-007-impl",
        "DEMO-KB-007",
        Stage::InReview,
    );
    // From the second pane read on, each finished agent's own pane is
    // focused, and no pane comes or goes.
    let reads = Cell::new(0);
    test_tools.herdr.on_list_panes = Some(Box::new(move || {
        reads.set(reads.get() + 1);
        if reads.get() == 1 {
            return Vec::new();
        }
        ["demo-kb-001-impl", "demo-kb-007-impl"]
            .map(|name| Pane {
                pane_id: format!("w1:{name}"),
                tab_id: format!("w1:t-{name}"),
                focused: true,
                has_agent: true,
            })
            .to_vec()
    }));

    let outcome = clean(&test_tools, false);

    assert_eq!(
        outcome.lines,
        [
            "kept demo-kb-001-impl: its tab holds other work",
            "kept demo-kb-007-impl: its tab holds other work",
            "nothing to close",
        ]
    );
    assert!(test_tools.herdr.closed_tabs.borrow().is_empty());
}

/// A quiet shell in a finished tab can start an agent while the pass
/// parks or closes others. The tab must stay, or the user's new session
/// closes with the finished one.
#[test]
fn run_clean_keeps_tab_of_new_agent() {
    let mut test_tools = TestTools::new();
    add_finished_worker(
        &mut test_tools,
        "demo-kb-001-impl",
        "DEMO-KB-001",
        Stage::Done,
    );
    test_tools.herdr.quiet_shells = vec!["w1:p-user".to_owned()];
    // The user's pane is a quiet shell at the first pane read and runs
    // an agent from the second read on.
    let reads = Cell::new(0);
    test_tools.herdr.on_list_panes = Some(Box::new(move || {
        reads.set(reads.get() + 1);
        vec![Pane {
            pane_id: "w1:p-user".to_owned(),
            tab_id: "w1:t-demo-kb-001-impl".to_owned(),
            focused: false,
            has_agent: reads.get() > 1,
        }]
    }));

    let outcome = clean(&test_tools, false);

    assert_eq!(
        outcome.lines,
        [
            "kept demo-kb-001-impl: its tab holds other work",
            "nothing to close",
        ]
    );
    assert!(test_tools.herdr.closed_tabs.borrow().is_empty());
}

/// A finished worker's tab can hold a shell the user opened. The tab
/// may close with the worker only while that shell sits at its prompt,
/// or the user loses the editor or program running in it.
#[test]
fn run_clean_closes_only_quiet_shell_tabs() {
    let make_user_pane = || vec!["w1:p-user".to_owned()];
    let cases = [
        (
            "quiet-shell",
            make_user_pane(),
            Vec::new(),
            vec!["closed demo-kb-001-impl: DEMO-KB-001 done"],
            vec!["w1:t-demo-kb-001-impl"],
        ),
        (
            "editor-in-front",
            Vec::new(),
            make_user_pane(),
            vec![
                "kept demo-kb-001-impl: its tab holds other work",
                "nothing to close",
            ],
            Vec::new(),
        ),
    ];

    for (case_name, quiet_shells, program_in_front, expected_lines, expected_closed) in cases {
        let mut test_tools = TestTools::new();
        add_finished_worker(
            &mut test_tools,
            "demo-kb-001-impl",
            "DEMO-KB-001",
            Stage::Done,
        );
        test_tools.herdr.quiet_shells = quiet_shells;
        test_tools.herdr.program_in_front = program_in_front;
        test_tools.herdr.on_list_panes = Some(Box::new(|| {
            vec![Pane {
                pane_id: "w1:p-user".to_owned(),
                tab_id: "w1:t-demo-kb-001-impl".to_owned(),
                focused: false,
                has_agent: false,
            }]
        }));

        let outcome = clean(&test_tools, false);

        assert_eq!(outcome.lines, expected_lines, "{case_name}");
        assert_eq!(
            *test_tools.herdr.closed_tabs.borrow(),
            expected_closed,
            "{case_name}"
        );
    }
}

/// `run_status` and `run_next` call a session `to-close` whose tab a
/// busy shell keeps, such as an editor beside an old hub. Without a line
/// saying why, `run_clean` prints only `nothing to close`, and the user
/// cannot tell why the tab stays open.
#[test]
fn run_clean_names_session_kept_by_shell() {
    let mut test_tools = TestTools::new();
    add_finished_worker(
        &mut test_tools,
        "demo-kb-001-impl",
        "DEMO-KB-001",
        Stage::Done,
    );
    // The editor pane is left out of `quiet_shells`, so it runs something.
    test_tools.herdr.on_list_panes = Some(Box::new(|| {
        vec![Pane {
            pane_id: "w1:p-editor".to_owned(),
            tab_id: "w1:t-demo-kb-001-impl".to_owned(),
            focused: false,
            has_agent: false,
        }]
    }));

    for dry_run in [true, false] {
        let outcome = clean(&test_tools, dry_run);

        assert_eq!(
            outcome.lines,
            [
                "kept demo-kb-001-impl: its tab holds other work",
                "nothing to close",
            ],
            "dry_run {dry_run}"
        );
    }
    assert!(test_tools.herdr.closed_tabs.borrow().is_empty());
}

/// A park that fails after saving its record leaves that record beside
/// the live session. The next pass must delete the old record before it
/// parks again, or it deletes the new one and the parked session can
/// never reopen.
#[test]
fn run_clean_parks_session_beside_stale_record() {
    let mut test_tools = TestTools::new();
    add_finished_worker(
        &mut test_tools,
        "demo-kb-007-impl",
        "DEMO-KB-007",
        Stage::InReview,
    );
    let parked = test_tools.state_folder().join("hub/parked");
    fs::create_dir_all(&parked).unwrap();
    let record = json!({
        "name": "demo-kb-007-impl",
        "session_id": "ses_old",
        "card": "DEMO-KB-007",
        "workspace_label": "DEMO-KB-007 impl",
        "parked_at": "2026-11-10T22:41:00Z",
    });
    fs::write(parked.join("demo-kb-007-impl.json"), record.to_string()).unwrap();

    clean(&test_tools, false);

    assert_eq!(
        *test_tools.herdr.closed_tabs.borrow(),
        ["w1:t-demo-kb-007-impl"]
    );
    let record = read_parked_record(&test_tools, "demo-kb-007-impl").unwrap();
    assert_eq!(record["session_id"], "ses_demo-kb-007-impl");
}

/// A record left by a failed close belongs to a session that is live
/// again. Left in place, it reopens a second agent on the same session
/// once that tab closes by hand.
#[test]
fn run_clean_forgets_live_session_record() {
    let mut test_tools = TestTools::new();
    test_tools.herdr.agents = vec![make_agent("demo-kb-007-impl", AgentStatus::Working)];
    let parked = test_tools.state_folder().join("hub/parked");
    fs::create_dir_all(&parked).unwrap();
    let record = json!({
        "name": "demo-kb-007-impl",
        "session_id": "ses_007",
        "card": "DEMO-KB-007",
        "workspace_label": "DEMO-KB-007 impl",
        "parked_at": "2026-11-10T22:41:00Z",
    });
    fs::write(parked.join("demo-kb-007-impl.json"), record.to_string()).unwrap();

    let outcome = clean(&test_tools, false);

    assert_eq!(outcome.lines, ["forgot demo-kb-007-impl"]);
    assert_eq!(read_parked_record(&test_tools, "demo-kb-007-impl"), None);
}

/// A parked session's record is its only way back to its OpenCode
/// session. `run_clean` must keep it while the card needs the user's
/// review, or the next message to that session has nothing to reopen.
#[test]
fn run_clean_keeps_record_of_parked_session() {
    let mut test_tools = TestTools::new();
    test_tools
        .cards
        .stages
        .insert("DEMO-KB-007".to_owned(), Stage::InReview);
    test_tools
        .cards
        .card_notes
        .insert("DEMO-KB-007".to_owned(), CardNote::default());
    let parked = test_tools.state_folder().join("hub/parked");
    fs::create_dir_all(&parked).unwrap();
    let record = json!({
        "name": "demo-kb-007-impl",
        "session_id": "ses_007",
        "card": "DEMO-KB-007",
        "workspace_label": "DEMO-KB-007 impl",
        "parked_at": "2026-11-10T22:41:00Z",
    });
    fs::write(parked.join("demo-kb-007-impl.json"), record.to_string()).unwrap();
    let archive = test_tools
        .run_path()
        .join("report-demo-kb-007-impl.2214.read");
    fs::write(archive, "PR opened.\n").unwrap();

    let outcome = clean(&test_tools, false);

    assert_eq!(outcome.lines, ["nothing to close"]);
    assert!(read_parked_record(&test_tools, "demo-kb-007-impl").is_some());
}

/// A request file that does not parse may be the approval that keeps
/// a finished session open. Closing anything while it shows could close
/// the session that waits for the user's approval.
#[test]
fn run_clean_closes_nothing_while_requests_unreadable() {
    let mut test_tools = TestTools::new();
    add_finished_worker(
        &mut test_tools,
        "demo-kb-001-impl",
        "DEMO-KB-001",
        Stage::Done,
    );
    let requests = test_tools.state_folder().join("pr-write");
    fs::create_dir_all(&requests).unwrap();
    fs::write(requests.join("a.json"), "{cut off").unwrap();

    let outcome = clean(&test_tools, false);

    assert_eq!(
        outcome.lines,
        [
            "kept every session: requests: 1 files did not parse",
            "nothing to close",
        ]
    );
    assert!(test_tools.herdr.closed_tabs.borrow().is_empty());
}

/// A worker whose card's work is over may still be merging another
/// pull request. A request it filed in the last hour must keep its
/// tab through `run_clean`, or the user approves a request whose
/// session is gone.
#[test]
fn run_clean_keeps_worker_with_recent_request() {
    let mut test_tools = TestTools::new();
    add_finished_worker(
        &mut test_tools,
        "demo-kb-001-impl",
        "DEMO-KB-001",
        Stage::Done,
    );
    let requests = test_tools.state_folder().join("pr-write");
    fs::create_dir_all(&requests).unwrap();
    let request = json!({
        "state": "ran",
        "pane_id": "w1:demo-kb-001-impl",
        "created": (Timestamp::now() - SignedDuration::from_mins(10)).to_string(),
    });
    fs::write(requests.join("a.json"), request.to_string()).unwrap();

    let outcome = clean(&test_tools, false);

    assert_eq!(
        outcome.lines,
        [
            "kept demo-kb-001-impl: request in the last hour",
            "nothing to close",
        ]
    );
    assert!(test_tools.herdr.closed_tabs.borrow().is_empty());
}

/// A parked session can reopen only from its record, and a person
/// finds it again from the card. The record must name the session,
/// card and workspace, the card must list the session, and only then
/// may the tab close.
#[test]
fn run_park_records_session_then_closes_tab() {
    let mut test_tools = TestTools::new();
    add_finished_worker(
        &mut test_tools,
        "demo-kb-007-impl",
        "DEMO-KB-007",
        Stage::InReview,
    );

    let outcome = park(&test_tools, "demo-kb-007-impl").unwrap();

    assert_eq!(outcome.lines, ["parked demo-kb-007-impl on DEMO-KB-007"]);
    let record = read_parked_record(&test_tools, "demo-kb-007-impl").unwrap();
    assert_eq!(record["session_id"], "ses_demo-kb-007-impl");
    assert_eq!(record["card"], "DEMO-KB-007");
    assert_eq!(
        *test_tools.cards.links.borrow(),
        [("DEMO-KB-007".to_owned(), "demo-kb-007-impl".to_owned())]
    );
    assert_eq!(
        *test_tools.cards.notes.borrow(),
        [(
            "DEMO-KB-007".to_owned(),
            "parked demo-kb-007-impl".to_owned()
        )]
    );
    assert_eq!(
        *test_tools.herdr.closed_tabs.borrow(),
        ["w1:t-demo-kb-007-impl"]
    );
    assert!(
        read_notes(&test_tools.run_path())
            .contains("- 2214 parked demo-kb-007-impl (DEMO-KB-007 in-review)\n")
    );
    assert_eq!(
        read_events(&test_tools.run_path()).last().unwrap()["event"],
        "parked"
    );
}

/// `run_park` on a worker still in its turn must refuse and change
/// nothing. A park there closes the tab and cuts the turn off.
#[test]
fn run_park_refuses_session_still_working() {
    let mut test_tools = TestTools::new();
    add_finished_worker(
        &mut test_tools,
        "demo-kb-007-impl",
        "DEMO-KB-007",
        Stage::InReview,
    );
    test_tools.herdr.agents[0].status = AgentStatus::Working;

    let error = park(&test_tools, "demo-kb-007-impl").unwrap_err();

    assert_eq!(error.to_string(), "demo-kb-007-impl stays: working");
    assert_eq!(read_parked_record(&test_tools, "demo-kb-007-impl"), None);
    assert!(test_tools.herdr.closed_tabs.borrow().is_empty());
}

/// A `Cards::link_session` failure after the record is written must
/// leave both the record and the open tab. Closing the tab first, or
/// deleting the record, loses the session ID of work that needs the
/// user's review.
#[test]
fn run_park_failure_keeps_record_and_tab() {
    let mut test_tools = TestTools::new();
    add_finished_worker(
        &mut test_tools,
        "demo-kb-007-impl",
        "DEMO-KB-007",
        Stage::InReview,
    );
    test_tools.cards.link_error = Some("card: vault is read-only".to_owned());

    let error = park(&test_tools, "demo-kb-007-impl").unwrap_err();

    assert!(matches!(error, HubError::Card(_)), "{error:?}");
    assert!(read_parked_record(&test_tools, "demo-kb-007-impl").is_some());
    assert!(test_tools.herdr.closed_tabs.borrow().is_empty());
}

/// `run_park` closes the session's whole tab. Another agent or a busy
/// shell in that tab must keep it open, as in the tidy pass, or the
/// hub parking one worker kills the other's turn.
#[test]
fn run_park_keeps_tab_with_other_pane() {
    let mut test_tools = TestTools::new();
    add_finished_worker(
        &mut test_tools,
        "demo-kb-007-impl",
        "DEMO-KB-007",
        Stage::InReview,
    );
    test_tools.herdr.agents.push(Agent {
        tab_id: "w1:t-demo-kb-007-impl".to_owned(),
        ..make_agent("docs-fix", AgentStatus::Working)
    });

    let error = park(&test_tools, "demo-kb-007-impl").unwrap_err();

    assert_eq!(
        error.to_string(),
        "demo-kb-007-impl stays: its tab holds other work"
    );
    assert_eq!(read_parked_record(&test_tools, "demo-kb-007-impl"), None);
    assert!(test_tools.herdr.closed_tabs.borrow().is_empty());
}

/// A message can come in while `Cards::link_session` runs on the
/// network drive. The park must check again after the link and keep
/// the tab, or the message waits for a closed session. The record
/// stays for the next pass, which deletes it while the session is
/// live. The card's Log must not say the session parked.
#[test]
fn run_park_keeps_tab_messaged_during_link() {
    let mut test_tools = TestTools::new();
    add_finished_worker(
        &mut test_tools,
        "demo-kb-007-impl",
        "DEMO-KB-007",
        Stage::InReview,
    );
    let sends = test_tools.run_path().join("sends");
    test_tools.cards.on_link = Some(Box::new(move || {
        fs::create_dir_all(&sends).unwrap();
        let held = sends.join("demo-kb-007-impl.20261111-221400.1.held");
        fs::write(held, "{}").unwrap();
    }));

    let error = park(&test_tools, "demo-kb-007-impl").unwrap_err();

    assert_eq!(error.to_string(), "demo-kb-007-impl stays: hub");
    assert!(test_tools.herdr.closed_tabs.borrow().is_empty());
    assert!(read_parked_record(&test_tools, "demo-kb-007-impl").is_some());
    assert!(test_tools.cards.notes.borrow().is_empty());
}
