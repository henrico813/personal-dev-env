mod common;

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use hub::commands::Outcome;
use hub::commands::clean::run_park;
use hub::commands::next::{NextOptions, run_next};
use hub::commands::send::{SendRequest, run_send};
use hub::external::cards::{CardNote, LoggedSession, Stage};
use hub::external::herdr::{Agent, AgentStatus, Workspace};
use hub::shared::HubError;
use serde_json::{Value, json};

use common::{
    TestTools, make_agent, make_time, read_events, write_opencode_session, write_opencode_times,
};

/// The worker every test here parks or reopens.
const WORKER: &str = "demo-kb-007-impl";

/// Parks WORKER as the tidy pass leaves it, with a record for session
/// `ses_007` on card DEMO-KB-007 and the session's row in the OpenCode
/// database with its folder. Once an agent starts, Herdr shows WORKER
/// idle with `session_id`.
fn park_worker(test_tools: &mut TestTools, session_id: &str) {
    let parked = test_tools.state_folder().join("hub/parked");
    fs::create_dir_all(&parked).unwrap();
    let record = json!({
        "name": WORKER,
        "session_id": "ses_007",
        "card": "DEMO-KB-007",
        "workspace_label": "DEMO-KB-007 impl",
        "parked_at": "2026-11-10T22:41:00Z",
    });
    fs::write(parked.join(format!("{WORKER}.json")), record.to_string()).unwrap();
    write_opencode_session(
        &test_tools.build_opencode_path(),
        "ses_007",
        Path::new("/home/u/wt/demo-kb-007"),
    );
    test_tools.herdr.after_start = vec![Agent {
        session_id: Some(session_id.to_owned()),
        ..make_agent(WORKER, AgentStatus::Idle)
    }];
}

/// Tells whether WORKER still has a parked record.
fn has_parked_record(test_tools: &TestTools) -> bool {
    test_tools
        .state_folder()
        .join(format!("hub/parked/{WORKER}.json"))
        .exists()
}

/// Runs `hub send WORKER TEXT` on this machine at `22:14`.
fn send(test_tools: &TestTools, text: &str) -> Result<Outcome, HubError> {
    let request = SendRequest {
        name: WORKER.parse().unwrap(),
        machine: None,
        text: text.to_owned(),
    };
    let now = make_time("2026-11-11T22:14:00+00:00[UTC]");
    run_send(&test_tools.tools(), &test_tools.run(), &request, &now)
}

/// Runs one `hub next` check and returns its line.
fn check_once(test_tools: &TestTools) -> String {
    let options = NextOptions {
        timeout: Some(Duration::ZERO),
        poll: Duration::ZERO,
        json: false,
        process_id: 1,
        cwd: test_tools.path().to_owned(),
        start_reviews: false,
    };
    let outcome = run_next(&test_tools.tools(), &test_tools.run(), &options).unwrap();
    outcome.lines.join("\n")
}

/// Adds WORKER idle on card DEMO-KB-007 in review with its report read,
/// so it can be parked. Herdr shows `session_id`, and the card's Log
/// lists each `(agent name, session ID)` in `logged`, oldest first.
fn add_finished_worker(test_tools: &mut TestTools, session_id: &str, logged: &[(&str, &str)]) {
    let folder = test_tools.path().to_owned();
    test_tools.herdr.agents.push(Agent {
        session_id: Some(session_id.to_owned()),
        folders: vec![folder],
        ..make_agent(WORKER, AgentStatus::Idle)
    });
    test_tools
        .cards
        .stages
        .insert("DEMO-KB-007".to_owned(), Stage::InReview);
    let logged_sessions = logged
        .iter()
        .map(|(name, session_id)| LoggedSession {
            name: (*name).to_owned(),
            session_id: (*session_id).to_owned(),
        })
        .collect();
    let note = CardNote {
        logged_sessions,
        ..CardNote::default()
    };
    test_tools
        .cards
        .card_notes
        .insert("DEMO-KB-007".to_owned(), note);
    let archive = format!("report-{WORKER}.2214.read");
    fs::write(test_tools.run_path().join(archive), "PR opened.\n").unwrap();
}

/// Runs `hub park WORKER` at `22:14` and returns WORKER's parked record.
fn park_and_read_record(test_tools: &TestTools) -> Value {
    let now = make_time("2026-11-11T22:14:00+00:00[UTC]");
    run_park(
        &test_tools.tools(),
        &test_tools.run(),
        &WORKER.parse().unwrap(),
        &now,
    )
    .unwrap();
    let path = test_tools
        .state_folder()
        .join(format!("hub/parked/{WORKER}.json"));
    serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
}

