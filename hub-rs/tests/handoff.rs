mod common;

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use hub::commands::handoff::{run_adopt, run_handoff};
use hub::external::herdr::{Agent, AgentStatus, HerdrCode};
use hub::shared::{AgentName, HubError};
use serde_json::json;

use common::{
    OpenCodeRow, TestTools, make_agent, make_refusal, make_time, make_workspaces, read_events,
    write_opencode_db,
};

/// The model the newest `rules.md` line names.
const MODEL: &str = "claude-code/claude-opus-5-5[1m]#medium";

/// A hub `kanban-hub` in pane `w1:kanban-hub` and tab `w1:t1` with
/// session `ses_old`, the next hub `kanban-hub-2` with session
/// `ses_new`, a `rules.md` whose last model is `MODEL`, and the next
/// hub's handoff file, whose path it returns.
fn make_handoff_tools() -> (TestTools, PathBuf) {
    let mut test_tools = TestTools::new();
    test_tools.herdr.agents = vec![
        Agent {
            tab_id: "w1:t1".to_owned(),
            session_id: Some("ses_old".to_owned()),
            ..make_agent("kanban-hub", AgentStatus::Working)
        },
        Agent {
            session_id: Some("ses_new".to_owned()),
            ..make_agent("kanban-hub-2", AgentStatus::Working)
        },
    ];
    test_tools.own_pane = Some("w1:kanban-hub".to_owned());
    fs::write(
        test_tools.run_path().join("rules.md"),
        format!(
            "- old: start hubs with --model 'claude-code/old'.\n\
             - new: start hubs with `--model '{MODEL}'`.\n"
        ),
    )
    .unwrap();
    let handoff = test_tools.path().join("hub-2.md");
    fs::write(&handoff, "# Handoff: run the hub\n").unwrap();
    (test_tools, handoff)
}

/// An OpenCode database where `ses_new`'s first reply ran at `effort`
/// and a later one at `xhigh`, as when the user switches effort later.
fn write_first_reply(test_tools: &TestTools, effort: &str) {
    write_opencode_db(
        &test_tools.opencode_path(),
        &[
            OpenCodeRow {
                session: "ses_new",
                kind: "assistant",
                updated_ms: 1,
                data: json!({"model": {"id": "claude-opus-5-5[1m]", "variant": effort}}),
            },
            OpenCodeRow {
                session: "ses_new",
                kind: "assistant",
                updated_ms: 2,
                data: json!({"model": {"id": "claude-opus-5-5[1m]", "variant": "xhigh"}}),
            },
        ],
    );
}

/// Runs `hub handoff kanban-hub-2 HANDOFF` without waiting between
/// checks.
fn run_next_hub(test_tools: &TestTools, handoff: &Path) -> Result<Vec<String>, HubError> {
    let name: AgentName = "kanban-hub-2".parse().unwrap();
    run_handoff(
        &test_tools.tools(),
        &test_tools.run(),
        &name,
        handoff,
        test_tools.path(),
        &make_time("2026-11-11T22:14:00+00:00[UTC]"),
        Duration::ZERO,
    )
    .map(|outcome| outcome.lines)
}

/// Runs `hub adopt OLD`.
fn adopt_hub(test_tools: &TestTools, old: &str) -> Result<Vec<String>, HubError> {
    run_adopt(
        &test_tools.tools(),
        &test_tools.run(),
        &old.parse().unwrap(),
        &make_time("2026-11-11T22:14:00+00:00[UTC]"),
    )
    .map(|outcome| outcome.lines)
}

/// A hub once told the user its next hub ran at medium effort when it
/// ran at the default, because nothing passed or checked the effort.
/// The next hub starts on the newest rules model, and the printed
/// effort is the first reply's, not a later switch.
#[test]
fn run_handoff_reports_effort_of_rules_model() {
    let (test_tools, handoff) = make_handoff_tools();
    write_first_reply(&test_tools, "medium");

    let lines = run_next_hub(&test_tools, &handoff).unwrap();

    assert_eq!(
        lines[0],
        format!("started kanban-hub-2 on {MODEL}; its first reply ran at medium")
    );
    let started = test_tools.herdr.started.borrow();
    assert_eq!(started.len(), 1);
    assert_eq!(started[0].args, ["--model", MODEL, "--auto"]);
}

