mod common;

use hub::commands::ask::{run_answer, run_ask};
use hub::external::cards::{CardKey, CardNote, OpenItem, Stage};
use hub::shared::HubError;

use common::TestTools;

/// A whole question file, as the hub writes one.
const QUESTION: &str = "## Deploy tonight?\n\
                        \n\
                        Deploy the new model tonight?\n\
                        \n\
                        - A. Deploy from the branch.\n\
                        - B. Wait until you return.\n\
                        \n\
                        Recommended: A, since you test in the morning.\n\
                        If no reply: B.\n";

/// Doubles with the card `DEMO-KB-007`, which shows `note`.
fn make_tools_with_card(note: CardNote) -> TestTools {
    let mut test_tools = TestTools::new();
    test_tools
        .cards
        .card_notes
        .insert("DEMO-KB-007".to_owned(), note);
    test_tools
}

/// The key of the card in `make_tools_with_card`.
fn make_demo_key() -> CardKey {
    "DEMO-KB-007".parse().unwrap()
}

/// Returns open question Q`number` asked on 2026-11-24, with `lines`
/// after its title.
fn make_item(number: u32, title: &str, lines: &[&str]) -> OpenItem {
    OpenItem {
        number,
        asked: Some("2026-11-24".parse().unwrap()),
        text: title.to_owned(),
        lines: lines.iter().map(|line| (*line).to_owned()).collect(),
    }
}

/// `card` keeps an item whole only while each later line starts with
/// two spaces, so a split question loses its choices from `hub waiting`.
/// A card may hold several open questions, so an open one must not stop
/// a new one.
#[test]
fn run_ask_adds_whole_second_question() {
    let test_tools = make_tools_with_card(CardNote {
        stage: Some(Stage::Active),
        open_questions: vec![make_item(1, "Which bucket?", &[])],
        ..CardNote::default()
    });

    let outcome = run_ask(&test_tools.tools(), &make_demo_key(), QUESTION).unwrap();

    assert_eq!(outcome.lines, ["DEMO-KB-007 asks 'Deploy tonight?'"]);
    assert_eq!(
        *test_tools.cards.writes.borrow(),
        ["add DEMO-KB-007 Open Questions: Deploy tonight?\n  \
          Deploy the new model tonight?\n  \
          - A. Deploy from the branch.\n  \
          - B. Wait until you return.\n  \
          Recommended: A, since you test in the morning.\n  \
          If no reply: B."]
    );
}

/// The user decides from the question alone, often on the phone, so
/// it needs its choices, a recommendation and what happens without a
/// reply. `hub ask` names what is missing and writes nothing, so the
/// card never holds half a question.
#[test]
fn run_ask_refuses_incomplete_question() {
    let test_tools = make_tools_with_card(CardNote::default());
    let text = "## Deploy?\nDeploy now?\n- A. Yes\n- B. No\nIf no reply: B.\n";

    let error = run_ask(&test_tools.tools(), &make_demo_key(), text).unwrap_err();

    let HubError::NotDone(message) = error else {
        panic!("expected NotDone, got {error:?}");
    };
    assert_eq!(message, "question 'Deploy?': no Recommended: line");
    assert!(test_tools.cards.writes.borrow().is_empty());
}

/// The hub lists no question on a draft card, so a question asked
/// there would never reach the user. `hub ask` refuses and writes
/// nothing.
#[test]
fn run_ask_refuses_draft_card() {
    let test_tools = make_tools_with_card(CardNote {
        stage: Some(Stage::Draft),
        ..CardNote::default()
    });

    let error = run_ask(&test_tools.tools(), &make_demo_key(), QUESTION).unwrap_err();

    let HubError::NotDone(message) = error else {
        panic!("expected NotDone, got {error:?}");
    };
    assert_eq!(
        message,
        "DEMO-KB-007 is in draft; the hub lists questions only on todo, \
         active, in-review or blocked cards"
    );
    assert!(test_tools.cards.writes.borrow().is_empty());
}

/// A rerun of `hub ask` after a timeout or a handoff must not add a
/// second copy, which would stay listed after the user answers the
/// first.
#[test]
fn run_ask_refuses_same_open_question() {
    let test_tools = make_tools_with_card(CardNote {
        stage: Some(Stage::Active),
        open_questions: vec![make_item(6, "Deploy tonight?", &[])],
        ..CardNote::default()
    });

    let error = run_ask(&test_tools.tools(), &make_demo_key(), QUESTION).unwrap_err();

    let HubError::NotDone(message) = error else {
        panic!("expected NotDone, got {error:?}");
    };
    assert_eq!(message, "DEMO-KB-007 already asks 'Deploy tonight?' as Q6");
    assert!(test_tools.cards.writes.borrow().is_empty());
}

