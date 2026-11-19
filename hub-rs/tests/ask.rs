mod common;

use hub::commands::ask::{run_answer, run_ask};
use hub::external::cards::{CardKey, CardNote};
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
fn tools_with_card(note: CardNote) -> TestTools {
    let mut test_tools = TestTools::new();
    test_tools
        .cards
        .card_notes
        .insert("DEMO-KB-007".to_owned(), note);
    test_tools
}

/// The key of the card in `tools_with_card`.
fn demo_key() -> CardKey {
    "DEMO-KB-007".parse().unwrap()
}

/// `card` keeps a section item together only while each later line
/// starts with two spaces and none is blank. A question split into
/// several items loses its choices from `hub brief`, and the needs-you
/// mark must follow the item, since the mark's text finds it.
#[test]
fn run_ask_adds_question_then_marks_card() {
    let test_tools = tools_with_card(CardNote::default());

    let outcome = run_ask(&test_tools.tools(), &demo_key(), QUESTION).unwrap();

    assert_eq!(outcome.lines, ["DEMO-KB-007 asks 'Deploy tonight?'"]);
    assert_eq!(
        *test_tools.cards.writes.borrow(),
        [
            "add DEMO-KB-007 Open Questions: Deploy tonight?\n  \
             Deploy the new model tonight?\n  \
             - A. Deploy from the branch.\n  \
             - B. Wait until you return.\n  \
             Recommended: A, since you test in the morning.\n  \
             If no reply: B.",
            "needs-you DEMO-KB-007: Deploy tonight?",
        ]
    );
}

/// The user decides from the question alone, often on the phone, so
/// it needs its choices, a recommendation and what happens without a
/// reply. `hub ask` names what is missing and writes nothing, so the
/// card never holds half a question.
#[test]
fn run_ask_refuses_incomplete_question() {
    let test_tools = tools_with_card(CardNote::default());
    let text = "## Deploy?\nDeploy now?\n- A. Yes\n- B. No\nIf no reply: B.\n";

    let error = run_ask(&test_tools.tools(), &demo_key(), text).unwrap_err();

    let HubError::NotDone(message) = error else {
        panic!("expected NotDone, got {error:?}");
    };
    assert_eq!(message, "question 'Deploy?': no Recommended: line");
    assert!(test_tools.cards.writes.borrow().is_empty());
}

/// A card shows one needs-you mark, so a second question would hide
/// the first from `hub brief` and `hub board`. `hub ask` refuses and
/// names the question that already waits.
#[test]
fn run_ask_refuses_card_already_waiting() {
    let test_tools = tools_with_card(CardNote {
        needs_you: Some("Which bucket?".to_owned()),
        ..CardNote::default()
    });

    let error = run_ask(&test_tools.tools(), &demo_key(), QUESTION).unwrap_err();

    let HubError::NotDone(message) = error else {
        panic!("expected NotDone, got {error:?}");
    };
    assert_eq!(
        message,
        "DEMO-KB-007 already waits on the user: Which bucket?"
    );
    assert!(test_tools.cards.writes.borrow().is_empty());
}

/// The answer keeps the question beside it in Decisions, so a later
/// reader sees what was decided and why. `card` numbers items from 1,
/// so removing the wrong number deletes another open question. The
/// answered mark comes last, so a failed step leaves the card waiting.
#[test]
fn run_answer_moves_question_to_decisions() {
    let test_tools = tools_with_card(CardNote {
        needs_you: Some("Deploy tonight?".to_owned()),
        open_questions: vec![
            vec!["Is the cache needed?".to_owned()],
            vec![
                "Deploy tonight?".to_owned(),
                "- A. Deploy from the branch.".to_owned(),
                "- B. Wait until you return.".to_owned(),
            ],
        ],
        ..CardNote::default()
    });

    let outcome = run_answer(&test_tools.tools(), &demo_key(), "A").unwrap();

    assert_eq!(outcome.lines, ["DEMO-KB-007 answered 'Deploy tonight?': A"]);
    assert_eq!(
        *test_tools.cards.writes.borrow(),
        [
            "add DEMO-KB-007 Decisions: Deploy tonight?\n  \
             - A. Deploy from the branch.\n  \
             - B. Wait until you return.\n  \
             Answer: A",
            "remove DEMO-KB-007 Open Questions 2",
            "answered DEMO-KB-007: A",
        ]
    );
}
