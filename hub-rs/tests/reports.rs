mod common;

use std::fs;

use hub::commands::reports::{CardChoice, ReadRequest, run_note, run_read, run_report};
use hub::shared::{AgentName, HubError};

use common::{TestTools, make_time, read_events};

/// A `hub read` request for `name` with this note and card.
fn make_read_request(name: &str, note: Option<&str>, card: CardChoice) -> ReadRequest {
    ReadRequest {
        name: name.parse().unwrap(),
        note: note.map(str::to_owned),
        card,
    }
}

/// The hub acts on what `hub read` prints. A report left unarchived
/// wakes `hub next` again, a note stamped with another time no longer
/// matches its archive, and a missing `read` event leaves the hub
/// unsure whether it answered the report. The `unread` line lets the
/// hub read the next report in the same step, and lists no archive.
#[test]
fn run_read_archives_logs_and_lists_unread() {
    let test_tools = TestTools::new();
    let run_path = test_tools.run_path();
    fs::write(run_path.join("report-docs-fix.md"), "PR opened.\n").unwrap();
    fs::write(run_path.join("report-notes-fix.md"), "Rebased.\n").unwrap();
    fs::write(run_path.join("report-old.1200.read"), "Handled.\n").unwrap();
    fs::write(
        run_path.join("notes.md"),
        "## 2026-11-11\n- 1200 earlier line\n",
    )
    .unwrap();
    let request = make_read_request("docs-fix", Some("docs-fix opened #9."), CardChoice::NoCard);
    let now = make_time("2026-11-11T22:14:00+00:00[UTC]");

    let outcome = run_read(&test_tools.tools(), &test_tools.run(), &request, &now).unwrap();

    assert_eq!(outcome.lines, ["PR opened.", "unread: notes-fix"]);
    assert!(!run_path.join("report-docs-fix.md").exists());
    assert_eq!(
        fs::read_to_string(run_path.join("report-docs-fix.2214.read")).unwrap(),
        "PR opened.\n"
    );
    assert_eq!(
        fs::read_to_string(run_path.join("notes.md")).unwrap(),
        "## 2026-11-11\n- 1200 earlier line\n- 2214 docs-fix opened #9.\n"
    );
    let events = read_events(&run_path);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["event"], "read");
    assert_eq!(events[0]["name"], "docs-fix");
    assert_eq!(events[0]["archive"], "report-docs-fix.2214.read");
}

/// A session can write a new report within a minute of the last, such
/// as a question right after a plan. Reusing the first archive name
/// would replace the first report, and the hub would lose it.
#[test]
fn run_read_keeps_reports_from_same_minute() {
    let test_tools = TestTools::new();
    let run_path = test_tools.run_path();
    let request = make_read_request("docs-fix", None, CardChoice::NoCard);
    let now = make_time("2026-11-11T22:14:00+00:00[UTC]");
    fs::write(run_path.join("report-docs-fix.md"), "Plan ready.\n").unwrap();
    run_read(&test_tools.tools(), &test_tools.run(), &request, &now).unwrap();
    fs::write(run_path.join("report-docs-fix.md"), "Question.\n").unwrap();

    run_read(&test_tools.tools(), &test_tools.run(), &request, &now).unwrap();

    assert_eq!(
        fs::read_to_string(run_path.join("report-docs-fix.2214.read")).unwrap(),
        "Plan ready.\n"
    );
    assert_eq!(
        fs::read_to_string(run_path.join("report-docs-fix.2214-2.read")).unwrap(),
        "Question.\n"
    );
}

/// A mistyped name must not log a note, a `read` event or a card's
/// Log line for a report the hub never read. Exit 3 tells the hub the
/// read did not happen.
#[test]
fn run_read_missing_report_writes_no_note() {
    let test_tools = TestTools::new();
    let request = make_read_request(
        "docs-fx",
        Some("Done."),
        CardChoice::Given("demo-kb-012".parse().unwrap()),
    );
    let now = make_time("2026-11-11T22:14:00+00:00[UTC]");

    let error = run_read(&test_tools.tools(), &test_tools.run(), &request, &now).unwrap_err();

    assert!(matches!(error, HubError::NotDone(_)), "{error:?}");
    assert!(!test_tools.run_path().join("notes.md").exists());
    assert!(read_events(&test_tools.run_path()).is_empty());
    assert!(test_tools.cards.notes.borrow().is_empty());
}

/// `card note` can fail after the report moved to its archive, such as
/// when the vault cannot be reached. A second `hub read` then finds
/// nothing unread, so the error must name the archive or the hub loses
/// the report.
#[test]
fn run_read_names_archive_when_card_fails() {
    let mut test_tools = TestTools::new();
    test_tools.cards.note_error = Some("card: vault not found".to_owned());
    let run_path = test_tools.run_path();
    fs::write(run_path.join("report-docs-fix.md"), "Merged.\n").unwrap();
    let request = make_read_request(
        "docs-fix",
        Some("merged #9"),
        CardChoice::Given("demo-kb-012".parse().unwrap()),
    );
    let now = make_time("2026-11-11T22:14:00+00:00[UTC]");

    let error = run_read(&test_tools.tools(), &test_tools.run(), &request, &now).unwrap_err();

    assert_eq!(error.exit_code(), 1);
    let HubError::PartlyDone { done, error: cause } = error else {
        panic!("expected PartlyDone, got {error:?}");
    };
    assert_eq!(done, "archived report-docs-fix.2214.read");
    assert!(matches!(*cause, HubError::Card(_)), "{cause:?}");
    assert_eq!(
        fs::read_to_string(run_path.join("report-docs-fix.2214.read")).unwrap(),
        "Merged.\n"
    );
}

