mod common;

use std::fs;

use hub::commands::status::run_status;
use hub::outside::cards::{CardNote, Stage};
use hub::outside::herdr::{Agent, AgentStatus};
use serde_json::{Value, json};

use common::{TestTools, make_agent, make_card_repo, make_time};

/// Runs `hub status --json` and returns what it printed.
fn read_status_json(test_tools: &TestTools) -> Value {
    let now = make_time("2026-11-11T22:14:00+00:00[UTC]");
    let outcome = run_status(&test_tools.tools(), &test_tools.run(), true, &now).unwrap();
    serde_json::from_str(&outcome.lines[0]).unwrap()
}

/// Returns each session row's name, state and WHY cell.
fn list_rows(status: &Value) -> Vec<(String, String, String)> {
    status["sessions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| {
            let text = |field: &str| row[field].as_str().unwrap().to_owned();
            (text("name"), text("state"), text("why"))
        })
        .collect()
}

/// Writes a parked record as the later parking step will, with the
/// fields reopening needs.
fn write_parked_record(test_tools: &TestTools, name: &str, card: &str) {
    let parked = test_tools.state_folder().join("hub/parked");
    fs::create_dir_all(&parked).unwrap();
    let record = json!({
        "name": name,
        "kind": "opencode",
        "session_id": format!("ses_{name}"),
        "folder": "/home/u/wt",
        "card": card,
        "workspace_label": card,
        "parked_at": "2026-11-10T22:41:00Z",
    });
    fs::write(parked.join(format!("{name}.json")), record.to_string()).unwrap();
}

/// `hub status` is the one place the hub sees every session and what
/// waits on whom. The hub's own pane as a row could later close the
/// hub itself, and a session left out is one nobody watches. Each
/// source must reach its row. A pending approval from a worker's pane
/// keeps it open. A message a killed process left half sent shows as
/// unsure. The hub named in the last handoff closes. A parked record
/// shows with its time. A worker whose card is not on the board stays
/// kept, since no rule can tell whether its work is done.
#[test]
fn run_status_rows_cover_sessions_not_hub() {
    // The hub's own pane, the old hub it took over from, a working
    // worker, a finished worker whose approval waits, and a session
    // with a half-sent message.
    let mut test_tools = TestTools::new();
    test_tools.own_pane = Some("w1:kanban-hub-18".to_owned());
    // A folder outside any repository, where git finds nothing to keep.
    let plain_folder = test_tools.path().to_owned();
    test_tools.herdr.agents = vec![
        make_agent("kanban-hub-18", AgentStatus::Working),
        make_agent("kanban-hub-16", AgentStatus::Idle),
        make_agent("demo-kb-009-auto", AgentStatus::Working),
        Agent {
            session_id: Some("ses_007".to_owned()),
            folders: vec![plain_folder.clone()],
            ..make_agent("demo-kb-007-impl", AgentStatus::Idle)
        },
        make_agent("docs-fix", AgentStatus::Idle),
        Agent {
            folders: vec![plain_folder],
            ..make_agent("demo-kb-012-impl", AgentStatus::Idle)
        },
    ];
    test_tools.cards.stages = [
        ("DEMO-KB-007".to_owned(), Stage::Review),
        ("DEMO-KB-009".to_owned(), Stage::Active),
        ("DEMO-KB-081".to_owned(), Stage::Review),
    ]
    .into();
    for key in ["DEMO-KB-007", "DEMO-KB-009", "DEMO-KB-081"] {
        test_tools
            .cards
            .card_notes
            .insert(key.to_owned(), CardNote::default());
    }
    let run_path = test_tools.run_path();
    fs::write(
        run_path.join("report-demo-kb-007-impl.2214.read"),
        "PR opened.\n",
    )
    .unwrap();
    fs::write(
        run_path.join("report-demo-kb-012-impl.2210.read"),
        "Tests added.\n",
    )
    .unwrap();
    let handoff = json!({
        "at": "2026-11-11T22:08:00Z",
        "event": "handoff",
        "old": "kanban-hub-16",
        "tab": "w1:t9",
        "new": "kanban-hub-18",
    });
    fs::write(run_path.join("events.jsonl"), format!("{handoff}\n")).unwrap();
    // The approval request came from the finished worker's pane.
    let requests = test_tools.state_folder().join("pr-write");
    fs::create_dir(&requests).unwrap();
    let request = json!({"state": "pending", "pane_id": "w1:demo-kb-007-impl"});
    fs::write(requests.join("req1.json"), request.to_string()).unwrap();
    // A `.sending` file is what a process killed mid-send leaves.
    fs::create_dir(run_path.join("sends")).unwrap();
    fs::write(
        run_path.join("sends/docs-fix.20261111-221400.1.sending"),
        r#"{"name":"docs-fix","machine":null,"text":"Rebase.","created":"2026-11-11T22:14:00Z"}"#,
    )
    .unwrap();
    write_parked_record(&test_tools, "demo-kb-081-auto", "DEMO-KB-081");

    let status = read_status_json(&test_tools);

    assert_eq!(
        list_rows(&status),
        [
            (
                "demo-kb-009-auto".to_owned(),
                "working".to_owned(),
                "-".to_owned()
            ),
            ("docs-fix".to_owned(), "hub".to_owned(), "unsure".to_owned()),
            (
                "demo-kb-007-impl".to_owned(),
                "kept".to_owned(),
                "approval waits".to_owned(),
            ),
            (
                "demo-kb-012-impl".to_owned(),
                "kept".to_owned(),
                "card unknown".to_owned(),
            ),
            (
                "kanban-hub-16".to_owned(),
                "close".to_owned(),
                "handed off".to_owned(),
            ),
            (
                "demo-kb-081-auto".to_owned(),
                "parked".to_owned(),
                "since 11-10 22:41".to_owned(),
            ),
        ]
    );
    assert_eq!(status["sessions"][1]["unsure"], 1);
}

