mod common;

use std::cell::Cell;
use std::fs;
use std::path::Path;

use hub::commands::Outcome;
use hub::commands::clean::{run_clean, run_park};
use hub::external::cards::{CardNote, Stage};
use hub::external::herdr::{Agent, AgentStatus, Pane};
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

/// Runs `hub clean`, or `hub clean --dry-run`, at `22:14`.
fn clean(test_tools: &TestTools, dry_run: bool) -> Outcome {
    let now = make_time("2026-11-11T22:14:00+00:00[UTC]");
    run_clean(&test_tools.tools(), &test_tools.run(), dry_run, &now).unwrap()
}

/// Runs `hub park NAME` at `22:14`.
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

/// A finished session on an accepted card loses its tab, and the line
/// says which and why. A close with no line, or a line with no close,
/// tells the user the wrong thing. `notes.md` and the event log are the
/// only record of what the hub closed on its own.
#[test]
fn run_clean_closes_tab_and_says_why() {
    let mut test_tools = TestTools::new();
    add_finished_worker(
        &mut test_tools,
        "demo-kb-001-impl",
        "DEMO-KB-001",
        Stage::Accepted,
    );

    let outcome = clean(&test_tools, false);

    assert_eq!(
        outcome.lines,
        ["closed demo-kb-001-impl: DEMO-KB-001 accepted"]
    );
    assert_eq!(
        *test_tools.herdr.closed_tabs.borrow(),
        ["w1:t-demo-kb-001-impl"]
    );
    assert!(
        read_notes(&test_tools.run_path())
            .contains("- 2214 closed demo-kb-001-impl (DEMO-KB-001 accepted)\n")
    );
    let events = read_events(&test_tools.run_path());
    assert_eq!(events.last().unwrap()["event"], "closed");
    assert_eq!(events.last().unwrap()["tab"], "w1:t-demo-kb-001-impl");
}

/// The user reads `hub clean --dry-run` on real tabs before the first
/// real run, so it must close, park, link and write nothing while it
/// names what it would do.
#[test]
fn run_clean_dry_run_changes_nothing() {
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

    let outcome = clean(&test_tools, true);

    assert_eq!(
        outcome.lines,
        [
            "would close demo-kb-001-impl: DEMO-KB-001 accepted",
            "would park demo-kb-007-impl: DEMO-KB-007 review",
        ]
    );
    assert!(test_tools.herdr.closed_tabs.borrow().is_empty());
    assert!(test_tools.cards.links.borrow().is_empty());
    assert!(test_tools.cards.notes.borrow().is_empty());
    assert_eq!(read_parked_record(&test_tools, "demo-kb-007-impl"), None);
    assert!(read_events(&test_tools.run_path()).is_empty());
}

/// The hub runs `hub send` as a process of its own, beside `hub clean`.
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
        Stage::Accepted,
    );
    add_finished_worker(
        &mut test_tools,
        "demo-kb-007-impl",
        "DEMO-KB-007",
        Stage::Review,
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
        Stage::Accepted,
    );
    add_finished_worker(
        &mut test_tools,
        "demo-kb-007-impl",
        "DEMO-KB-007",
        Stage::Review,
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
        Stage::Accepted,
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
    let user_pane = || vec!["w1:p-user".to_owned()];
    let cases = [
        (
            "quiet-shell",
            user_pane(),
            Vec::new(),
            vec!["closed demo-kb-001-impl: DEMO-KB-001 accepted"],
            vec!["w1:t-demo-kb-001-impl"],
        ),
        (
            "editor-in-front",
            Vec::new(),
            user_pane(),
            vec!["nothing to close"],
            Vec::new(),
        ),
    ];

    for (case_name, quiet_shells, program_in_front, expected_lines, expected_closed) in cases {
        let mut test_tools = TestTools::new();
        add_finished_worker(
            &mut test_tools,
            "demo-kb-001-impl",
            "DEMO-KB-001",
            Stage::Accepted,
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
        Stage::Review,
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
/// session. `hub clean` must keep it while the card waits in review,
/// or the next message to that session has nothing to reopen.
#[test]
fn run_clean_keeps_record_of_parked_session() {
    let mut test_tools = TestTools::new();
    test_tools
        .cards
        .stages
        .insert("DEMO-KB-007".to_owned(), Stage::Review);
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
        Stage::Accepted,
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

/// A worker whose card is accepted may still be merging another pull
/// request. A request it filed in the last hour must keep its tab
/// through `hub clean`, or the user approves a request whose session
/// is gone.
#[test]
fn run_clean_keeps_worker_with_recent_request() {
    let mut test_tools = TestTools::new();
    add_finished_worker(
        &mut test_tools,
        "demo-kb-001-impl",
        "DEMO-KB-001",
        Stage::Accepted,
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
        Stage::Review,
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
            .contains("- 2214 parked demo-kb-007-impl (DEMO-KB-007 review)\n")
    );
    assert_eq!(
        read_events(&test_tools.run_path()).last().unwrap()["event"],
        "parked"
    );
}

/// `hub park` on a worker still in its turn must refuse and change
/// nothing. A park there closes the tab and cuts the turn off.
#[test]
fn run_park_refuses_session_still_working() {
    let mut test_tools = TestTools::new();
    add_finished_worker(
        &mut test_tools,
        "demo-kb-007-impl",
        "DEMO-KB-007",
        Stage::Review,
    );
    test_tools.herdr.agents[0].status = AgentStatus::Working;

    let error = park(&test_tools, "demo-kb-007-impl").unwrap_err();

    assert_eq!(error.to_string(), "demo-kb-007-impl stays: working");
    assert_eq!(read_parked_record(&test_tools, "demo-kb-007-impl"), None);
    assert!(test_tools.herdr.closed_tabs.borrow().is_empty());
}

/// A `card link` failure after the record is written must leave both
/// the record and the open tab. Closing the tab first, or deleting the
/// record, loses the session ID of work that waits for review.
#[test]
fn run_park_failure_keeps_record_and_tab() {
    let mut test_tools = TestTools::new();
    add_finished_worker(
        &mut test_tools,
        "demo-kb-007-impl",
        "DEMO-KB-007",
        Stage::Review,
    );
    test_tools.cards.link_error = Some("card: vault is read-only".to_owned());

    let error = park(&test_tools, "demo-kb-007-impl").unwrap_err();

    assert!(matches!(error, HubError::Card(_)), "{error:?}");
    assert!(read_parked_record(&test_tools, "demo-kb-007-impl").is_some());
    assert!(test_tools.herdr.closed_tabs.borrow().is_empty());
}

/// `hub park` closes the session's whole tab. Another agent or a busy
/// shell in that tab must keep it open, as in the tidy pass, or the
/// hub parking one worker kills the other's turn.
#[test]
fn run_park_keeps_tab_with_other_pane() {
    let mut test_tools = TestTools::new();
    add_finished_worker(
        &mut test_tools,
        "demo-kb-007-impl",
        "DEMO-KB-007",
        Stage::Review,
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

/// A message can come in while `card link` runs on the network drive.
/// The park must check again after the link and keep the tab, or the
/// message waits for a closed session. The record stays for the next
/// pass, which deletes it while the session is live. The card's Log
/// must not say the session parked.
#[test]
fn run_park_keeps_tab_messaged_during_link() {
    let mut test_tools = TestTools::new();
    add_finished_worker(
        &mut test_tools,
        "demo-kb-007-impl",
        "DEMO-KB-007",
        Stage::Review,
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
