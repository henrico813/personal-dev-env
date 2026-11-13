mod common;

use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use hub::commands::at::{AtRequest, Recipients, run_at};
use hub::commands::next::{NextOptions, run_next};
use hub::external::herdr::AgentStatus;
use hub::shared::HubError;

use common::{TestTools, make_agent, make_time};

/// Noon on a day long past, so a message for 20:00 that day is due.
const PAST_NOON: &str = "2020-01-01T12:00:00+00:00[UTC]";

/// Noon on a day far ahead, so a message for 20:00 that day waits.
const FUTURE_NOON: &str = "2099-01-01T12:00:00+00:00[UTC]";

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

/// Stores `text` from `/tmp/resume.md` for 20:00 on the day of `now`.
fn store_for_eight_pm(test_tools: &TestTools, now: &str, to: Recipients, text: &str) {
    let request = AtRequest {
        time: "20:00".parse().unwrap(),
        to,
        file: PathBuf::from("/tmp/resume.md"),
        text: text.to_owned(),
    };
    run_at(&test_tools.run(), request, &make_time(now)).unwrap();
}

/// Names one session, as `--to docs-fix` does.
fn to_docs_fix() -> Recipients {
    Recipients::Names(vec!["docs-fix".to_owned()])
}

/// `run_at` must refuse a time already past today and store nothing.
/// Taken as tomorrow, a message meant for 20:00 would go out a day
/// late.
#[test]
fn run_at_refuses_time_already_past() {
    let test_tools = TestTools::new();
    let request = AtRequest {
        time: "20:00".parse().unwrap(),
        to: to_docs_fix(),
        file: PathBuf::from("/tmp/resume.md"),
        text: "Resume work.".to_owned(),
    };
    let now = make_time("2026-11-11T22:14:00+00:00[UTC]");

    let result = run_at(&test_tools.run(), request, &now);

    assert!(matches!(result, Err(HubError::Usage(_))));
    assert!(!test_tools.run_path().join("timed").exists());
}

/// A message must go out once its time comes, and only then. Sent
/// early, it ends a rule before its time. Sent on every check, it
/// interrupts the session every 5 seconds. The hub must see the timed
/// line once, in the check that sent it, or it never learns the
/// message went out.
#[test]
fn run_next_sends_and_announces_due_message() {
    let mut test_tools = TestTools::new();
    test_tools.herdr.agents = vec![make_agent("docs-fix", AgentStatus::Idle)];
    store_for_eight_pm(&test_tools, PAST_NOON, to_docs_fix(), "Resume work.");
    store_for_eight_pm(
        &test_tools,
        FUTURE_NOON,
        to_docs_fix(),
        "Stop for the night.",
    );

    let first = check_once(&test_tools);
    let second = check_once(&test_tools);

    assert_eq!(first, "timed message to 1 session: /tmp/resume.md (hub -)");
    assert_eq!(second, "timeout (hub -)");
    assert_eq!(
        *test_tools.herdr.prompts.borrow(),
        [("docs-fix".to_owned(), "Resume work.".to_owned())]
    );
}

/// `--all` must reach only workers, a session on a card or one that
/// has reported. A hub may act on a worker's message, such as one to
/// stop for the night, and stop its own sessions. The user's own chat
/// would get an order meant for workers.
#[test]
fn run_next_limits_timed_all_to_workers() {
    let mut test_tools = TestTools::new();
    test_tools.herdr.agents = vec![
        make_agent("kanban-hub-3", AgentStatus::Idle),
        make_agent("demo-kb-007-impl", AgentStatus::Idle),
        make_agent("docs-fix", AgentStatus::Idle),
        make_agent("notes-chat", AgentStatus::Idle),
    ];
    // A hub can report to the hub that started it, so a report alone
    // does not make a worker.
    for name in ["kanban-hub-3", "docs-fix"] {
        let read_report = test_tools
            .run_path()
            .join(format!("report-{name}.1200.read"));
        fs::write(read_report, "Plan ready.\n").unwrap();
    }
    store_for_eight_pm(
        &test_tools,
        PAST_NOON,
        Recipients::AllWorkers,
        "Resume work.",
    );

    check_once(&test_tools);

    assert_eq!(
        *test_tools.herdr.prompts.borrow(),
        [
            ("demo-kb-007-impl".to_owned(), "Resume work.".to_owned()),
            ("docs-fix".to_owned(), "Resume work.".to_owned()),
        ]
    );
}

/// A send problem wins the line of the check that held the message.
/// The timed line must still print once in a later check, or the hub
/// never learns that the message went out.
#[test]
fn run_next_prints_timed_line_after_failure() {
    let mut test_tools = TestTools::new();
    test_tools.herdr.agents = vec![make_agent("docs-fix", AgentStatus::Idle)];
    let to = Recipients::Names(vec!["docs-fix".to_owned(), "gone-fix".to_owned()]);
    store_for_eight_pm(&test_tools, PAST_NOON, to, "Resume work.");

    let first = check_once(&test_tools);
    let second = check_once(&test_tools);
    let third = check_once(&test_tools);

    assert_eq!(first, "not sent gone-fix: the session is gone (hub -)");
    assert_eq!(
        second,
        "timed message to 2 sessions: /tmp/resume.md (hub -)"
    );
    assert_eq!(third, "timeout (hub -)");
}
