mod common;

use std::fs::{self, File};
use std::time::{Duration, SystemTime};

use hub::commands::next::{NextOptions, run_next};
use hub::commands::send::{SendRequest, run_send};
use hub::external::cards::{BoardCounts, CardNote, Stage};
use hub::external::herdr::{Agent, AgentStatus};
use serde_json::{Value, json};

use common::{
    OpenCodeRow, TestTools, make_agent, make_time, read_events, write_opencode_db,
    write_opencode_session,
};

/// Runs one `hub next` check and returns its line.
fn check_once(test_tools: &TestTools) -> String {
    let options = NextOptions {
        timeout: Some(Duration::ZERO),
        poll: Duration::ZERO,
        json: false,
        process_id: 1,
    };
    let outcome = run_next(&test_tools.tools(), &test_tools.run(), &options).unwrap();
    outcome.lines.join("\n")
}

/// Writes `report-NAME.md` with a modification time a minute ago, old
/// enough for `hub next` to announce it.
fn write_old_report(test_tools: &TestTools, name: &str) {
    let path = test_tools.run_path().join(format!("report-{name}.md"));
    fs::write(&path, "PR opened.\n").unwrap();
    let a_minute_ago = SystemTime::now() - Duration::from_secs(60);
    File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_modified(a_minute_ago)
        .unwrap();
}

/// Deletes `.minute-pass.json`, so the next check runs the minute pass
/// at once instead of a minute later.
fn forget_minute_pass(test_tools: &TestTools) {
    fs::remove_file(test_tools.run_path().join(".minute-pass.json")).unwrap();
}

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

/// Makes the hub's own pane `w1:kanban-hub` hold session `ses_hub`,
/// whose last step read `tokens` of context.
fn set_hub_context(test_tools: &mut TestTools, tokens: u64) {
    test_tools.herdr.agents.push(Agent {
        session_id: Some("ses_hub".to_owned()),
        ..make_agent("kanban-hub", AgentStatus::Working)
    });
    test_tools.own_pane = Some("w1:kanban-hub".to_owned());
    let step = OpenCodeRow {
        session: "ses_hub",
        kind: "assistant",
        updated_ms: 0,
        data: json!({"tokens": {"input": tokens, "cache": {"read": 0, "write": 0}}}),
    };
    write_opencode_db(&test_tools.opencode_path(), &[step]);
}

/// The hub runs `hub read` and a new `hub next` in one reply, so a
/// report line must be recorded when it prints. Wiring that prints it
/// but never records it wakes the hub for the same report on every
/// restart. Each line ends with the hub's own context.
#[test]
fn run_next_records_report_it_prints() {
    let mut test_tools = TestTools::new();
    set_hub_context(&mut test_tools, 131_204);
    write_old_report(&test_tools, "docs-fix");

    let first = check_once(&test_tools);
    let second = check_once(&test_tools);

    assert_eq!(first, "report docs-fix (hub 131k)");
    assert_eq!(second, "timeout (hub 131k)");
    let events = read_events(&test_tools.run_path());
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["event"], "announced");
    assert_eq!(events[0]["name"], "docs-fix");
}

/// A session that stays blocked must wake the hub once, not every 5
/// seconds, and must wake it again when it blocks a second time.
#[test]
fn run_next_prints_each_blocked_state_once() {
    let mut test_tools = TestTools::new();
    let blocked = |state_seq| Agent {
        state_seq,
        ..make_agent("demo-kb-009-auto", AgentStatus::Blocked)
    };
    test_tools.herdr.agents = vec![blocked(4)];

    let first = check_once(&test_tools);
    let second = check_once(&test_tools);
    test_tools.herdr.agents = vec![blocked(6)];
    let third = check_once(&test_tools);

    assert_eq!(first, "blocked demo-kb-009-auto (hub -)");
    assert_eq!(second, "timeout (hub -)");
    assert_eq!(third, "blocked demo-kb-009-auto (hub -)");
}