/// `--card` puts the hub's note on the card's Log, where the user and
/// later sessions read the card's history. A note on the wrong card
/// tells another card's story.
#[test]
fn run_read_card_notes_the_right_card() {
    let cases = [
        (
            "key-in-name",
            "demo-kb-007-impl",
            CardChoice::FromName,
            "DEMO-KB-007",
        ),
        (
            "key-given",
            "docs-fix",
            CardChoice::Given("demo-kb-012".parse().unwrap()),
            "DEMO-KB-012",
        ),
    ];

    for (case_name, name, card, expected_key) in cases {
        let test_tools = TestTools::new();
        fs::write(
            test_tools.run_path().join(format!("report-{name}.md")),
            "Merged.\n",
        )
        .unwrap();
        let request = make_read_request(name, Some("merged #9"), card);
        let now = make_time("2026-11-11T22:14:00+00:00[UTC]");

        run_read(&test_tools.tools(), &test_tools.run(), &request, &now).unwrap();

        assert_eq!(
            *test_tools.cards.notes.borrow(),
            [(expected_key.to_owned(), "merged #9".to_owned())],
            "{case_name}"
        );
    }
}

/// `--card` with no key is a usage mistake when the session's name
/// has no key either. The hub must catch it before the report moves.
/// Archiving first would hide the report from the next `hub read`.
#[test]
fn run_read_card_without_key_changes_nothing() {
    let test_tools = TestTools::new();
    fs::write(
        test_tools.run_path().join("report-docs-fix.md"),
        "Merged.\n",
    )
    .unwrap();
    let request = make_read_request("docs-fix", Some("merged #9"), CardChoice::FromName);
    let now = make_time("2026-11-11T22:14:00+00:00[UTC]");

    let error = run_read(&test_tools.tools(), &test_tools.run(), &request, &now).unwrap_err();

    assert!(matches!(error, HubError::Usage(_)), "{error:?}");
    assert!(test_tools.run_path().join("report-docs-fix.md").exists());
    assert!(!test_tools.run_path().join("notes.md").exists());
}

/// Hubs typed note times by hand, up to two and a half hours off, and
/// a log over several days had no dates. The note takes the real local
/// time, and a date heading starts each new day once.
#[test]
fn run_note_stamps_time_and_day_heading() {
    let test_tools = TestTools::new();
    let notes = test_tools.run_path().join("notes.md");
    fs::write(&notes, "## 2026-11-10\n- 2350 late line\n").unwrap();

    let first = run_note(
        &test_tools.run(),
        "first line",
        &make_time("2026-11-11T00:05:00+00:00[UTC]"),
    )
    .unwrap();
    run_note(
        &test_tools.run(),
        "second line",
        &make_time("2026-11-11T00:10:00+00:00[UTC]"),
    )
    .unwrap();

    assert_eq!(first.lines, ["noted 0005"]);
    assert_eq!(
        fs::read_to_string(&notes).unwrap(),
        "## 2026-11-10\n- 2350 late line\n## 2026-11-11\n- 0005 first line\n- 0010 second line\n"
    );
}

/// Workers write reports with `hub report` instead of their file
/// tools. A report written under the wrong name never reaches the
/// hub, and a wrong line count hides a cut-off report.
#[test]
fn run_report_replaces_report_and_counts_lines() {
    let test_tools = TestTools::new();
    let report = test_tools.run_path().join("report-docs-fix.md");
    fs::write(&report, "Old report.\n").unwrap();
    let name: AgentName = "docs-fix".parse().unwrap();

    let outcome = run_report(&test_tools.run(), &name, "PR opened.\nChecks pass.\n").unwrap();

    assert_eq!(outcome.lines, ["report docs-fix written, 2 lines"]);
    assert_eq!(
        fs::read_to_string(&report).unwrap(),
        "PR opened.\nChecks pass.\n"
    );
}

/// A worker that pipes nothing into `hub report` must not blank the
/// report it already wrote.
#[test]
fn run_report_empty_text_writes_nothing() {
    let test_tools = TestTools::new();
    let report = test_tools.run_path().join("report-docs-fix.md");
    fs::write(&report, "PR opened.\n").unwrap();
    let name: AgentName = "docs-fix".parse().unwrap();

    let error = run_report(&test_tools.run(), &name, " \n").unwrap_err();

    assert!(matches!(error, HubError::NotDone(_)), "{error:?}");
    assert_eq!(fs::read_to_string(&report).unwrap(), "PR opened.\n");
}
