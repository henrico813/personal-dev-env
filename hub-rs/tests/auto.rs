mod common;

use std::fs::{self, File};
use std::time::SystemTime;

use hub::commands::auto::{
    AutoApproval, run_auto_approve, run_auto_check, run_auto_now, run_auto_todo,
};
use hub::external::cards::{CardNote, Stage};
use hub::external::herdr::{AgentStatus, HerdrError};
use hub::shared::HubError;
use hub::watch::wait::{WaitRequest, run_wait};
use jiff::SignedDuration;

use common::{TestTools, make_agent, make_time, make_workspaces};

/// The time `run_auto_now` runs at in these tests, in UTC so the times
/// it prints do not depend on the machine.
const NOW: &str = "2026-11-20T15:00:00+00:00[UTC]";

/// Doubles whose cards ready to start are these keys, each with a
/// note, and an empty run folder.
fn make_todo_cards(keys: &[&str]) -> TestTools {
    let mut test_tools = TestTools::new();
    for key in keys {
        test_tools.cards.todo.push((*key).to_owned());
        test_tools
            .cards
            .stages
            .insert((*key).to_owned(), Stage::Todo);
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

/// The AI model sorts the cards ready to start from `todo.md` alone,
/// so a card left out or cut short is sorted on a guess. A card with
/// no project shows `-` rather than an empty line.
#[test]
fn run_auto_todo_describes_each_todo_card() {
    let mut test_tools = TestTools::new();
    test_tools.cards.todo = vec!["DEMO-KB-001".to_owned(), "DEMO-KB-002".to_owned()];
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

    let outcome = run_auto_todo(&test_tools.tools(), &test_tools.run()).unwrap();

    let expected = "## DEMO-KB-001 Retry failed uploads\n\
                    Project: Demo\n\
                    - Uploads fail on a slow link.\n\
                    - [ ] A failed upload retries.\n\
                    \n\
                    ## DEMO-KB-002 Plug in the new disk\n\
                    Project: -\n";
    assert_eq!(outcome.lines.join("\n") + "\n", expected);
    assert_eq!(
        fs::read_to_string(test_tools.run_path().join("todo.md")).unwrap(),
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
                           DEMO-KB-009 is not on the board\n\
                           todo card DEMO-KB-002 is missing from run.md"),
        },
    ];

    for case in cases {
        let test_tools = make_todo_cards(&["DEMO-KB-001", "DEMO-KB-002"]);
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
        let test_tools = make_todo_cards(&["DEMO-KB-001"]);
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

/// `run_auto_approve` re-checks `run.md`, so a card a run may not
/// work, such as one the user has not refined, stops the approval and
/// leaves no
/// `approved.md` for workers to start from.
#[test]
fn run_auto_approve_stops_when_list_fails() {
    let mut test_tools = make_todo_cards(&["DEMO-KB-001"]);
    test_tools
        .cards
        .stages
        .insert("DEMO-KB-001".to_owned(), Stage::Draft);
    write_run_list(&test_tools, "- DEMO-KB-001 demo: retry", "");
    let approval = make_approval(2, "approve run 2", false);
    let now = make_time("2026-11-11T22:14:00+00:00[UTC]");

    let result = run_auto_approve(&test_tools.tools(), &test_tools.run(), &approval, &now);

    assert!(matches!(result, Err(HubError::NotDone(_))));
    assert!(!test_tools.run_path().join("approved.md").exists());
}

/// `approved.md` records what the user approved, in the user's own
/// words and with the time, so later sessions can check it. Case and a
/// final period in the reply do not matter.
#[test]
fn run_auto_approve_saves_words_and_list() {
    let test_tools = make_todo_cards(&["DEMO-KB-001"]);
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
        let test_tools = make_todo_cards(&["DEMO-KB-001", "DEMO-KB-002", "DEMO-KB-003"]);
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
    let test_tools = make_todo_cards(&["DEMO-KB-001", "DEMO-KB-002"]);
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
    let test_tools = make_todo_cards(&["DEMO-KB-001"]);
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
/// "nothing new" while four cards were finished and a question
/// waited.
/// Each run card's line comes from the files instead, not from the
/// hub's memory. A card under another heading gets no line, and its
/// open question belongs to no run card, so it is left out.
#[test]
fn run_auto_now_writes_brief_from_files() {
    let mut test_tools = TestTools::new();
    for (key, title) in [
        ("DEMO-KB-002", "Which command name?"),
        ("DEMO-KB-004", "Which disk?"),
    ] {
        test_tools.cards.card_notes.insert(
            key.to_owned(),
            CardNote {
                needs_you: Some(title.to_owned()),
                ..CardNote::default()
            },
        );
    }
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

    let outcome = run_auto_now(&test_tools.tools(), &test_tools.run(), &make_time(NOW)).unwrap();

    let expected = "Auto run 2\n\
                    DEMO-KB-001: PR #12 opened; waiting for the user's review.\n\
                    DEMO-KB-002 (needs you): not started\n\
                    DEMO-KB-003: not started\n\
                    Questions for you: 1\n\
                    - DEMO-KB-002 Which command name?\n";
    assert_eq!(outcome.lines.join("\n") + "\n", expected);
    assert_eq!(
        fs::read_to_string(run_path.join("brief-now.md")).unwrap(),
        expected
    );
}

/// The auto hub runs `run_auto_now` first, so a run the user never
/// approved stops with exit 3 before any worker starts.
#[test]
fn run_auto_now_stops_without_approval() {
    let test_tools = TestTools::new();
    write_run_list(&test_tools, "- DEMO-KB-001 demo: retry", "");

    let result = run_auto_now(&test_tools.tools(), &test_tools.run(), &make_time(NOW));

    assert!(matches!(result, Err(HubError::NotDone(_))));
    assert!(!test_tools.run_path().join("brief-now.md").exists());
}

/// An approved auto run 2 with one card, `DEMO-KB-001`, and no status
/// or questions yet.
fn write_approved_run(test_tools: &TestTools) {
    fs::write(
        test_tools.run_path().join("approved.md"),
        "# Auto run 2\n\n## Run\n- DEMO-KB-001 demo: retry\n",
    )
    .unwrap();
}

/// The auto hub runs `run_auto_now` at each start, and its run's
/// workspace once sat at the bottom of the sidebar because nothing ran
/// `run_gather`. The front hub prints the output word for word, so a
/// working gather adds no line to it.
#[test]
fn run_auto_now_gathers_hubs_silently() {
    let mut test_tools = TestTools::new();
    test_tools.herdr.agents = vec![make_agent("auto-hub-1", AgentStatus::Working)];
    test_tools.herdr.workspaces = make_workspaces(&["w2", "w1"]);
    write_approved_run(&test_tools);

    let outcome = run_auto_now(&test_tools.tools(), &test_tools.run(), &make_time(NOW)).unwrap();

    assert_eq!(
        *test_tools.herdr.moves.borrow(),
        [(vec!["w1".to_owned()], "w2".to_owned())]
    );
    assert_eq!(
        outcome.lines,
        [
            "Auto run 2",
            "DEMO-KB-001: not started",
            "Questions for you: 0"
        ]
    );
}

/// The brief is how the user learns where the run stands, so a Herdr
/// that cannot be reached must not hide it. The warning line goes to
/// the output only, never into `brief-now.md`.
#[test]
fn run_auto_now_prints_brief_when_herdr_fails() {
    let test_tools = TestTools::new();
    test_tools.herdr.fail_next(
        "list_agents",
        HerdrError::Failed("herdr is not running".to_owned()),
    );
    write_approved_run(&test_tools);

    let outcome = run_auto_now(&test_tools.tools(), &test_tools.run(), &make_time(NOW)).unwrap();

    assert_eq!(
        outcome.lines,
        [
            "Auto run 2",
            "DEMO-KB-001: not started",
            "Questions for you: 0",
            "could not put the hubs on top: herdr is not running; hub gather tries again",
        ]
    );
    assert_eq!(
        fs::read_to_string(test_tools.run_path().join("brief-now.md")).unwrap(),
        "Auto run 2\nDEMO-KB-001: not started\nQuestions for you: 0\n"
    );
}

/// Writes the status file of card KEY's worker, last changed MINUTES
/// before `NOW`.
fn write_status(test_tools: &TestTools, key: &str, line: &str, minutes: i64) {
    let path = test_tools
        .run_path()
        .join(format!("status-{}-auto.md", key.to_lowercase()));
    fs::write(&path, format!("{line}\n")).unwrap();
    let changed = make_time(NOW)
        .checked_sub(SignedDuration::from_mins(minutes))
        .unwrap();
    File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_modified(SystemTime::from(changed.timestamp()))
        .unwrap();
}

/// Approves auto run 2 of three cards at 12:00 UTC. DEMO-KB-001 has
/// stopped at a pull request; DEMO-KB-002 parked on the question
/// `Which name?` with an idle worker; DEMO-KB-003 never started.
fn write_run_parked_on_question(test_tools: &mut TestTools) {
    fs::write(
        test_tools.run_path().join("approved.md"),
        "# Auto run 2 approved\n\nApproved: 2026-11-20 12:00\n\n# Auto run 2\n\n## Run\n\
         - DEMO-KB-001 demo: retry\n- DEMO-KB-002 demo: rename\n- DEMO-KB-003 demo: export\n",
    )
    .unwrap();
    write_status(test_tools, "DEMO-KB-001", "PR #12 open", 90);
    test_tools
        .cards
        .stages
        .insert("DEMO-KB-001".to_owned(), Stage::InReview);
    write_status(test_tools, "DEMO-KB-002", "Parked on a question", 60);
    test_tools
        .herdr
        .agents
        .push(make_agent("demo-kb-002-auto", AgentStatus::Idle));
    test_tools.cards.card_notes.insert(
        "DEMO-KB-002".to_owned(),
        CardNote {
            needs_you: Some("Which name?".to_owned()),
            ..CardNote::default()
        },
    );
}

/// Returns the `STALLED` or `Run ended` line `run_auto_now` ends with,
/// if any.
fn run_and_read_stall_line(test_tools: &TestTools) -> Option<String> {
    let outcome = run_auto_now(&test_tools.tools(), &test_tools.run(), &make_time(NOW)).unwrap();
    outcome
        .lines
        .last()
        .filter(|line| line.starts_with("STALLED") || line.starts_with("Run ended"))
        .cloned()
}

/// A run once sat stalled for three hours. One card waited on a
/// question, the card stacked on it could not start, and nothing told
/// the user. The brief must end with a `STALLED` line naming each card
/// that waits and why. A card's worker or the auto hub that works, or a
/// worker that waits on a job with `hub wait`, is progress, so no line
/// may appear then.
#[test]
fn run_auto_now_reports_stall_without_progress() {
    struct Case {
        name: &'static str,
        agent: Option<(&'static str, AgentStatus)>,
        waits: bool,
        expected: Option<&'static str>,
    }
    let cases = [
        Case {
            name: "stalled",
            agent: None,
            waits: false,
            expected: Some(
                "STALLED since 14:00: DEMO-KB-002, waits on your question: Which name?; \
                 DEMO-KB-003, not started (1 of 3 cards done)",
            ),
        },
        Case {
            name: "worker-working",
            agent: Some(("demo-kb-003-auto", AgentStatus::Working)),
            waits: false,
            expected: None,
        },
        Case {
            name: "auto-hub-working",
            agent: Some(("auto-hub-2", AgentStatus::Working)),
            waits: false,
            expected: None,
        },
        Case {
            name: "hub-wait",
            agent: None,
            waits: true,
            expected: None,
        },
    ];
    for case in cases {
        let mut test_tools = TestTools::new();
        write_run_parked_on_question(&mut test_tools);
        if let Some((name, status)) = case.agent {
            test_tools.herdr.agents.push(make_agent(name, status));
        }
        if case.waits {
            let request = WaitRequest {
                name: "demo-kb-002-auto".parse().unwrap(),
                pid: None,
                file: Some(test_tools.path().join("tests.log")),
                deadline_minutes: 120,
            };
            run_wait(&test_tools.run(), &request, &make_time(NOW)).unwrap();
        }

        let line = run_and_read_stall_line(&test_tools);

        assert_eq!(line.as_deref(), case.expected, "{}", case.name);
    }
}

/// The auto-run skill ends a run when every card stopped, at a pull
/// request or a question, and the auto hub then sends its end brief.
/// Such a run must say it ended, never `STALLED`, or the phone would
/// buzz each hour after every run that ends on a question.
#[test]
fn run_auto_now_reports_end_not_stall() {
    let cases = [
        (
            "all-done",
            "Run ended: every card stopped at a pull request",
        ),
        ("brief-sent", "Run ended: the auto hub sent its end brief"),
    ];
    for (name, expected) in cases {
        let mut test_tools = TestTools::new();
        write_run_parked_on_question(&mut test_tools);
        if name == "all-done" {
            test_tools.cards.card_notes.clear();
            for key in ["DEMO-KB-002", "DEMO-KB-003"] {
                write_status(&test_tools, key, "PR open", 60);
                test_tools
                    .cards
                    .stages
                    .insert(key.to_owned(), Stage::InReview);
            }
        } else {
            let brief = test_tools.run_path().join("report-auto-hub-2.md");
            fs::write(brief, "End brief\n").unwrap();
        }

        let line = run_and_read_stall_line(&test_tools);

        assert_eq!(line.as_deref(), Some(expected), "{name}");
    }
}