/// The next hub runs `hub` commands without `--run`, which works only
/// when its pane has `HUB_RUN`. The agent must start in the new tab,
/// in the old hub's workspace.
#[test]
fn run_handoff_gives_new_tab_run_folder() {
    let (test_tools, handoff) = make_handoff_tools();
    write_first_reply(&test_tools, "medium");

    run_next_hub(&test_tools, &handoff).unwrap();

    let created_tabs = test_tools.herdr.created_tabs.borrow();
    let (workspace_id, place) = &created_tabs[0];
    assert_eq!(workspace_id, "w1");
    assert_eq!(place.label, "kanban-hub-2");
    let run_path = test_tools.run().path().display().to_string();
    assert_eq!(place.env, [("HUB_RUN".to_owned(), run_path)]);
    assert_eq!(
        test_tools.herdr.started.borrow()[0].pane_id,
        "w1:p-kanban-hub-2"
    );
}

/// Every hub that hands off gets a session review handoff, used when
/// reviews are on. The reviewer finds the old hub by its session ID,
/// and the next hub learns from its own handoff, the one its only
/// prompt names, which reviewer starts and from which file.
#[test]
fn run_handoff_tells_next_hub_review_steps() {
    let (test_tools, handoff) = make_handoff_tools();
    write_first_reply(&test_tools, "medium");

    run_next_hub(&test_tools, &handoff).unwrap();

    let reviews: Vec<PathBuf> = fs::read_dir(test_tools.state_folder().join("handoffs"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    assert_eq!(reviews.len(), 1);
    assert!(
        reviews[0]
            .to_string_lossy()
            .ends_with("20261111-kanban-hub-review.md")
    );
    assert!(
        fs::read_to_string(&reviews[0])
            .unwrap()
            .contains("--session ses_old")
    );
    let next_handoff = fs::read_to_string(&handoff).unwrap();
    assert!(next_handoff.starts_with("# Handoff: run the hub\n"));
    assert!(next_handoff.contains("review-kanban-hub"));
    assert!(next_handoff.contains(&reviews[0].display().to_string()));
    let prompts = test_tools.herdr.prompts.borrow();
    assert_eq!(prompts.len(), 1);
    assert!(prompts[0].1.contains(&handoff.display().to_string()));
}

/// Appending to a missing handoff would create it, and the next hub
/// would start with none of the run's state. A run folder without
/// `rules.md` names no model to start the next hub on. Stopping before
/// any Herdr change leaves no tab to clean up.
#[test]
fn run_handoff_stops_before_starting_without_input() {
    let cases = ["handoff", "rules", "own-pane"];

    for case_name in cases {
        let (mut test_tools, handoff) = make_handoff_tools();
        match case_name {
            "handoff" => fs::remove_file(&handoff).unwrap(),
            "rules" => fs::remove_file(test_tools.run_path().join("rules.md")).unwrap(),
            _ => test_tools.own_pane = None,
        }

        let error = run_next_hub(&test_tools, &handoff).unwrap_err();

        assert!(
            matches!(error, HubError::NotDone(_)),
            "{case_name}: {error:?}"
        );
        assert!(
            test_tools.herdr.created_tabs.borrow().is_empty(),
            "{case_name}"
        );
    }
}

/// Without the word "started", the hub would read a plain Herdr error
/// and run the command again, which opens a second hub and adds the
/// old hub's steps to the handoff twice.
#[test]
fn run_handoff_says_started_when_prompt_fails() {
    let (test_tools, handoff) = make_handoff_tools();
    test_tools
        .herdr
        .fail_next("prompt", make_refusal(HerdrCode::Other(String::new())));

    let error = run_next_hub(&test_tools, &handoff).unwrap_err();

    let HubError::NotDone(message) = error else {
        panic!("expected NotDone, got {error:?}");
    };
    assert_eq!(message, "started kanban-hub-2, but Herdr said no");
}

/// This check would have caught the wrong claim. The hub must not
/// report a handoff it did not check, and the message names both
/// efforts for the user.
#[test]
fn run_handoff_stops_when_reply_effort_differs() {
    let (test_tools, handoff) = make_handoff_tools();
    write_first_reply(&test_tools, "default");

    let error = run_next_hub(&test_tools, &handoff).unwrap_err();

    let HubError::NotDone(message) = error else {
        panic!("expected NotDone, got {error:?}");
    };
    assert_eq!(
        message,
        "started kanban-hub-2, but it runs at default effort, not medium"
    );
}

/// The tidy pass closes the old hub's tab from the `handoff` event,
/// and `hub next` then starts its review from the event's review file.
/// The event is written once the new tab exists, even when the effort
/// check then fails, so the old hub never stays open forever.
#[test]
fn run_handoff_records_old_hub_and_review() {
    let (test_tools, handoff) = make_handoff_tools();
    write_first_reply(&test_tools, "default");

    run_next_hub(&test_tools, &handoff).unwrap_err();

    let events = read_events(&test_tools.run_path());
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["event"], "handoff");
    assert_eq!(events[0]["old"], "kanban-hub");
    assert_eq!(events[0]["tab"], "w1:t1");
    assert_eq!(events[0]["new"], "kanban-hub-2");
    assert!(
        events[0]["review"]
            .as_str()
            .unwrap()
            .ends_with("20261111-kanban-hub-review.md")
    );
}

/// Two hubs once started without anyone running `hub gather`, and the
/// run's workspace sat at the bottom of the sidebar. A handoff puts the
/// hubs' workspace above the workers' and tags the next hub for the
/// "go to" list, with no extra line when that works.
#[test]
fn run_handoff_gathers_hubs_silently() {
    let (mut test_tools, handoff) = make_handoff_tools();
    test_tools.herdr.workspaces = make_workspaces(&["w2", "w1"]);
    write_first_reply(&test_tools, "medium");

    let lines = run_next_hub(&test_tools, &handoff).unwrap();

    assert_eq!(
        *test_tools.herdr.moves.borrow(),
        [(vec!["w1".to_owned()], "w2".to_owned())]
    );
    let tagged_hubs = test_tools.herdr.tagged_hubs.borrow();
    assert!(
        tagged_hubs
            .iter()
            .any(|(pane_id, _)| pane_id == "w1:kanban-hub-2")
    );
    assert_eq!(test_tools.herdr.agent_filters.get(), 1);
    assert_eq!(lines.len(), 2);
}

/// A handoff that reported a sidebar problem as a failure would make
/// the hub run it again and start a second hub. The handoff still
/// reports success, with one line that names the problem.
#[test]
fn run_handoff_warns_when_gather_fails() {
    let (mut test_tools, handoff) = make_handoff_tools();
    test_tools.herdr.workspaces = make_workspaces(&["w2", "w1"]);
    test_tools.herdr.fail_next(
        "move_workspaces",
        make_refusal(HerdrCode::Other(String::new())),
    );
    write_first_reply(&test_tools, "medium");

    let lines = run_next_hub(&test_tools, &handoff).unwrap();

    assert!(lines[0].starts_with("started kanban-hub-2 on "));
    assert_eq!(
        lines[2],
        "could not put the hubs on top: Herdr said no; hub gather tries again"
    );
    assert_eq!(lines.len(), 3);
}

/// A hub started without `hub handoff` leaves the older hub open with
/// no review. Adopting it records the same `handoff` event a handoff
/// writes, so the tidy pass closes the old hub and
/// `hub next --start-reviews` starts its review, which finds the old hub by its session ID.
#[test]
fn run_adopt_records_handoff_with_review() {
    let mut test_tools = TestTools::new();
    test_tools.herdr.agents = vec![
        make_agent("kanban-hub-4", AgentStatus::Working),
        Agent {
            session_id: Some("ses_old".to_owned()),
            ..make_agent("kanban-hub-3", AgentStatus::Working)
        },
    ];
    test_tools.own_pane = Some("w1:kanban-hub-4".to_owned());

    adopt_hub(&test_tools, "kanban-hub-3").unwrap();

    let events = read_events(&test_tools.run_path());
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["event"], "handoff");
    assert_eq!(events[0]["old"], "kanban-hub-3");
    assert_eq!(events[0]["tab"], "w1:t-kanban-hub-3");
    assert_eq!(events[0]["new"], "kanban-hub-4");
    let review = events[0]["review"].as_str().unwrap();
    assert!(review.ends_with("20261111-kanban-hub-3-review.md"));
    assert!(
        fs::read_to_string(review)
            .unwrap()
            .contains("--session ses_old")
    );
}

