mod common;

use std::fs;
use std::path::{Path, PathBuf};

use hub::commands::requests::run_requests;
use hub::external::herdr::{Agent, AgentStatus};
use jiff::{SignedDuration, Timestamp};
use serde_json::{Value, json};

use common::{TestTools, make_agent};

const NOW: &str = "2026-11-16T10:00:00Z";

/// Test tools with two live agents, demo-kb-007-impl matched by pane
/// and demo-kb-008-impl by session ID `ses_008`, and an empty
/// pde-gh-write request folder.
fn tools_with_live_agents() -> (TestTools, PathBuf) {
    let mut test_tools = TestTools::new();
    test_tools.herdr.agents = vec![
        make_agent("demo-kb-007-impl", AgentStatus::Idle),
        Agent {
            session_id: Some("ses_008".to_owned()),
            ..make_agent("demo-kb-008-impl", AgentStatus::Idle)
        },
    ];
    let requests = test_tools.state_folder().join("pr-write");
    fs::create_dir(&requests).unwrap();
    (test_tools, requests)
}

/// A saved merge request for demo/app #20, last changed `changed_age`
/// before `NOW`, filed from `source` (a `pane_id` or `session_id`).
fn write_request(folder: &Path, id: &str, state: &str, changed_age: SignedDuration, source: Value) {
    let now: Timestamp = NOW.parse().unwrap();
    let mut request = json!({
        "id": id,
        "state": state,
        "repo": "demo/app",
        "args": ["pr", "merge", "20"],
        "updated": (now - changed_age).to_string(),
    });
    request
        .as_object_mut()
        .unwrap()
        .extend(source.as_object().unwrap().clone());
    fs::write(folder.join(format!("{id}.json")), request.to_string()).unwrap();
}

/// Hubs read a request gone from a quick check as stale when its file
/// said `ran`, and repeated "approval waits" after the user approved.
/// Each row must give the file's own state, with `expired` for a
/// pending request past four hours, and name the live session that
/// filed it, newest change first.
#[test]
fn run_requests_shows_state_and_filing_session() {
    let (test_tools, requests) = tools_with_live_agents();
    let from_007 = json!({"pane_id": "w1:demo-kb-007-impl"});
    write_request(
        &requests,
        "aa0001",
        "ran",
        SignedDuration::from_mins(40),
        from_007.clone(),
    );
    write_request(
        &requests,
        "aa0002",
        "pending",
        SignedDuration::from_hours(5),
        from_007,
    );
    write_request(
        &requests,
        "aa0004",
        "pending",
        SignedDuration::from_mins(5),
        json!({"session_id": "ses_008"}),
    );
    write_request(
        &requests,
        "aa0005",
        "approved",
        SignedDuration::from_mins(2),
        json!({"pane_id": "w9:gone"}),
    );

    let outcome = run_requests(&test_tools.tools(), false, NOW.parse().unwrap()).unwrap();

    assert_eq!(
        outcome.lines,
        [
            "REF      SESSION           STATE     AGE    PULL REQUEST",
            "Aaa0005  -                 approved  2m     app #20 merge",
            "Aaa0004  demo-kb-008-impl  pending   5m     app #20 merge",
            "Aaa0001  demo-kb-007-impl  ran       40m    app #20 merge",
            "Aaa0002  demo-kb-007-impl  expired   5h00m  app #20 merge",
        ]
    );
}

/// A live session's finished request from over a day ago, a closed
/// session's finished request, and a run that died weeks ago would
/// bury the requests a hub must act on, so they do not show beside
/// a live session's recent one.
#[test]
fn run_requests_hides_old_or_orphaned_requests() {
    let (test_tools, requests) = tools_with_live_agents();
    write_request(
        &requests,
        "aa0001",
        "ran",
        SignedDuration::from_mins(40),
        json!({"pane_id": "w1:demo-kb-007-impl"}),
    );
    write_request(
        &requests,
        "aa0003",
        "ran",
        SignedDuration::from_hours(30),
        json!({"pane_id": "w1:demo-kb-007-impl"}),
    );
    write_request(
        &requests,
        "aa0006",
        "ran",
        SignedDuration::from_mins(1),
        json!({"pane_id": "w9:gone"}),
    );
    write_request(
        &requests,
        "aa0007",
        "running",
        SignedDuration::from_hours(600),
        json!({"pane_id": "w9:gone"}),
    );

    let outcome = run_requests(&test_tools.tools(), false, NOW.parse().unwrap()).unwrap();

    assert_eq!(
        outcome.lines,
        [
            "REF      SESSION           STATE  AGE  PULL REQUEST",
            "Aaa0001  demo-kb-007-impl  ran    40m  app #20 merge",
        ]
    );
}
