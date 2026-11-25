mod common;

use hub::commands::waiting::run_waiting;
use hub::external::cards::CardNote;
use hub::external::herdr::AgentStatus;
use jiff::civil::date;

use common::{TestTools, make_agent};

/// A card whose open question, titled `title` with two choices, was
/// asked on `day` of November 2026.
fn make_question_card(title: &str, day: i8) -> CardNote {
    CardNote {
        project: "Demo".to_owned(),
        needs_you: Some(title.to_owned()),
        needs_you_date: Some(date(2026, 11, day)),
        open_questions: vec![vec![
            title.to_owned(),
            "- A. Yes.".to_owned(),
            "- B. No.".to_owned(),
        ]],
        ..CardNote::default()
    }
}

/// The user answers from the end of the hub's newest message, so
/// every open question must show there whole, with the key that
/// answers it. The oldest comes first even when its card key sorts
/// later, since it has waited longest.
#[test]
fn run_waiting_prints_oldest_question_first_whole() {
    let mut test_tools = TestTools::new();
    let cards = &mut test_tools.cards.card_notes;
    cards.insert(
        "DEMO-KB-002".to_owned(),
        make_question_card("Pick a name", 24),
    );
    cards.insert(
        "DEMO-KB-009".to_owned(),
        make_question_card("Deploy tonight", 21),
    );

    let outcome = run_waiting(&test_tools.tools(), &test_tools.run()).unwrap();

    assert_eq!(
        outcome.lines,
        [
            "2 questions wait on you.",
            "",
            "**Q1. Deploy tonight (DEMO-KB-009)**",
            "- A. Yes.",
            "- B. No.",
            "",
            "**Q2. Pick a name (DEMO-KB-002)**",
            "- A. Yes.",
            "- B. No.",
            "",
            "Reply like `Q1A`.",
        ]
    );
}

/// A reply key such as `Q2A` must name the same question in every
/// message, or a reply to an older message answers the wrong card.
/// Numbers start again at Q1 only once no numbered question waits.
#[test]
fn run_waiting_keeps_numbers_until_none_wait() {
    let mut test_tools = TestTools::new();
    let cards = &mut test_tools.cards.card_notes;
    cards.insert(
        "DEMO-KB-001".to_owned(),
        make_question_card("Deploy tonight", 21),
    );
    cards.insert(
        "DEMO-KB-002".to_owned(),
        make_question_card("Pick a name", 22),
    );
    let read_titles = |lines: Vec<String>| -> Vec<String> {
        lines
            .into_iter()
            .filter(|line| line.starts_with("**Q"))
            .collect()
    };

    // The first run numbers DEMO-KB-001 as Q1 and DEMO-KB-002 as Q2.
    run_waiting(&test_tools.tools(), &test_tools.run()).unwrap();

    // DEMO-KB-001 is answered and DEMO-KB-003 asks a new question.
    let cards = &mut test_tools.cards.card_notes;
    cards.remove("DEMO-KB-001");
    cards.insert(
        "DEMO-KB-003".to_owned(),
        make_question_card("Drop the old page", 23),
    );
    let kept = run_waiting(&test_tools.tools(), &test_tools.run()).unwrap();

    // Every question is answered, which ends the numbers.
    test_tools.cards.card_notes.clear();
    let cleared = run_waiting(&test_tools.tools(), &test_tools.run()).unwrap();

    // The next question starts again at Q1.
    let cards = &mut test_tools.cards.card_notes;
    cards.insert(
        "DEMO-KB-004".to_owned(),
        make_question_card("Rename the tool", 24),
    );
    let restarted = run_waiting(&test_tools.tools(), &test_tools.run()).unwrap();

    assert_eq!(
        read_titles(kept.lines),
        [
            "**Q2. Pick a name (DEMO-KB-002)**",
            "**Q3. Drop the old page (DEMO-KB-003)**",
        ]
    );
    assert_eq!(cleared.lines, ["No question waits on you."]);
    assert_eq!(
        read_titles(restarted.lines),
        ["**Q1. Rename the tool (DEMO-KB-004)**"]
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
    let cards = &mut test_tools.cards.card_notes;
    cards.insert(
        "DEMO-KB-001".to_owned(),
        make_question_card("Deploy tonight", 21),
    );
    cards.insert(
        "DEMO-KB-002".to_owned(),
        make_question_card("Pick a name", 22),
    );

    run_waiting(&test_tools.tools(), &test_tools.run()).unwrap();
    test_tools.cards.card_notes.clear();
    run_waiting(&test_tools.tools(), &test_tools.run()).unwrap();

    assert_eq!(
        *test_tools.herdr.waiting_counts.borrow(),
        [
            ("w1:kanban-hub-7".to_owned(), 2),
            ("w1:kanban-hub-7".to_owned(), 0),
        ]
    );
}