/// The answer keeps the question beside it in Decisions, with its Q
/// number so `card` never reuses it, and a later reader sees what was
/// decided and why. Only the named item may go, since
/// removing another number or the first item deletes a question the
/// user has not answered.
#[test]
fn run_answer_moves_only_named_question() {
    let test_tools = make_tools_with_card(CardNote {
        stage: Some(Stage::Active),
        open_questions: vec![
            make_item(1, "Is the cache needed?", &["- A. Yes.", "- B. No."]),
            make_item(
                3,
                "Deploy tonight?",
                &[
                    "- A. Deploy from the branch.",
                    "- B. Wait until you return.",
                ],
            ),
        ],
        ..CardNote::default()
    });

    let outcome = run_answer(&test_tools.tools(), &make_demo_key(), 3, "A").unwrap();

    assert_eq!(
        outcome.lines,
        ["DEMO-KB-007 Q3 answered 'Deploy tonight?': A"]
    );
    assert_eq!(
        *test_tools.cards.writes.borrow(),
        [
            "add DEMO-KB-007 Decisions: Q3 asked 2026-11-24: Deploy tonight?\n  \
             - A. Deploy from the branch.\n  \
             - B. Wait until you return.\n  \
             Answer: A",
            "remove DEMO-KB-007 Q3",
        ]
    );
}

/// A failed Decisions write must leave the question open, or the user's
/// answer and the question are both lost.
#[test]
fn run_answer_failed_decision_keeps_question() {
    let mut test_tools = make_tools_with_card(CardNote {
        stage: Some(Stage::Active),
        open_questions: vec![make_item(3, "Deploy tonight?", &[])],
        ..CardNote::default()
    });
    test_tools.cards.add_error = Some("card: vault not mounted".to_owned());

    let error = run_answer(&test_tools.tools(), &make_demo_key(), 3, "A").unwrap_err();

    assert!(matches!(error, HubError::Card(_)), "got {error:?}");
    assert!(test_tools.cards.writes.borrow().is_empty());
}

/// When the removal fails after the answer is saved, the error must
/// say so, or a retried `hub answer` adds the answer to Decisions twice.
#[test]
fn run_answer_failed_removal_says_answer_saved() {
    let mut test_tools = make_tools_with_card(CardNote {
        stage: Some(Stage::Active),
        open_questions: vec![make_item(3, "Deploy tonight?", &[])],
        ..CardNote::default()
    });
    test_tools.cards.remove_error = Some("card: vault not mounted".to_owned());

    let error = run_answer(&test_tools.tools(), &make_demo_key(), 3, "A").unwrap_err();

    assert_eq!(
        error.to_string(),
        "added the answer to DEMO-KB-007 Decisions, but card: vault not mounted"
    );
    assert_eq!(
        *test_tools.cards.writes.borrow(),
        ["add DEMO-KB-007 Decisions: Q3 asked 2026-11-24: Deploy tonight?\n  Answer: A"]
    );
}

/// A hand edit between the hub's read and its removal can give Q3 to
/// another item, as a hand-typed item's number can shift. The hub
/// keeps the answer but must not remove a question the user did not
/// answer.
#[test]
fn run_answer_keeps_question_changed_since_read() {
    let mut test_tools = make_tools_with_card(CardNote {
        stage: Some(Stage::Active),
        open_questions: vec![make_item(3, "Deploy tonight?", &[])],
        ..CardNote::default()
    });
    test_tools.cards.notes_after_write.insert(
        "DEMO-KB-007".to_owned(),
        CardNote {
            stage: Some(Stage::Active),
            open_questions: vec![make_item(3, "Keep the old logs?", &[])],
            ..CardNote::default()
        },
    );

    let error = run_answer(&test_tools.tools(), &make_demo_key(), 3, "A").unwrap_err();

    assert_eq!(
        error.to_string(),
        "added the answer to DEMO-KB-007 Decisions, but DEMO-KB-007 Q3 \
         changed since the hub read it, so 'Deploy tonight?' stays open; \
         find its number in hub waiting, then run card edit DEMO-KB-007 \
         \"Open Questions\" Q<n> --remove, never hub answer"
    );
    assert_eq!(
        *test_tools.cards.writes.borrow(),
        ["add DEMO-KB-007 Decisions: Q3 asked 2026-11-24: Deploy tonight?\n  Answer: A"]
    );
}

/// A mistyped or already answered number must not record a decision
/// for a question that is not open.
#[test]
fn run_answer_refuses_number_with_no_question() {
    let test_tools = make_tools_with_card(CardNote {
        stage: Some(Stage::Active),
        open_questions: vec![make_item(1, "Is the cache needed?", &[])],
        ..CardNote::default()
    });

    let error = run_answer(&test_tools.tools(), &make_demo_key(), 4, "A").unwrap_err();

    let HubError::NotDone(message) = error else {
        panic!("expected NotDone, got {error:?}");
    };
    assert_eq!(message, "DEMO-KB-007 has no open question Q4");
    assert!(test_tools.cards.writes.borrow().is_empty());
}

/// A line break in the answer lets `card` read the rest as an item or
/// Log line of its own, such as an open question nobody asked, and an
/// empty answer records no decision. Both are refused before any write.
#[test]
fn run_answer_refuses_empty_or_multiline_answer() {
    let cases = [("empty", " "), ("newline", "A\n- Q9: injected?")];
    for (name, answer) in cases {
        let test_tools = make_tools_with_card(CardNote {
            open_questions: vec![make_item(1, "Deploy tonight?", &[])],
            ..CardNote::default()
        });

        let error = run_answer(&test_tools.tools(), &make_demo_key(), 1, answer).unwrap_err();

        let HubError::NotDone(message) = error else {
            panic!("{name}: expected NotDone, got {error:?}");
        };
        assert_eq!(message, "the answer must be one line of text", "{name}");
        assert!(test_tools.cards.writes.borrow().is_empty(), "{name}");
    }
}
