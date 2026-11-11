mod common;

use std::fs;

use hub::commands::auto::{
    AutoApproval, run_auto_approve, run_auto_check, run_auto_now, run_auto_ready,
};
use hub::outside::cards::CardNote;
use hub::shared::HubError;

use common::{TestTools, make_time};

/// Doubles whose ready cards are these keys, each with a note, and an
/// empty run folder.
fn make_ready_cards(keys: &[&str]) -> TestTools {
    let mut test_tools = TestTools::new();
    for key in keys {
        test_tools.cards.ready.push((*key).to_owned());
        test_tools.cards.card_notes.insert(
            (*key).to_owned(),
            CardNote {
                title: "Retry failed uploads".to_owned(),
                project: "Demo".to_owned(),
                ..CardNote::default()
            },
        );
    }
    test_tools
}

/// Writes `run.md` for auto run 2 with these lines under `## Run`, then
/// any further headings and their lines.
fn write_run_list(test_tools: &TestTools, run: &str, rest: &str) {
    fs::write(
        test_tools.run_path().join("run.md"),
        format!("# Auto run 2\n\n## Run\n{run}\n{rest}"),
    )
    .unwrap();
}

/// An approval of run `number` with this reply.
fn make_approval(number: u32, reply: &str, given: bool) -> AutoApproval {
    AutoApproval {
        number,
        reply: reply.to_owned(),
        given,
    }
}

/// The AI model sorts the ready cards from `ready.md` alone, so a card
/// left out or cut short is sorted on a guess. A card with no project
/// shows `-` rather than an empty line.
#[test]
fn run_auto_ready_describes_each_ready_card() {
    let mut test_tools = TestTools::new();
    test_tools.cards.ready = vec!["DEMO-KB-001".to_owned(), "DEMO-KB-002".to_owned()];
    test_tools.cards.card_notes = [
        (
            "DEMO-KB-001".to_owned(),
            CardNote {
                title: "Retry failed uploads".to_owned(),
                project: "Demo".to_owned(),
                criteria: vec![
                    "- Uploads fail on a slow link.".to_owned(),
                    "- [ ] A failed upload retries.".to_owned(),
                ],
                ..CardNote::default()
            },
        ),
        (
            "DEMO-KB-002".to_owned(),
            CardNote {
                title: "Plug in the new disk".to_owned(),
                ..CardNote::default()
            },
        ),
    ]
    .into();

    let outcome = run_auto_ready(&test_tools.tools(), &test_tools.run()).unwrap();

    let expected = "## DEMO-KB-001 Retry failed uploads\n\
                    Project: Demo\n\
                    - Uploads fail on a slow link.\n\
                    - [ ] A failed upload retries.\n\
                    \n\
                    ## DEMO-KB-002 Plug in the new disk\n\
                    Project: -\n";
    assert_eq!(outcome.lines.join("\n") + "\n", expected);
    assert_eq!(
        fs::read_to_string(test_tools.run_path().join("ready.md")).unwrap(),
        expected
    );
}

/// The auto hub reads this output to fix `run.md` before the user sees
/// it. A problem left out of the output goes to the user's briefing
/// unfixed, and a wrong card count misstates what the run works.
#[test]
fn run_auto_check_prints_problems_or_count() {
    struct Case {
        name: &'static str,
        run: &'static str,
        rest: &'static str,
        expected: Result<&'static str, &'static str>,
    }
    let cases = [
        Case {
            name: "complete",
            run: "- DEMO-KB-001 demo: retry",
            rest: "\n## Needs your hands\n- DEMO-KB-002 demo: plug in a disk\n",
            expected: Ok("run 2: 1 cards to run"),
        },
        Case {
            name: "problems",
            run: "- DEMO-KB-001 demo: retry\n\
                  - DEMO-KB-001 demo: retry\n\
                  - DEMO-KB-009 demo: export",
            rest: "",
            expected: Err("DEMO-KB-001 is listed more than once\n\
                           DEMO-KB-009 is not a ready card; only the user moves a card to ready\n\
                           ready card DEMO-KB-002 is missing from run.md"),
        },
    ];

    for case in cases {
        let test_tools = make_ready_cards(&["DEMO-KB-001", "DEMO-KB-002"]);
        write_run_list(&test_tools, case.run, case.rest);

        let printed = match run_auto_check(&test_tools.tools(), &test_tools.run()) {
            Ok(outcome) => Ok(outcome.lines.join("\n")),
            Err(HubError::NotDone(problems)) => Err(problems),
            Err(error) => panic!("{}: {error}", case.name),
        };

        let expected = case.expected.map(str::to_owned).map_err(str::to_owned);
        assert_eq!(printed, expected, "{}", case.name);
    }
}