/// The hub reads every unread report on start, and a run folder holds
/// hundreds of archived ones. Listing archives as unread would bury
/// the two the hub must read.
#[test]
fn run_status_lists_only_unread_reports() {
    let test_tools = TestTools::new();
    let run_path = test_tools.run_path();
    for number in 0..50 {
        fs::write(
            run_path.join(format!("report-worker-{number:02}.1200.read")),
            "Done.\n",
        )
        .unwrap();
    }
    fs::write(run_path.join("report-demo-kb-007-impl.md"), "PR opened.\n").unwrap();
    fs::write(run_path.join("report-docs-fix.md"), "Rebased.\n").unwrap();

    let status = read_status_json(&test_tools);

    assert_eq!(status["unread"], json!(["demo-kb-007-impl", "docs-fix"]));
}

/// A `card` that fails, such as without a vault, must not hide the
/// sessions. The hub still sees every row, and one line says why the
/// card facts are missing. An idle worker whose report was read must
/// stay `kept`, since the tidy pass would close a `close` row while
/// its card may still be active.
#[test]
fn run_status_keeps_rows_when_card_fails() {
    let mut test_tools = TestTools::new();
    let plain_folder = test_tools.path().to_owned();
    test_tools.herdr.agents = vec![
        make_agent("demo-kb-009-auto", AgentStatus::Working),
        Agent {
            folders: vec![plain_folder],
            ..make_agent("demo-kb-007-impl", AgentStatus::Idle)
        },
    ];
    fs::write(
        test_tools
            .run_path()
            .join("report-demo-kb-007-impl.2214.read"),
        "PR opened.\n",
    )
    .unwrap();
    test_tools.cards.read_error = Some("card: no default vault".to_owned());
    let now = make_time("2026-11-11T22:14:00+00:00[UTC]");

    let outcome = run_status(&test_tools.tools(), &test_tools.run(), false, &now).unwrap();

    assert_eq!(outcome.lines[1], "board: card: no default vault");
    assert!(
        outcome
            .lines
            .iter()
            .any(|line| line.starts_with("demo-kb-009-auto  working")),
        "{:?}",
        outcome.lines
    );
    assert!(
        outcome.lines.iter().any(
            |line| line.starts_with("demo-kb-007-impl  kept") && line.ends_with("card unknown")
        ),
        "{:?}",
        outcome.lines
    );
    assert_eq!(
        outcome.lines.last().unwrap(),
        "cards: card: no default vault"
    );
}