/// A message to a parked worker, such as review comments, must reach
/// the same session in its own folder, with `HUB_RUN` set so its
/// report reaches this hub. A wrong ID starts an empty session that
/// knows nothing of the work, and a record left behind reopens it
/// twice.
#[test]
fn run_send_reopens_parked_session() {
    let mut test_tools = TestTools::new();
    park_worker(&mut test_tools, "ses_007");
    test_tools.herdr.workspaces = vec![Workspace {
        workspace_id: "w2".to_owned(),
        label: "DEMO-KB-007 impl".to_owned(),
    }];

    let outcome = send(&test_tools, "Review comments are in.").unwrap();

    assert_eq!(
        outcome.lines,
        ["reopening demo-kb-007-impl; held until it starts"]
    );
    let created = test_tools.herdr.created_tabs.borrow();
    let (workspace_id, place) = &created[0];
    assert_eq!(workspace_id, "w2");
    assert_eq!(place.label, WORKER);
    assert_eq!(place.cwd, PathBuf::from("/home/u/wt/demo-kb-007"));
    assert_eq!(
        place.env,
        [(
            "HUB_RUN".to_owned(),
            test_tools.run().path().display().to_string()
        )]
    );
    let started = test_tools.herdr.started.borrow();
    assert_eq!(started[0].pane_id, "w2:p-demo-kb-007-impl");
    assert_eq!(started[0].args, ["--session", "ses_007", "--auto"]);
    assert!(!has_parked_record(&test_tools));
    let events = read_events(&test_tools.run_path());
    let kinds: Vec<&str> = events
        .iter()
        .map(|event| event["event"].as_str().unwrap())
        .collect();
    assert_eq!(kinds, ["held", "reopened"]);
}

/// Herdr may close a workspace with its last tab, so a parked worker's
/// workspace is often gone. Reopening must then make the workspace
/// again under its old label, or every such session cannot reopen.
#[test]
fn run_send_makes_workspace_when_none_left() {
    let mut test_tools = TestTools::new();
    park_worker(&mut test_tools, "ses_007");

    send(&test_tools, "Review comments are in.").unwrap();

    let created = test_tools.herdr.created_workspaces.borrow();
    assert_eq!(created[0].label, "DEMO-KB-007 impl");
    assert!(test_tools.herdr.created_tabs.borrow().is_empty());
    assert_eq!(
        test_tools.herdr.started.borrow()[0].pane_id,
        "new:p-DEMO-KB-007 impl"
    );
}

/// `opencode --session` with an ID it lacks quietly starts an empty
/// session. A record whose session is gone must start nothing and keep
/// the record, and the message stays held for the hub to see.
#[test]
fn run_send_starts_nothing_for_unknown_session() {
    let mut test_tools = TestTools::new();
    park_worker(&mut test_tools, "ses_007");
    let other = test_tools.path().join("other.db");
    write_opencode_session(&other, "ses_other", Path::new("/home/u"));
    fs::rename(other, test_tools.build_opencode_path()).unwrap();

    let error = send(&test_tools, "Review comments are in.").unwrap_err();

    assert_eq!(
        error.to_string(),
        "held demo-kb-007-impl, but not sent demo-kb-007-impl: the session is not in the database"
    );
    assert!(test_tools.herdr.started.borrow().is_empty());
    assert!(has_parked_record(&test_tools));
}

/// A launcher that ignored `--session` starts a new, empty session
/// under the worker's name. The hub must close that tab and keep the
/// record, or the held message lands in a session that knows nothing.
/// The tab it closes is the one the reopen made.
#[test]
fn run_send_closes_tab_of_other_session() {
    let mut test_tools = TestTools::new();
    park_worker(&mut test_tools, "ses_new");
    test_tools.herdr.after_start[0].pane_id = "new:p-DEMO-KB-007 impl".to_owned();
    test_tools.herdr.after_start[0].tab_id = "new:t1".to_owned();

    let error = send(&test_tools, "Review comments are in.").unwrap_err();

    assert!(
        error.to_string().ends_with("another session started"),
        "{error}"
    );
    assert_eq!(*test_tools.herdr.closed_tabs.borrow(), ["new:t1"]);
    assert!(has_parked_record(&test_tools));
}

/// Herdr learns a session ID only after OpenCode starts, so the first
/// read after a reopen may show none. Reopening must read again, or
/// every reopen closes its tab and says another session started.
#[test]
fn run_send_waits_for_session_id() {
    let mut test_tools = TestTools::new();
    park_worker(&mut test_tools, "ses_007");
    let resumed = test_tools.herdr.after_start[0].clone();
    test_tools.herdr.after_start = vec![
        Agent {
            session_id: None,
            ..resumed.clone()
        },
        resumed,
    ];

    let outcome = send(&test_tools, "Review comments are in.").unwrap();

    assert_eq!(
        outcome.lines,
        ["reopening demo-kb-007-impl; held until it starts"]
    );
    assert!(test_tools.herdr.closed_tabs.borrow().is_empty());
    assert!(!has_parked_record(&test_tools));
}

