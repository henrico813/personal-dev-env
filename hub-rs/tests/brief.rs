mod common;

use std::fs;

use jiff::Zoned;

use hub::commands::brief::run_brief;
use hub::external::cards::BoardCounts;
use hub::shared::HubError;

use common::TestTools;

/// A whole `decisions.md` entry, as the hub writes one.
const DECISION: &str = "## New model overnight\n\
                        \n\
                        Deploy the new model tonight so you can test it in the morning?\n\
                        \n\
                        - A. Deploy from the branch, with the undo path ready.\n\
                        - B. Wait until you return.\n\
                        \n\
                        Recommended: A, since you want to test it in the morning.\n\
                        If no reply: A.\n";

/// The time each test runs `hub brief` at, before the `20:00` put-off
/// time.
fn fixed_now() -> Zoned {
    "2026-11-11T13:00[UTC]".parse().unwrap()
}

/// The user decides from this one message, often away from the
/// desk. Hubs once named questions by number alone and the user
/// had to ask what they were, so every line of each entry prints,
/// after the board line and the pull requests.
#[test]
fn run_brief_prints_board_reviews_then_decisions() {
    let mut test_tools = TestTools::new();
    test_tools.cards.board = Some(BoardCounts {
        active: 5,
        review: 19,
        busy: 24,
        limit: 8,
    });
    fs::write(
        test_tools.run_path().join("reviews.md"),
        "# PRs\n\n| #1 | Fix |\n",
    )
    .unwrap();
    fs::write(test_tools.run_path().join("decisions.md"), DECISION).unwrap();

    let outcome = run_brief(&test_tools.tools(), &test_tools.run(), &fixed_now()).unwrap();

    assert_eq!(
        outcome.lines,
        [
            "board 24/8, review 19; 1 decision",
            "",
            "# PRs",
            "",
            "| #1 | Fix |",
            "",
            "Decision: New model overnight",
            "Deploy the new model tonight so you can test it in the morning?",
            "- A. Deploy from the branch, with the undo path ready.",
            "- B. Wait until you return.",
            "Recommended: A, since you want to test it in the morning.",
            "If no reply: A.",
        ]
    );
}

/// The Python hub printed `Nothing waits on the user.` when nothing
/// waits, and the user reads it as the all-clear. Without it the
/// brief is a bare board line that looks cut off.
#[test]
fn run_brief_says_when_nothing_waits() {
    let mut test_tools = TestTools::new();
    test_tools.cards.board = Some(BoardCounts {
        active: 3,
        review: 0,
        busy: 3,
        limit: 8,
    });

    let outcome = run_brief(&test_tools.tools(), &test_tools.run(), &fixed_now()).unwrap();

    assert_eq!(
        outcome.lines,
        [
            "board 3/8, review 0; 0 decisions",
            "",
            "Nothing waits on the user.",
        ]
    );
}

/// An entry without a recommendation or a no-reply default cannot
/// go to a user who decides from the message alone. The command
/// names each missing part, so the hub can fix the entry.
#[test]
fn run_brief_stops_on_entry_missing_parts() {
    let test_tools = TestTools::new();
    fs::write(
        test_tools.run_path().join("decisions.md"),
        "## Pause cards\n\nPause stale cards?\n\n- A. Yes\n- B. No\n",
    )
    .unwrap();

    let error = run_brief(&test_tools.tools(), &test_tools.run(), &fixed_now()).unwrap_err();

    let HubError::NotDone(message) = error else {
        panic!("expected NotDone, got {error:?}");
    };
    assert_eq!(
        message,
        "decisions.md entry 'Pause cards' lacks the Recommended: line, \
         the If no reply: line"
    );
}

/// The user answers questions from a phone, and a long list buries the
/// ones that matter. With more than 5 waiting now, the hub must hold
/// new ones, so `hub brief` stops. A question put off until later does
/// not count, or putting one off would not free a place.
#[test]
fn run_brief_limits_questions_waiting_now() {
    let test_tools = TestTools::new();
    let decisions_path = test_tools.run_path().join("decisions.md");
    let six: Vec<String> = (1..=6)
        .map(|number| DECISION.replace("New model overnight", &format!("Question {number}")))
        .collect();
    fs::write(&decisions_path, six.concat()).unwrap();

    let error = run_brief(&test_tools.tools(), &test_tools.run(), &fixed_now()).unwrap_err();
    fs::write(
        &decisions_path,
        six.concat() + "Not before: 2026-11-11 20:00\n",
    )
    .unwrap();
    let put_off = run_brief(&test_tools.tools(), &test_tools.run(), &fixed_now());

    let HubError::NotDone(message) = error else {
        panic!("expected NotDone, got {error:?}");
    };
    assert_eq!(
        message,
        "6 decisions.md entries wait on the user now, over the limit of 5; \
         hold new questions until the user answers one"
    );
    assert!(put_off.is_ok(), "{put_off:?}");
}

/// The hub puts a question off with a `Not before:` time. A time the
/// board cannot read, such as a bare `22:30.`, would show the question
/// at once and break the promise to wait. `hub brief` names the entry
/// so the hub fixes the line.
#[test]
fn run_brief_stops_on_unreadable_not_before() {
    let test_tools = TestTools::new();
    fs::write(
        test_tools.run_path().join("decisions.md"),
        format!("{DECISION}Not before: 22:30.\n"),
    )
    .unwrap();

    let error = run_brief(&test_tools.tools(), &test_tools.run(), &fixed_now()).unwrap_err();

    let HubError::NotDone(message) = error else {
        panic!("expected NotDone, got {error:?}");
    };
    assert_eq!(
        message,
        "decisions.md entry 'New model overnight' has Not before: 22:30., \
         not a time such as 2026-11-11 20:00"
    );
}