/// A request file that does not parse may be the approval a finished
/// session waits on. Every session must then stay `kept` until the
/// file is fixed. Otherwise the tidy pass could close the tab whose
/// approval still waits, and the user would lose the way back to it.
#[test]
fn run_status_bad_request_file_keeps_sessions() {
    let mut test_tools = TestTools::new();
    // A folder outside any repository, where git finds nothing to keep.
    let plain_folder = test_tools.path().to_owned();
    test_tools.herdr.agents = vec![Agent {
        folders: vec![plain_folder],
        ..make_agent("demo-kb-007-impl", AgentStatus::Idle)
    }];
    test_tools.cards.stages = [("DEMO-KB-007".to_owned(), Stage::Accepted)].into();
    let requests = test_tools.state_folder().join("pr-write");
    fs::create_dir(&requests).unwrap();
    fs::write(requests.join("req1.json"), r#"{"state": "pend"#).unwrap();

    let status = read_status_json(&test_tools);

    assert_eq!(
        list_rows(&status),
        [(
            "demo-kb-007-impl".to_owned(),
            "kept".to_owned(),
            "approval waits".to_owned(),
        )]
    );
    assert_eq!(
        status["problems"],
        json!(["requests: 1 files did not parse"])
    );
}

/// An active or review card with no live or parked session is work
/// nobody is doing, and the hub must list it as unworked. A card that
/// already has a live or parked session must not appear there, since
/// the work is already under way.
#[test]
fn run_status_names_cards_with_no_session() {
    let mut test_tools = TestTools::new();
    test_tools.herdr.agents = vec![make_agent("demo-kb-007-impl", AgentStatus::Working)];
    test_tools.cards.stages = [
        ("DEMO-KB-004".to_owned(), Stage::Active),
        ("DEMO-KB-007".to_owned(), Stage::Active),
        ("DEMO-KB-010".to_owned(), Stage::Review),
        ("DEMO-KB-011".to_owned(), Stage::Accepted),
    ]
    .into();
    for key in ["DEMO-KB-007", "DEMO-KB-010"] {
        test_tools
            .cards
            .card_notes
            .insert(key.to_owned(), CardNote::default());
    }
    write_parked_record(&test_tools, "demo-kb-010-auto", "DEMO-KB-010");

    let status = read_status_json(&test_tools);

    assert_eq!(
        status["no_session"],
        json!([{"card": "DEMO-KB-004", "stage": "active"}])
    );
}

/// The tidy pass will close or park a `close` or `park` row, so git
/// must look before either. A session with no card must stay while
/// the linked worktree that holds its folder has uncommitted files. A
/// git failure, in listing worktrees or inside one, or an agent Herdr
/// gave no folder, must keep its session too, since nothing is known
/// to be safe.
#[test]
fn run_status_keeps_sessions_git_cannot_clear() {
    let mut test_tools = TestTools::new();
    // A session with no card works in a linked worktree with a new file.
    let dirty_worktree = make_card_repo(&test_tools.path().join("notes-repo"))
        .join("../card-wt")
        .canonicalize()
        .unwrap();
    fs::write(dirty_worktree.join("notes.txt"), "x").unwrap();
    fs::write(
        test_tools.run_path().join("report-notes.2214.read"),
        "Notes written.\n",
    )
    .unwrap();
    // A review worker's card worktree has a broken `.git` file, so git
    // fails inside it.
    let broken_main = make_card_repo(&test_tools.path().join("broken-repo"));
    let broken_worktree = broken_main.join("../card-wt");
    fs::write(broken_worktree.join(".git"), "gitdir: /gone\n").unwrap();
    // A second review worker has no folder from Herdr. A third works
    // inside the broken worktree, where `git worktree list` fails.
    test_tools.herdr.agents = vec![
        Agent {
            folders: vec![dirty_worktree.clone()],
            ..make_agent("notes", AgentStatus::Idle)
        },
        Agent {
            session_id: Some("ses_001".to_owned()),
            folders: vec![broken_main],
            ..make_agent("demo-kb-001-impl", AgentStatus::Idle)
        },
        Agent {
            session_id: Some("ses_002".to_owned()),
            ..make_agent("demo-kb-002-impl", AgentStatus::Idle)
        },
        Agent {
            session_id: Some("ses_003".to_owned()),
            folders: vec![broken_worktree],
            ..make_agent("demo-kb-003-impl", AgentStatus::Idle)
        },
    ];
    test_tools.cards.stages = [
        ("DEMO-KB-001".to_owned(), Stage::Review),
        ("DEMO-KB-002".to_owned(), Stage::Review),
        ("DEMO-KB-003".to_owned(), Stage::Review),
    ]
    .into();
    for key in ["DEMO-KB-001", "DEMO-KB-002", "DEMO-KB-003"] {
        test_tools
            .cards
            .card_notes
            .insert(key.to_owned(), CardNote::default());
    }

    let rows = list_rows(&read_status_json(&test_tools));

    assert_eq!(rows.len(), 4, "{rows:?}");
    assert_eq!(rows[0].0, "demo-kb-001-impl");
    assert_eq!(rows[0].1, "kept");
    assert!(rows[0].2.starts_with("git failed, "), "{rows:?}");
    assert_eq!(
        rows[1],
        (
            "demo-kb-002-impl".to_owned(),
            "kept".to_owned(),
            "git failed, Herdr gave no folder".to_owned(),
        )
    );
    assert_eq!(rows[2].0, "demo-kb-003-impl");
    assert_eq!(rows[2].1, "kept");
    assert!(rows[2].2.starts_with("git failed, "), "{rows:?}");
    assert_eq!(
        rows[3],
        (
            "notes".to_owned(),
            "kept".to_owned(),
            format!("unsaved files in {}", dirty_worktree.display()),
        )
    );
}
