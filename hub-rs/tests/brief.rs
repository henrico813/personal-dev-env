mod common;

use std::fs;

use hub::commands::brief::run_brief;
use hub::external::cards::{BlockedCard, BoardCounts, CardNote};
use hub::external::herdr::AgentStatus;
use jiff::tz::TimeZone;
use jiff::{SignedDuration, Timestamp};
use serde_json::json;

use common::{OpenCodeRow, TestTools, make_agent, write_opencode_db};

/// The user decides from this one message, often away from the
/// desk. Hubs once named questions by number alone and the user
/// had to ask what they were, so every line of each card's open
/// question prints, after the board line and the pull requests.
/// `hub ask` adds its item last, so an older item with the same title
/// must not hide the question's lines.
#[test]
fn run_brief_prints_board_reviews_then_questions() {
    let mut test_tools = TestTools::new();
    test_tools.cards.board = Some(BoardCounts {
        active: 5,
        in_review: 19,
        active_limit: 8,
        in_review_limit: 6,
    });
    test_tools.cards.card_notes.insert(
        "DEMO-KB-007".to_owned(),
        CardNote {
            project: "Demo".to_owned(),
            needs_you: Some("New model overnight".to_owned()),
            open_questions: vec![
                vec!["New model overnight".to_owned()],
                vec![
                    "New model overnight".to_owned(),
                    "Deploy the new model tonight?".to_owned(),
                    "- A. Deploy from the branch.".to_owned(),
                    "- B. Wait until you return.".to_owned(),
                    "Recommended: A, since you test in the morning.".to_owned(),
                    "If no reply: B.".to_owned(),
                ],
            ],
            ..CardNote::default()
        },
    );
    fs::write(
        test_tools.run_path().join("reviews.md"),
        "# PRs\n\n| #1 | Fix |\n",
    )
    .unwrap();

    let outcome = run_brief(&test_tools.tools(), &test_tools.run(), Timestamp::now()).unwrap();

    assert_eq!(
        outcome.lines,
        [
            "board active 5 of 8, in-review 19 of 6; 1 question",
            "",
            "# PRs",
            "",
            "| #1 | Fix |",
            "",
            "Question DEMO-KB-007: New model overnight",
            "Deploy the new model tonight?",
            "- A. Deploy from the branch.",
            "- B. Wait until you return.",
            "Recommended: A, since you test in the morning.",
            "If no reply: B.",
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
        in_review: 0,
        active_limit: 8,
        in_review_limit: 6,
    });

    let outcome = run_brief(&test_tools.tools(), &test_tools.run(), Timestamp::now()).unwrap();

    assert_eq!(
        outcome.lines,
        [
            "board active 3 of 8, in-review 0 of 6; 0 questions",
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
fn run_brief_shows_blocked_cards_without_action() {
    let mut test_tools = TestTools::new();
    test_tools.cards.board = Some(BoardCounts {
        active: 3,
        in_review: 0,
        active_limit: 8,
        in_review_limit: 6,
    });
    test_tools.cards.blocked = vec![
        BlockedCard {
            key: "DEMO-KB-003".parse().unwrap(),
            days: 8,
            title: "Retry failed uploads".to_owned(),
            reason: "platform team's review".to_owned(),
        },
        BlockedCard {
            key: "DEMO-KB-009".parse().unwrap(),
            days: 1,
            title: "Add a cache".to_owned(),
            reason: String::new(),
        },
    ];

    let outcome = run_brief(&test_tools.tools(), &test_tools.run(), Timestamp::now()).unwrap();

    assert_eq!(
        outcome.lines,
        [
            "board active 3 of 8, in-review 0 of 6; 0 questions",
            "blocked 2, oldest 8 days",
            "",
            "Nothing waits on the user.",
            "",
            "Blocked (no action needed)",
            "- DEMO-KB-003, 8 days",
            "  Retry failed uploads",
            "  platform team's review",
            "- DEMO-KB-009, 1 day",
            "  Add a cache",
        ]
    );
}

/// The user reads `Nothing waits on the user.` as the all-clear. When
/// `card` cannot list the open questions, one may still wait, so the
/// brief names the failure and never gives the all-clear.
#[test]
fn run_brief_reports_failed_question_read() {
    let mut test_tools = TestTools::new();
    test_tools.cards.board = Some(BoardCounts {
        active: 3,
        in_review: 0,
        active_limit: 8,
        in_review_limit: 6,
    });
    test_tools.cards.read_error = Some("card: grep failed".to_owned());

    let outcome = run_brief(&test_tools.tools(), &test_tools.run(), Timestamp::now()).unwrap();

    assert_eq!(
        outcome.lines,
        [
            "board active 3 of 8, in-review 0 of 6; questions unread",
            "",
            "questions: card: grep failed",
        ]
    );
}

/// The search finds a marked card that `card show` then cannot read.
/// Its question still waits, so skipping the card would give the user
/// a false all-clear; the brief names the failure instead.
#[test]
fn run_brief_reports_unreadable_marked_card() {
    let mut test_tools = TestTools::new();
    test_tools.cards.board = Some(BoardCounts {
        active: 3,
        in_review: 0,
        active_limit: 8,
        in_review_limit: 6,
    });
    test_tools.cards.unreadable_needs_you = vec!["DEMO-KB-008".to_owned()];

    let outcome = run_brief(&test_tools.tools(), &test_tools.run(), Timestamp::now()).unwrap();

    assert_eq!(
        outcome.lines,
        [
            "board active 3 of 8, in-review 0 of 6; questions unread",
            "",
            "questions: card: no card DEMO-KB-008",
        ]
    );
}

/// Hubs told the user an approval waited after it had run or expired,
/// because they repeated an old line. The brief reads each request
/// file fresh, so it names only pending requests with the time left,
/// and drops a pending request past pde-gh-write's four-hour expiry.
#[test]
fn run_brief_lists_only_live_pending_approvals() {
    let mut test_tools = TestTools::new();
    test_tools.cards.board = Some(BoardCounts {
        active: 3,
        in_review: 0,
        active_limit: 8,
        in_review_limit: 6,
    });
    let requests = test_tools.state_folder().join("pr-write");
    fs::create_dir(&requests).unwrap();
    let now: Timestamp = "2026-11-16T10:00:00Z".parse().unwrap();
    let write_request = |id: &str, state: &str, changed_age: SignedDuration, args: &[&str]| {
        let request = json!({
            "id": id,
            "state": state,
            "repo": "demo/app",
            "args": args,
            "updated": (now - changed_age).to_string(),
        });
        fs::write(requests.join(format!("{id}.json")), request.to_string()).unwrap();
    };
    write_request(
        "0016cdaa",
        "pending",
        SignedDuration::from_mins(168),
        &["pr", "merge", "515"],
    );
    write_request(
        "0096efbb",
        "pending",
        SignedDuration::from_hours(5),
        &["pr", "create"],
    );
    write_request(
        "00c8be58",
        "ran",
        SignedDuration::from_mins(10),
        &["pr", "merge", "20"],
    );

    let outcome = run_brief(&test_tools.tools(), &test_tools.run(), now).unwrap();

    assert_eq!(
        outcome.lines,
        [
            "board active 3 of 8, in-review 0 of 6; 0 questions",
            "",
            "approval A0016cd 1h12m left app #515 merge",
        ]
    );
}

/// The user often answers a question in the worker's own pane, and a
/// hub that asks it again makes them repeat themselves. So a question
/// whose card session got a message the user typed since the day it
/// was asked carries a `Check first:` line naming that session. The
/// session's start prompt, hub messages, in their older `From the
/// hub` form too, and watch prods are not the user's, and neither is
/// anything from before the ask.
#[test]
fn run_brief_flags_sessions_user_typed_in() {
    let mut test_tools = TestTools::new();
    let asked_on = "2026-11-21".parse().unwrap();
    let vault = CardNote {
        project: "Demo".to_owned(),
        needs_you: Some("Keep the main vault in nobs?".to_owned()),
        needs_you_date: Some(asked_on),
        sessions: vec!["ses_old".to_owned(), "ses_new".to_owned()],
        ..CardNote::default()
    };
    let patch = CardNote {
        project: "Demo".to_owned(),
        needs_you: Some("Commit the fix after the merge?".to_owned()),
        needs_you_date: Some(asked_on),
        sessions: vec!["ses_fix".to_owned()],
        ..CardNote::default()
    };
    test_tools
        .cards
        .card_notes
        .insert("DEMO-320".to_owned(), vault);
    test_tools
        .cards
        .card_notes
        .insert("DEMO-323".to_owned(), patch);
    let mut worker = make_agent("demo-320-design-10", AgentStatus::Idle);
    worker.session_id = Some("ses_new".to_owned());
    test_tools.herdr.agents = vec![worker];
    let at = |text: &str| text.parse::<Timestamp>().unwrap().as_millisecond();
    let user = |session, time, text| OpenCodeRow {
        session,
        kind: "user",
        updated_ms: at(time),
        data: json!({ "text": text }),
    };
    write_opencode_db(
        &test_tools.opencode_path(),
        &[
            user(
                "ses_old",
                "2026-11-19T09:00:00Z",
                "Read the handoff at a.md",
            ),
            user("ses_old", "2026-11-19T12:00:00Z", "keep it"),
            user(
                "ses_new",
                "2026-11-21T12:00:00Z",
                "Read the handoff at b.md",
            ),
            user("ses_new", "2026-11-21T13:00:00Z", "hub: the user is away"),
            user(
                "ses_new",
                "2026-11-21T14:00:00Z",
                "hub watch: you have been idle",
            ),
            user("ses_new", "2026-11-21T15:14:00Z", "On Q33: keep it out"),
            user(
                "ses_fix",
                "2026-11-21T12:00:00Z",
                "Read the handoff at c.md",
            ),
            user("ses_fix", "2026-11-21T13:00:00Z", "hub: commit after PR #8"),
            user(
                "ses_fix",
                "2026-11-21T14:00:00Z",
                "From the hub: PR #8 merged",
            ),
        ],
    );

    let outcome = run_brief(&test_tools.tools(), &test_tools.run(), Timestamp::now()).unwrap();

    let typed = "2026-11-21T15:14:00Z"
        .parse::<Timestamp>()
        .unwrap()
        .to_zoned(TimeZone::system())
        .strftime("%Y-%m-%d %H:%M")
        .to_string();
    let questions: Vec<&str> = outcome
        .lines
        .iter()
        .map(String::as_str)
        .filter(|line| line.starts_with("Question ") || line.starts_with("Check first"))
        .collect();
    assert_eq!(
        questions,
        [
            "Question DEMO-320: Keep the main vault in nobs?",
            &format!("Check first: the user wrote in demo-320-design-10 {typed}"),
            "Question DEMO-323: Commit the fix after the merge?",
        ]
    );
}