/// The hub must hear "hand off now" once it reaches 150k, and only
/// once, or it either runs out of context or is told on every wake.
#[test]
fn run_next_tells_hub_hand_off_once() {
    let mut test_tools = TestTools::new();
    set_hub_context(&mut test_tools, 150_000);

    let first = check_once(&test_tools);
    let second = check_once(&test_tools);

    assert_eq!(first, "hand off now: hub at 150k");
    assert_eq!(second, "timeout (hub 150k)");
}

/// A crash can cut the log's last line short, and a newer hub can
/// write events this one does not know. Either must be skipped while
/// the lines after it still count, or `hub next` stops for the rest of
/// the run or announces a report again.
#[test]
fn run_next_reads_log_past_broken_lines() {
    let test_tools = TestTools::new();
    write_old_report(&test_tools, "docs-fix");
    check_once(&test_tools);
    let run_path = test_tools.run_path();
    let announced = fs::read_to_string(run_path.join("events.jsonl")).unwrap();
    let unknown = r#"{"at":"2026-11-11T03:14:03Z","event":"newer_kind","name":"docs-fix"}"#;
    let cut_off = r#"{"at":"2026-11-11T03:14:05Z","event":"anno"#;
    fs::write(
        run_path.join("events.jsonl"),
        format!("{unknown}\n{announced}{cut_off}"),
    )
    .unwrap();

    let line = check_once(&test_tools);

    assert_eq!(line, "timeout (hub -)");
}

/// A newer `hub next` can take over while an older one is still in its
/// check. The older one must then record nothing, or the newer one
/// never prints that report and the hub never reads it.
#[test]
fn run_next_records_nothing_once_replaced() {
    let mut test_tools = TestTools::new();
    test_tools.herdr.agents = vec![make_agent("docs-fix", AgentStatus::Blocked)];
    let request = SendRequest {
        name: "docs-fix".parse().unwrap(),
        machine: None,
        text: "Rebase on main.".to_owned(),
    };
    let now = make_time("2026-11-11T22:14:00+00:00[UTC]");
    run_send(&test_tools.tools(), &test_tools.run(), &request, &now).unwrap();
    test_tools.herdr.agents = vec![make_agent("docs-fix", AgentStatus::Idle)];
    write_old_report(&test_tools, "notes-fix");
    // A newer watcher claims the run folder while the held message is
    // typed.
    let claim = test_tools.run_path().join(".watcher-pid");
    test_tools.herdr.on_prompt = Some(Box::new(move || fs::write(&claim, "2\n").unwrap()));

    let replaced = check_once(&test_tools);
    let next_check = check_once(&test_tools);

    assert_eq!(replaced, "replaced by a newer hub next");
    assert_eq!(next_check, "report notes-fix (hub -)");
}

/// A worker past 150k must be asked once to write its handoff, or it
/// runs out of context mid-task. Asking on every minute pass interrupts
/// the handoff it is writing.
#[test]
fn run_next_asks_worker_handoff_once() {
    let mut test_tools = TestTools::new();
    test_tools.herdr.agents = vec![Agent {
        session_id: Some("ses_007".to_owned()),
        ..make_agent("demo-kb-007-impl", AgentStatus::Working)
    }];
    let step = OpenCodeRow {
        session: "ses_007",
        kind: "assistant",
        updated_ms: 0,
        data: json!({"tokens": {"input": 152_000, "cache": {"read": 0, "write": 0}}}),
    };
    write_opencode_db(&test_tools.opencode_path(), &[step]);

    check_once(&test_tools);
    forget_minute_pass(&test_tools);
    check_once(&test_tools);

    let ask = "Your context is about 152k. Write your handoff with herdr-handoff now, \
               put its path in your report, and end your turn.";
    assert_eq!(
        *test_tools.herdr.prompts.borrow(),
        [("demo-kb-007-impl".to_owned(), ask.to_owned())]
    );
    let asked: Vec<Value> = read_events(&test_tools.run_path())
        .into_iter()
        .filter(|event| event["event"] == "handoff_asked")
        .collect();
    assert_eq!(asked.len(), 1);
    assert_eq!(asked[0]["session"], "ses_007");
}

