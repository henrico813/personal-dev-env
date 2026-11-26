mod common;

use hub::commands::waiting::run_waiting;
use hub::external::cards::{CardNote, OpenItem, Stage};
use hub::external::herdr::AgentStatus;
use jiff::civil::date;

use common::{InMemoryCards, TestTools, make_agent};

/// Puts card `key` in `stage` with one Open Questions item, Q1, titled
/// `title` with two choices, asked on `day` of November 2026 or
/// undated.
fn add_question_card(
    cards: &mut InMemoryCards,
    key: &str,
    stage: Stage,
    title: &str,
    day: Option<i8>,
) {
    cards.stages.insert(key.to_owned(), stage);
    let item = OpenItem {
        number: 1,
        asked: day.map(|day| date(2026, 11, day)),
        text: title.to_owned(),
        lines: vec!["- A. Yes.".to_owned(), "- B. No.".to_owned()],
    };
    let note = CardNote {
        project: "Demo".to_owned(),
        open_questions: vec![item],
        ..CardNote::default()
    };
    cards.card_notes.insert(key.to_owned(), note);
}

/// The user answers from the end of the hub's newest message, so
/// every open question must show there whole, with its label and the
/// key that answers it. The oldest comes first even when its card key
/// sorts later, since it has waited longest, and an undated item typed
/// by hand comes last. A card's second question takes its own place
/// and number, so it never hides behind the first.
#[test]
fn run_waiting_prints_every_question_oldest_first() {
    let mut test_tools = TestTools::new();
    let cards = &mut test_tools.cards;
    add_question_card(
        cards,
        "DEMO-KB-001",
        Stage::Active,
        "Drop the old page",
        None,
    );
    add_question_card(cards, "DEMO-KB-002", Stage::Active, "Pick a name", Some(24));
    add_question_card(
        cards,
        "DEMO-KB-009",
        Stage::Todo,
        "Deploy tonight",
        Some(21),
    );
    let second = OpenItem {
        number: 5,
        asked: Some(date(2026, 11, 25)),
        text: "Keep the old logs".to_owned(),
        lines: Vec::new(),
    };
    let note = cards.card_notes.get_mut("DEMO-KB-009").unwrap();
    note.open_questions.push(second);

    let outcome = run_waiting(&test_tools.tools()).unwrap();

    assert_eq!(
        outcome.lines,
        [
            "4 questions wait on you.",
            "",
            "**Q1. Deploy tonight (DEMO-KB-009, asked 2026-11-21)**",
            "- A. Yes.",
            "- B. No.",
            "",
            "**Q1. Pick a name (DEMO-KB-002, asked 2026-11-24)**",
            "- A. Yes.",
            "- B. No.",
            "",
            "**Q5. Keep the old logs (DEMO-KB-009, asked 2026-11-25)**",
            "",
            "**Q1. Drop the old page (DEMO-KB-001, undated)**",
            "- A. Yes.",
            "- B. No.",
            "",
            "Reply like `DEMO-KB-009 Q1A`.",
        ]
    );
}

/// The user decided that only cards in todo, active, in-review or
/// blocked wait on them. A question on a draft card is still being
/// written, and one on a done or dropped card needs no answer, so
/// listing either asks the user for nothing.
#[test]
fn run_waiting_skips_draft_and_closed_cards() {
    let mut test_tools = TestTools::new();
    let cards = &mut test_tools.cards;
    let stages = [
        ("DEMO-KB-001", Stage::Draft),
        ("DEMO-KB-002", Stage::Todo),
        ("DEMO-KB-003", Stage::Active),
        ("DEMO-KB-004", Stage::InReview),
        ("DEMO-KB-005", Stage::Blocked),
        ("DEMO-KB-006", Stage::Done),
        ("DEMO-KB-007", Stage::Dropped),
    ];
    for (key, stage) in stages {
        add_question_card(cards, key, stage, "Deploy tonight", Some(21));
    }

    let outcome = run_waiting(&test_tools.tools()).unwrap();

    let titles: Vec<&str> = outcome
        .lines
        .iter()
        .filter(|line| line.starts_with("**Q"))
        .map(String::as_str)
        .collect();
    assert_eq!(
        titles,
        [
            "**Q1. Deploy tonight (DEMO-KB-002, asked 2026-11-21)**",
            "**Q1. Deploy tonight (DEMO-KB-003, asked 2026-11-21)**",
            "**Q1. Deploy tonight (DEMO-KB-004, asked 2026-11-21)**",
            "**Q1. Deploy tonight (DEMO-KB-005, asked 2026-11-21)**",
        ]
    );
}

/// The user glances at the sidebar to see whether questions wait, so
/// the hub's row carries the count and a worker's row does not. Once
/// the last question is answered, the row must drop its old count.
#[test]
fn run_waiting_shows_count_on_hub_row() {
    let mut test_tools = TestTools::new();
    test_tools.herdr.agents = vec![
        make_agent("kanban-hub-7", AgentStatus::Idle),
        make_agent("demo-impl", AgentStatus::Working),
    ];
    let cards = &mut test_tools.cards;
    add_question_card(
        cards,
        "DEMO-KB-001",
        Stage::Active,
        "Deploy tonight",
        Some(21),
    );
    add_question_card(cards, "DEMO-KB-002", Stage::Active, "Pick a name", Some(22));

    run_waiting(&test_tools.tools()).unwrap();
    test_tools.cards.card_notes.clear();
    run_waiting(&test_tools.tools()).unwrap();

    assert_eq!(
        *test_tools.herdr.waiting_counts.borrow(),
        [
            ("w1:kanban-hub-7".to_owned(), 2),
            ("w1:kanban-hub-7".to_owned(), 0),
        ]
    );
}