/// Auto run 1 started on a "go" the user meant for another question,
/// so only the words `approve run N` may start a run, where N is the
/// run's number in `run.md`. Any other reply stops with exit 3 and
/// writes no `approved.md`.
#[test]
fn run_auto_approve_ignores_other_replies() {
    struct Case {
        name: &'static str,
        number: u32,
        reply: &'static str,
    }
    let cases = [
        Case {
            name: "go",
            number: 2,
            reply: "go",
        },
        Case {
            name: "other-phrase",
            number: 2,
            reply: "approve run 3",
        },
        Case {
            name: "edit",
            number: 2,
            reply: "approve run 2 without DEMO-KB-001",
        },
        Case {
            name: "other-run",
            number: 3,
            reply: "approve run 3",
        },
    ];

    for case in cases {
        let test_tools = make_ready_cards(&["DEMO-KB-001"]);
        write_run_list(&test_tools, "- DEMO-KB-001 demo: retry", "");
        let approval = make_approval(case.number, case.reply, false);
        let now = make_time("2026-11-11T22:14:00+00:00[UTC]");

        let result = run_auto_approve(&test_tools.tools(), &test_tools.run(), &approval, &now);

        assert!(matches!(result, Err(HubError::NotDone(_))), "{}", case.name);
        assert!(
            !test_tools.run_path().join("approved.md").exists(),
            "{}",
            case.name
        );
    }
}

/// `approved.md` records what the user approved, in the user's own
/// words and with the time, so later sessions can check it. Case and a
/// final period in the reply do not matter.
#[test]
fn run_auto_approve_saves_list_with_reply() {
    let test_tools = make_ready_cards(&["DEMO-KB-001"]);
    write_run_list(&test_tools, "- DEMO-KB-001 demo: retry", "");
    let approval = make_approval(2, "Approve run 2.", false);
    let now = make_time("2026-11-11T22:14:00+00:00[UTC]");

    let outcome =
        run_auto_approve(&test_tools.tools(), &test_tools.run(), &approval, &now).unwrap();

    assert_eq!(outcome.lines, ["approved run 2: 1 cards"]);
    assert_eq!(
        fs::read_to_string(test_tools.run_path().join("approved.md")).unwrap(),
        "# Auto run 2 approved\n\n\
         Approved: 2026-11-11 22:14\n\
         The user's words: Approve run 2.\n\n\
         # Auto run 2\n\n## Run\n- DEMO-KB-001 demo: retry\n"
    );
}

/// Words the user gave before the briefing are checked only by the
/// cards they name. A card they did not name must not run. When the
/// list leaves out a card they did name, the list is not what they
/// approved.
#[test]
fn run_auto_approve_given_needs_exact_cards() {
    let cases = [
        ("card-missing", "go ahead with DEMO-KB-001"),
        ("extra-card", "run DEMO-KB-001, DEMO-KB-002 and DEMO-KB-003"),
        ("no-card", "you have my approval"),
    ];

    for (case_name, words) in cases {
        let test_tools = make_ready_cards(&["DEMO-KB-001", "DEMO-KB-002", "DEMO-KB-003"]);
        write_run_list(
            &test_tools,
            "- DEMO-KB-001 demo: retry\n- DEMO-KB-002 demo: resume",
            "\n## Needs your hands\n- DEMO-KB-003 demo: plug in a disk\n",
        );
        let approval = make_approval(2, words, true);
        let now = make_time("2026-11-11T22:14:00+00:00[UTC]");

        let result = run_auto_approve(&test_tools.tools(), &test_tools.run(), &approval, &now);

        assert!(matches!(result, Err(HubError::NotDone(_))), "{case_name}");
        assert!(
            !test_tools.run_path().join("approved.md").exists(),
            "{case_name}"
        );
    }
}

