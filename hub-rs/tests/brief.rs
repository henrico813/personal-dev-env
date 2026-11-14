mod common;

use std::fs;

use hub::commands::brief::run_brief;
use hub::external::cards::{BoardCounts, WaitingCard};
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
        active_limit: 8,
        review_limit: 6,
    });
    fs::write(
        test_tools.run_path().join("reviews.md"),
        "# PRs\n\n| #1 | Fix |\n",
    )
    .unwrap();
    fs::write(test_tools.run_path().join("decisions.md"), DECISION).unwrap();

    let outcome = run_brief(&test_tools.tools(), &test_tools.run()).unwrap();

    assert_eq!(
        outcome.lines,
        [
            "board active 5 of 8, review 19 of 6; 1 decision",
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
        active_limit: 8,
        review_limit: 6,
    });

    let outcome = run_brief(&test_tools.tools(), &test_tools.run()).unwrap();

    assert_eq!(
        outcome.lines,
        [
            "board active 3 of 8, review 0 of 6; 0 decisions",
            "",
            "Nothing waits on the user.",
        ]
    );
}

/// Cards that wait on another team are news the user may chase in a
/// standup, never a decision. If they read as waiting on the user, the
/// all-clear would vanish and the user would hunt for a step that is
/// not theirs.
#[test]
fn run_brief_lists_waiting_cards_as_information_only() {
    let mut test_tools = TestTools::new();
    test_tools.cards.board = Some(BoardCounts {
        active: 3,
        review: 0,
        active_limit: 8,
        review_limit: 6,
    });
    test_tools.cards.waiting = vec![
        WaitingCard {
            key: "DEMO-KB-003".parse().unwrap(),
            days: 8,
            title: "Retry failed uploads".to_owned(),
            reason: "platform team's review".to_owned(),
        },
        WaitingCard {
            key: "DEMO-KB-009".parse().unwrap(),
            days: 1,
            title: "Add a cache".to_owned(),
            reason: String::new(),
        },
    ];

    let outcome = run_brief(&test_tools.tools(), &test_tools.run()).unwrap();

    assert_eq!(
        outcome.lines,
        [
            "board active 3 of 8, review 0 of 6; 0 decisions",
            "waiting-on-others 2, oldest 8 days",
            "",
            "Nothing waits on the user.",
            "",
            "Waiting on others (no action needed)",
            "- DEMO-KB-003, 8 days",
            "  Retry failed uploads",
            "  platform team's review",
            "- DEMO-KB-009, 1 day",
            "  Add a cache",
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

    let error = run_brief(&test_tools.tools(), &test_tools.run()).unwrap_err();

    let HubError::NotDone(message) = error else {
        panic!("expected NotDone, got {error:?}");
    };
    assert_eq!(
        message,
        "decisions.md entry 'Pause cards' lacks the Recommended: line, \
         the If no reply: line"
    );
}