/// The hub must hear once when the board goes over its limit, so it
/// starts no new work, and again after it fell back and rose again.
/// Telling it on every check costs a hub turn each time, and never
/// telling it again lets the board pile up a second time.
#[test]
fn run_next_wakes_once_per_board_crossing() {
    let mut test_tools = TestTools::new();
    let board = |busy| BoardCounts {
        active: busy,
        review: 0,
        busy,
        limit: 8,
    };
    test_tools.cards.board = Some(board(25));

    let first = check_once(&test_tools);
    forget_minute_pass(&test_tools);
    let still_over = check_once(&test_tools);
    test_tools.cards.board = Some(board(5));
    forget_minute_pass(&test_tools);
    let under = check_once(&test_tools);
    test_tools.cards.board = Some(board(25));
    forget_minute_pass(&test_tools);
    let over_again = check_once(&test_tools);

    assert_eq!(first, "board over its limit: 25/8 (hub -)");
    assert_eq!(still_over, "timeout (hub -)");
    assert_eq!(under, "timeout (hub -)");
    assert_eq!(over_again, "board over its limit: 25/8 (hub -)");
}

/// `hub next` must name finished sessions and leave them open, so the
/// hub decides when to run `hub clean`. A minute pass that closed them
/// itself once closed a worker that had moved to another card and asked
/// for a pull request approval, and the user waited on a request that
/// no longer existed.
#[test]
fn run_next_only_lists_finished_sessions() {
    let mut test_tools = TestTools::new();
    add_finished_worker(
        &mut test_tools,
        "demo-kb-001-impl",
        "DEMO-KB-001",
        Stage::Accepted,
    );
    add_finished_worker(
        &mut test_tools,
        "demo-kb-007-impl",
        "DEMO-KB-007",
        Stage::Review,
    );

    let first = check_once(&test_tools);
    let second = check_once(&test_tools);

    assert_eq!(
        first,
        "to-close demo-kb-001-impl: DEMO-KB-001 accepted; run hub clean"
    );
    assert_eq!(
        second,
        "to-park demo-kb-007-impl: DEMO-KB-007 review; run hub clean"
    );
    assert!(test_tools.herdr.closed_tabs.borrow().is_empty());
    assert!(test_tools.cards.links.borrow().is_empty());
    assert!(!test_tools.state_folder().join("hub/parked").exists());
}

/// The hub must hear about a finished session once. Listing it again on
/// every minute pass costs the hub a turn each minute until it runs
/// `hub clean`.
#[test]
fn run_next_lists_each_finished_session_once() {
    let mut test_tools = TestTools::new();
    add_finished_worker(
        &mut test_tools,
        "demo-kb-001-impl",
        "DEMO-KB-001",
        Stage::Accepted,
    );

    let listed = check_once(&test_tools);
    forget_minute_pass(&test_tools);
    let next_pass = check_once(&test_tools);

    assert_eq!(
        listed,
        "to-close demo-kb-001-impl: DEMO-KB-001 accepted; run hub clean"
    );
    assert_eq!(next_pass, "timeout (hub -)");
}

/// A finished worker over the handoff limit gets no handoff ask. The
/// ask makes a worker the user is reviewing write a handoff nobody
/// needs, and once `hub clean` parks it, the held ask reopens it.
#[test]
fn run_next_skips_handoff_of_finished_worker() {
    let mut test_tools = TestTools::new();
    add_finished_worker(
        &mut test_tools,
        "demo-kb-007-impl",
        "DEMO-KB-007",
        Stage::Review,
    );
    let folder = test_tools.path().to_owned();
    let step = OpenCodeRow {
        session: "ses_demo-kb-007-impl",
        kind: "assistant",
        updated_ms: 0,
        data: json!({"tokens": {"input": 152_000, "cache": {"read": 0, "write": 0}}}),
    };
    write_opencode_db(&test_tools.opencode_path(), &[step]);
    write_opencode_session(&test_tools.opencode_path(), "ses_demo-kb-007-impl", &folder);

    let first = check_once(&test_tools);
    forget_minute_pass(&test_tools);
    check_once(&test_tools);

    assert_eq!(
        first,
        "to-park demo-kb-007-impl: DEMO-KB-007 review; run hub clean"
    );
    assert!(test_tools.herdr.closed_tabs.borrow().is_empty());
    assert!(test_tools.herdr.prompts.borrow().is_empty());
}