/// A user who approves a run before its briefing exists does so in
/// their own words, and the hub records them rather than writing
/// `approved.md` by hand. Keys count in lower case too.
#[test]
fn run_auto_approve_given_saves_own_words() {
    let test_tools = make_ready_cards(&["DEMO-KB-001", "DEMO-KB-002"]);
    write_run_list(
        &test_tools,
        "- DEMO-KB-001 demo: retry\n- DEMO-KB-002 demo: resume",
        "",
    );
    let words = "Run demo-kb-001 and DEMO-KB-002 while I am away.";
    let approval = make_approval(2, words, true);
    let now = make_time("2026-11-11T22:14:00+00:00[UTC]");

    run_auto_approve(&test_tools.tools(), &test_tools.run(), &approval, &now).unwrap();

    let approved = fs::read_to_string(test_tools.run_path().join("approved.md")).unwrap();
    assert!(approved.contains(&format!("The user's words: {words}\n")));
}

/// Later sessions check their work against `approved.md`. A second
/// approval must not replace it, or the record would show a list or
/// words other than the ones the user approved first.
#[test]
fn run_auto_approve_keeps_existing_approval() {
    let test_tools = make_ready_cards(&["DEMO-KB-001"]);
    write_run_list(&test_tools, "- DEMO-KB-001 demo: retry", "");
    let first = "# Auto run 2 approved\n\nThe user's words: approve run 2\n";
    fs::write(test_tools.run_path().join("approved.md"), first).unwrap();
    let approval = make_approval(2, "approve run 2", false);
    let now = make_time("2026-11-11T22:14:00+00:00[UTC]");

    let result = run_auto_approve(&test_tools.tools(), &test_tools.run(), &approval, &now);

    assert!(matches!(result, Err(HubError::NotDone(_))));
    assert_eq!(
        fs::read_to_string(test_tools.run_path().join("approved.md")).unwrap(),
        first
    );
}

/// The hub the user talks to once answered "status update" with
/// "nothing new" while four cards were done and a question waited.
/// Each run card's line comes from the files instead, not from the
/// hub's memory. A card under another heading gets no line.
#[test]
fn run_auto_now_writes_line_per_card() {
    let test_tools = TestTools::new();
    let run_path = test_tools.run_path();
    fs::write(
        run_path.join("approved.md"),
        "# Auto run 2 approved\n\nThe user's words: approve run 2\n\n\
         # Auto run 2\n\n## Run\n\
         - DEMO-KB-001 demo: retry\n\
         - DEMO-KB-002 demo: resize\n\
         - DEMO-KB-003 demo: export\n\n\
         ## Needs your hands\n- DEMO-KB-004 demo: plug in a disk\n",
    )
    .unwrap();
    fs::write(
        run_path.join("status-demo-kb-001-auto.md"),
        "\nPR #12 opened; waiting for the user's review.\nMore.\n",
    )
    .unwrap();
    fs::write(
        run_path.join("decisions.md"),
        "## DEMO-KB-002 command name\n\nWhich name?\n\n\
         - A. resize\n- B. scale\n\n\
         Recommended: A.\nIf no reply: A.\n",
    )
    .unwrap();

    let outcome = run_auto_now(&test_tools.run()).unwrap();

    let expected = "Auto run 2\n\
                    DEMO-KB-001: PR #12 opened; waiting for the user's review.\n\
                    DEMO-KB-002 (needs you): not started\n\
                    DEMO-KB-003: not started\n\
                    Questions for you: 1\n\
                    - DEMO-KB-002 command name\n";
    assert_eq!(outcome.lines.join("\n") + "\n", expected);
    assert_eq!(
        fs::read_to_string(run_path.join("brief-now.md")).unwrap(),
        expected
    );
}

/// The auto hub runs `hub auto now` first, so a run the user never
/// approved stops with exit 3 before any worker starts.
#[test]
fn run_auto_now_stops_without_approval() {
    let test_tools = TestTools::new();
    write_run_list(&test_tools, "- DEMO-KB-001 demo: retry", "");

    let result = run_auto_now(&test_tools.run());

    assert!(matches!(result, Err(HubError::NotDone(_))));
    assert!(!test_tools.run_path().join("brief-now.md").exists());
}