/// A workspace label is matched by the whole card key, so the worker
/// for DEMO-KB-007 never reopens inside DEMO-KB-0071's workspace, where
/// the user would look for other work.
#[test]
fn run_send_skips_workspace_of_longer_key() {
    let mut test_tools = TestTools::new();
    park_worker(&mut test_tools, "ses_007");
    test_tools.herdr.workspaces = vec![Workspace {
        workspace_id: "w3".to_owned(),
        label: "DEMO-KB-0071 impl".to_owned(),
    }];

    send(&test_tools, "Review comments are in.").unwrap();

    assert!(test_tools.herdr.created_tabs.borrow().is_empty());
    let created = test_tools.herdr.created_workspaces.borrow();
    assert_eq!(created[0].label, "DEMO-KB-007 impl");
}

/// A message held for a live worker that is parked before it goes out
/// must reopen the worker, then reach it once it starts. Without this,
/// a send that races a park leaves the message held forever.
#[test]
fn run_next_reopens_session_with_held_message() {
    let mut test_tools = TestTools::new();
    // Hold a message while the user is in the worker's pane.
    test_tools.herdr.agents = vec![Agent {
        focused: true,
        ..make_agent(WORKER, AgentStatus::Idle)
    }];
    send(&test_tools, "Review comments are in.").unwrap();
    // Park the worker before the message goes out.
    test_tools.herdr.agents.clear();
    park_worker(&mut test_tools, "ses_007");

    let first = check_once(&test_tools);
    let second = check_once(&test_tools);

    assert_eq!(first, "timeout (hub -)");
    assert_eq!(second, "timeout (hub -)");
    assert_eq!(test_tools.herdr.started.borrow().len(), 1);
    assert_eq!(
        *test_tools.herdr.prompts.borrow(),
        [(WORKER.to_owned(), "hub: Review comments are in.".to_owned())]
    );
}

/// A held message whose parked session cannot reopen must reach the
/// hub as `not sent`, once. Kept held, it would be tried on every
/// check and never sent, and nothing would tell the hub.
#[test]
fn run_next_prints_failed_reopen_once() {
    let mut test_tools = TestTools::new();
    // Hold a message while the user is in the worker's pane.
    test_tools.herdr.agents = vec![Agent {
        focused: true,
        ..make_agent(WORKER, AgentStatus::Idle)
    }];
    send(&test_tools, "Review comments are in.").unwrap();
    // Park the worker, then lose its session from the database.
    test_tools.herdr.agents.clear();
    park_worker(&mut test_tools, "ses_007");
    fs::remove_file(test_tools.build_opencode_path()).unwrap();

    let first = check_once(&test_tools);
    let second = check_once(&test_tools);

    assert_eq!(
        first,
        "not sent demo-kb-007-impl: the session is not in the database (hub -)"
    );
    assert_eq!(second, "timeout (hub -)");
    assert!(test_tools.herdr.started.borrow().is_empty());
}

/// The record and the `parked` event must name the session the card Log
/// lists for the worker, not the nested `opencode run` Herdr shows, or a
/// reopen resumes the nested run.
#[test]
fn run_park_saves_launched_session() {
    let mut test_tools = TestTools::new();
    add_finished_worker(&mut test_tools, "ses_herdr", &[(WORKER, "ses_launched")]);
    // `ses_launched` spans the start of Herdr's nested session.
    let opencode = test_tools.build_opencode_path();
    write_opencode_times(&opencode, "ses_launched", 1_000, 9_000);
    write_opencode_times(&opencode, "ses_herdr", 5_000, 8_000);

    let record = park_and_read_record(&test_tools);

    assert_eq!(record["session_id"], "ses_launched");
    let events = read_events(&test_tools.run_path());
    assert_eq!(events.last().unwrap()["session"], "ses_launched");
}

/// A session of another agent, an old one of the same name, or a nested
/// run an earlier park linked must not replace Herdr's, or a reopen
/// resumes another agent's or an ended session.
#[test]
fn run_park_keeps_herdr_session_without_match() {
    // Herdr's session `ses_herdr` began at 5000. `ses_launched` spans that
    // start, `ses_old` ended before it and `ses_late` began after it.
    let cases = [
        ("other-name", "demo-kb-007-plan", "ses_launched"),
        ("reused-name", WORKER, "ses_old"),
        ("relinked-nested", WORKER, "ses_late"),
    ];

    for (case, logged_name, logged_session) in cases {
        let mut test_tools = TestTools::new();
        add_finished_worker(
            &mut test_tools,
            "ses_herdr",
            &[(logged_name, logged_session)],
        );
        let opencode = test_tools.build_opencode_path();
        write_opencode_times(&opencode, "ses_launched", 1_000, 9_000);
        write_opencode_times(&opencode, "ses_old", 500, 3_000);
        write_opencode_times(&opencode, "ses_late", 6_000, 9_000);
        write_opencode_times(&opencode, "ses_herdr", 5_000, 8_000);

        let record = park_and_read_record(&test_tools);

        assert_eq!(record["session_id"], "ses_herdr", "{case}");
        let events = read_events(&test_tools.run_path());
        assert_eq!(events.last().unwrap()["session"], "ses_herdr", "{case}");
    }
}