/// Adopting the newer hub would make the tidy pass close the hub that
/// runs the board. The command must refuse and record nothing.
#[test]
fn run_adopt_refuses_newer_hub() {
    let mut test_tools = TestTools::new();
    test_tools.herdr.agents = vec![
        make_agent("kanban-hub-3", AgentStatus::Working),
        Agent {
            session_id: Some("ses_new".to_owned()),
            ..make_agent("kanban-hub-4", AgentStatus::Working)
        },
    ];
    test_tools.own_pane = Some("w1:kanban-hub-3".to_owned());

    let error = adopt_hub(&test_tools, "kanban-hub-4").unwrap_err();

    assert!(matches!(error, HubError::NotDone(_)), "{error:?}");
    assert!(read_events(&test_tools.run_path()).is_empty());
}

/// A hub `kanban-hub-4` in its own pane that can adopt `kanban-hub-3`,
/// both in workspace `w1`, below a worker's workspace `w2`.
fn make_adopt_tools() -> TestTools {
    let mut test_tools = TestTools::new();
    test_tools.herdr.agents = vec![
        make_agent("kanban-hub-4", AgentStatus::Working),
        Agent {
            session_id: Some("ses_old".to_owned()),
            ..make_agent("kanban-hub-3", AgentStatus::Working)
        },
    ];
    test_tools.herdr.workspaces = make_workspaces(&["w2", "w1"]);
    test_tools.own_pane = Some("w1:kanban-hub-4".to_owned());
    test_tools
}

/// A hub the user started by hand runs `hub adopt` first. Adopting
/// puts the hubs' workspace above the workers', so that hub's sidebar
/// is right without anyone running `hub gather`.
#[test]
fn run_adopt_gathers_hubs_silently() {
    let test_tools = make_adopt_tools();

    let lines = adopt_hub(&test_tools, "kanban-hub-3").unwrap();

    assert_eq!(
        *test_tools.herdr.moves.borrow(),
        [(vec!["w1".to_owned()], "w2".to_owned())]
    );
    assert_eq!(lines.len(), 1);
}

/// The adoption is recorded before the gather runs, so a sidebar
/// problem must not read as a failed adoption that the hub retries.
#[test]
fn run_adopt_warns_when_gather_fails() {
    let test_tools = make_adopt_tools();
    test_tools.herdr.fail_next(
        "move_workspaces",
        make_refusal(HerdrCode::Other(String::new())),
    );

    let lines = adopt_hub(&test_tools, "kanban-hub-3").unwrap();

    assert!(lines[0].starts_with("adopted kanban-hub-3; "));
    assert_eq!(
        lines[1],
        "could not put the hubs on top: Herdr said no; hub gather tries again"
    );
    assert_eq!(read_events(&test_tools.run_path()).len(), 1);
}
