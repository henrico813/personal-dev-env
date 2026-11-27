mod common;

use std::fs::{self, File};
use std::time::{Duration, SystemTime};

use hub::board::{self, Hubs, ScreenSize};
use hub::external::cards::{CardNote, OpenItem, Stage};
use hub::external::herdr::AgentStatus;
use hub::shared::HubError;
use jiff::tz::TimeZone;
use jiff::{SignedDuration, Timestamp};
use serde_json::{Value, json};

use common::{TestTools, make_agent};

/// The phone's screen size.
const PHONE: ScreenSize = ScreenSize {
    columns: 40,
    rows: 16,
};

/// Runs `board::control_board` with `steps` as its input, and returns
/// its result and each screen it printed.
fn control(test_tools: &TestTools, steps: &str) -> (Result<(), HubError>, Vec<Value>) {
    let mut output = Vec::new();
    let result = board::control_board(
        &test_tools.tools(),
        &Hubs::Given(test_tools.run_path()),
        &TimeZone::UTC,
        PHONE,
        &mut steps.as_bytes(),
        &mut output,
    );
    let screens = String::from_utf8(output)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    (result.map(|_| ()), screens)
}

/// Without a card vault every `Cards` call fails. The board must still
/// draw, with the failure as the first screen's first row, or the user
/// gets an error in place of the questions and sessions that wait.
#[test]
fn print_board_shows_card_failure_on_top() {
    let mut test_tools = TestTools::new();
    test_tools.cards.read_error = Some("card: no default vault".to_owned());

    let outcome = board::print_board(
        &test_tools.tools(),
        &Hubs::Given(test_tools.run_path()),
        &TimeZone::UTC,
        PHONE,
    )
    .unwrap();

    assert!(outcome.lines[1].starts_with("! questions: card: no"));
    assert!(outcome.lines[2].starts_with("! cards: card: no default"));
}

/// With no watcher, no report or stall alert reaches the phone, and a
/// stalled auto run waits all night. Both faults must lead the first
/// screen, where the user looks first, or the user finds out hours later. The
/// run is approved an hour ago, and its one card has no worker.
#[test]
fn print_board_shows_faults_on_top() {
    let mut test_tools = TestTools::new();
    let run = test_tools.path().join("herdr-hub.auto");
    fs::create_dir(&run).unwrap();
    let started = r#"{"at":"2026-11-12T08:00:00Z","event":"watch_started","hub":"auto-hub-3"}"#;
    fs::write(run.join("events.jsonl"), format!("{started}\n")).unwrap();
    let approved = Timestamp::now()
        .checked_sub(SignedDuration::from_mins(60))
        .unwrap()
        .to_zoned(TimeZone::UTC);
    let approved_text = format!(
        "# Auto run 3 approved\n\nApproved: {}\n\n# Auto run 3\n\n## Run\n\
         - DEMO-KB-001 demo: retry\n",
        approved.strftime("%Y-%m-%d %H:%M")
    );
    fs::write(run.join("approved.md"), approved_text).unwrap();
    test_tools
        .herdr
        .agents
        .push(make_agent("auto-hub-3", AgentStatus::Idle));
    let hubs = Hubs::Live(test_tools.path().to_owned());

    let outcome = board::print_board(&test_tools.tools(), &hubs, &TimeZone::UTC, PHONE).unwrap();

    let screen = outcome.lines.join("\n");
    assert!(
        outcome.lines[1].starts_with("! hub watcher stopped"),
        "{screen}"
    );
    assert!(
        outcome.lines[2].starts_with("! auto run 3 stalled"),
        "{screen}"
    );
}

/// The user opens the board from any shell, with no run folder named,
/// while two hubs work. A worker that reported to the second hub has
/// reported, so it must not wait on the user as one that never
/// reported. A card's open question shows in WAITING ON YOU, on its
/// card's row tagged `answer`, so the user knows which work it holds up.
#[test]
fn print_board_gathers_live_hubs_and_questions() {
    let mut test_tools = TestTools::new();
    for (folder, hub) in [
        ("herdr-hub.one", "kanban-hub-1"),
        ("herdr-hub.two", "auto-hub-2"),
    ] {
        let run = test_tools.path().join(folder);
        fs::create_dir(&run).unwrap();
        let started =
            format!(r#"{{"at":"2026-11-12T08:00:00Z","event":"watch_started","hub":"{hub}"}}"#);
        fs::write(run.join("events.jsonl"), started + "\n").unwrap();
    }
    let worker = "demo-kb-007-impl";
    fs::write(
        test_tools
            .path()
            .join(format!("herdr-hub.two/report-{worker}.md")),
        "DEMO-KB-007 is half done.",
    )
    .unwrap();
    for name in ["kanban-hub-1", "auto-hub-2", worker] {
        test_tools
            .herdr
            .agents
            .push(make_agent(name, AgentStatus::Idle));
    }
    test_tools
        .cards
        .stages
        .insert("DEMO-KB-007".to_owned(), Stage::Active);
    test_tools
        .cards
        .card_notes
        .insert("DEMO-KB-007".to_owned(), CardNote::default());
    test_tools
        .cards
        .stages
        .insert("DEMO-KB-009".to_owned(), Stage::Todo);
    test_tools.cards.card_notes.insert(
        "DEMO-KB-009".to_owned(),
        CardNote {
            project: "Demo".to_owned(),
            open_questions: vec![OpenItem {
                number: 1,
                asked: Some("2026-11-24".parse().unwrap()),
                text: "Deploy tonight?".to_owned(),
                lines: Vec::new(),
            }],
            ..CardNote::default()
        },
    );
    let hubs = Hubs::Live(test_tools.path().to_owned());

    let outcome = board::print_board(&test_tools.tools(), &hubs, &TimeZone::UTC, PHONE).unwrap();

    let screen = outcome.lines.join("\n");
    let waiting = find_line_starting(&outcome.lines, "WAITING ON YOU");
    let row = &outcome.lines[waiting + 1];
    assert!(
        outcome.lines[waiting].starts_with("WAITING ON YOU  1 "),
        "{screen}"
    );
    assert!(row.starts_with(" DEMO-KB-009"), "{screen}");
    assert!(row.ends_with(" answer"), "{screen}");
}

/// A worker sits idle in Herdr between steps, yet a status file it
/// wrote lately shows it is at work. Its card must count as working
/// then, or work in progress looks stuck. A status file older than 30
/// minutes must not keep a stopped card working.
#[test]
fn print_board_counts_working_from_fresh_status() {
    let mut test_tools = TestTools::new();
    let worker = "demo-kb-007-impl";
    let run = test_tools.run_path();
    fs::write(run.join(format!("report-{worker}.md")), "Half done.").unwrap();
    let status = run.join(format!("status-{worker}.md"));
    fs::write(&status, "Step 2 of 3 is built.\n").unwrap();
    test_tools
        .herdr
        .agents
        .push(make_agent(worker, AgentStatus::Idle));
    test_tools
        .cards
        .stages
        .insert("DEMO-KB-007".to_owned(), Stage::Active);
    test_tools
        .cards
        .card_notes
        .insert("DEMO-KB-007".to_owned(), CardNote::default());
    let cases = [
        ("fresh", 0, "ACTIVE  1 · 1 working"),
        ("stale", 31, "ACTIVE  1 · 0 working"),
    ];

    for (case_name, minutes_ago, expected) in cases {
        let written = SystemTime::now() - Duration::from_secs(minutes_ago * 60);
        let file = File::options().write(true).open(&status).unwrap();
        file.set_modified(written).unwrap();

        let outcome = board::print_board(
            &test_tools.tools(),
            &Hubs::Given(run.clone()),
            &TimeZone::UTC,
            PHONE,
        )
        .unwrap();

        let screen = outcome.lines.join("\n");
        let found = outcome.lines.iter().any(|line| line.starts_with(expected));
        assert!(found, "{case_name}: {screen}");
    }
}

/// A `notes.md` entry from `ago` before now, under its UTC day's
/// heading, such as `## 2026-11-15` and `- 0905 TEXT`.
fn format_note_entry(ago: SignedDuration, text: &str) -> String {
    let at = Timestamp::now()
        .checked_sub(ago)
        .unwrap()
        .to_zoned(TimeZone::UTC);
    format!(
        "## {}\n- {} {text}\n",
        at.strftime("%F"),
        at.strftime("%H%M")
    )
}

/// `--print` and `--control` show an agent what the user would see, so
/// SINCE YOU LEFT must start at the time the live board saved, and the
/// top line must say how long the user was away. A board that skipped the saved
/// time would show the agent old news as new.
#[test]
fn print_board_away_starts_at_saved_time() {
    let test_tools = TestTools::new();
    let notes = format_note_entry(SignedDuration::from_mins(120), "parked old-search")
        + &format_note_entry(SignedDuration::from_mins(30), "launched api-build");
    fs::write(test_tools.run_path().join("notes.md"), notes).unwrap();
    let seen = Timestamp::now()
        .checked_sub(SignedDuration::from_mins(90))
        .unwrap();
    let hub_state = test_tools.state_folder().join("hub");
    fs::create_dir_all(&hub_state).unwrap();
    fs::write(hub_state.join("board-seen"), format!("{seen}\n")).unwrap();

    let outcome = board::print_board(
        &test_tools.tools(),
        &Hubs::Given(test_tools.run_path()),
        &TimeZone::UTC,
        PHONE,
    )
    .unwrap();

    let screen = outcome.lines.join("\n");
    let since = find_line_starting(&outcome.lines, "SINCE YOU LEFT");
    assert!(outcome.lines[0].starts_with("away 1h30m · "), "{screen}");
    assert!(outcome.lines[since].contains("  1 card "), "{screen}");
    assert!(
        outcome.lines[since + 1].ends_with(" launched api-build"),
        "{screen}"
    );
}

/// A live hub with a note from 20 minutes ago, and a gone hub, which
/// Herdr no longer lists, with a note from 40 minutes ago that names a
/// card.
fn make_live_and_gone_hubs(test_tools: &mut TestTools) -> Hubs {
    let runs = [
        ("herdr-hub.live", "kanban-hub-2", 20, "launched api-build"),
        (
            "herdr-hub.gone",
            "kanban-hub-1",
            40,
            "parked old-search (DEMO-KB-001 in-review)",
        ),
    ];
    for (folder, hub, minutes_ago, note) in runs {
        let run = test_tools.path().join(folder);
        fs::create_dir(&run).unwrap();
        let started =
            format!(r#"{{"at":"2026-11-12T08:00:00Z","event":"watch_started","hub":"{hub}"}}"#);
        fs::write(run.join("events.jsonl"), started + "\n").unwrap();
        let entry = format_note_entry(SignedDuration::from_mins(minutes_ago), note);
        fs::write(run.join("notes.md"), entry).unwrap();
    }
    test_tools
        .herdr
        .agents
        .push(make_agent("kanban-hub-2", AgentStatus::Idle));
    Hubs::Live(test_tools.path().to_owned())
}

/// With no saved visit time, SINCE YOU LEFT shows the newest events of
/// the live hubs only. A hub that ended long ago must not fill the
/// block with old news.
#[test]
fn print_board_skips_gone_hubs_until_saved() {
    let mut test_tools = TestTools::new();
    let hubs = make_live_and_gone_hubs(&mut test_tools);

    let unsaved = board::print_board(&test_tools.tools(), &hubs, &TimeZone::UTC, PHONE).unwrap();

    let screen = unsaved.lines.join("\n");
    let since = find_line_starting(&unsaved.lines, "SINCE YOU LEFT");
    assert!(
        unsaved.lines[since].starts_with("SINCE YOU LEFT  newest "),
        "{screen}"
    );
    assert!(
        unsaved.lines[since + 1].ends_with(" launched api-build"),
        "{screen}"
    );
    assert!(unsaved.lines[since + 2].starts_with("ACTIVE"), "{screen}");
}

/// A hub that ended, or handed off to a hub in a new run folder, is gone
/// from Herdr, but what it did while the user was away is still news.
/// Once a board saved a time, SINCE YOU LEFT counts the gone hub's
/// notes beside the live hub's. The gone hub's note names a card, so it
/// gets a row of its own.
#[test]
fn print_board_counts_gone_hubs_once_saved() {
    let mut test_tools = TestTools::new();
    let hubs = make_live_and_gone_hubs(&mut test_tools);
    let seen = Timestamp::now()
        .checked_sub(SignedDuration::from_mins(60))
        .unwrap();
    let hub_state = test_tools.state_folder().join("hub");
    fs::create_dir_all(&hub_state).unwrap();
    fs::write(hub_state.join("board-seen"), format!("{seen}\n")).unwrap();

    let saved = board::print_board(&test_tools.tools(), &hubs, &TimeZone::UTC, PHONE).unwrap();

    let screen = saved.lines.join("\n");
    let since = find_line_starting(&saved.lines, "SINCE YOU LEFT");
    assert!(saved.lines[since].contains("  2 cards "), "{screen}");
    assert!(
        saved.lines[since + 1].ends_with(" launched api-build"),
        "{screen}"
    );
    assert_eq!(saved.lines[since + 2], "   + 1 finished", "{screen}");
}

/// With no hub running, as on a fresh login, the board must still draw,
/// and WAITING ON YOU must say no hub runs. An error there leaves the
/// user with no board at all, and `nothing waits on you` would be
/// false, since no hub can ask yet.
#[test]
fn print_board_hubless_says_no_hub_running() {
    let test_tools = make_tools_with_todo_card();
    let hubs = Hubs::Live(test_tools.path().to_owned());

    let outcome = board::print_board(&test_tools.tools(), &hubs, &TimeZone::UTC, PHONE).unwrap();

    let waiting = find_line_starting(&outcome.lines, "WAITING ON YOU");
    assert_eq!(outcome.lines[waiting + 1], " no hub running");
}

/// Returns the index of the first of `lines` that starts with `start`.
fn find_line_starting(lines: &[String], start: &str) -> usize {
    let found = lines.iter().position(|line| line.starts_with(start));
    found.unwrap_or_else(|| panic!("no line starts with {start}: {lines:#?}"))
}

/// The one criterion in the note of `make_tools_with_todo_card`.
const CRITERION: &str = "- [ ] A failed upload retries.";

/// A hub with one card ready to start, DEMO-KB-007, whose note holds
/// `CRITERION`.
fn make_tools_with_todo_card() -> TestTools {
    let mut test_tools = TestTools::new();
    test_tools
        .cards
        .stages
        .insert("DEMO-KB-007".to_owned(), Stage::Todo);
    test_tools.cards.card_notes.insert(
        "DEMO-KB-007".to_owned(),
        CardNote {
            title: "Retry failed uploads".to_owned(),
            criteria: vec![CRITERION.to_owned()],
            ..CardNote::default()
        },
    );
    test_tools
}

/// An agent checks the board by tapping what the user taps. Each step
/// must print the screen it leads to, or the agent reports a screen the
/// user never sees.
#[test]
fn control_board_prints_screen_after_each_step() {
    let test_tools = make_tools_with_todo_card();

    let steps = "tap block:active\ntap card:DEMO-KB-007\npress q\npress q\npress q\n";
    let (result, screens) = control(&test_tools, steps);

    result.unwrap();
    let pages: Vec<(&str, &str)> = screens
        .iter()
        .map(|screen| {
            let step = screen["step"].as_str().unwrap_or("first");
            (step, screen["page"].as_str().unwrap())
        })
        .collect();
    let expected = [
        ("first", "list"),
        ("tap block:active", "list"),
        ("tap card:DEMO-KB-007", "details"),
        ("press q", "list"),
        ("press q", "list"),
        ("press q", "quit"),
    ];
    assert_eq!(pages, expected);
}

/// The hub's read skips the notes of cards ready to start, so
/// opening one must read its note. Otherwise the details page shows
/// no criteria and the user cannot tell what the card has to do.
#[test]
fn control_board_open_card_reads_unread_note() {
    let test_tools = make_tools_with_todo_card();

    let steps = "tap block:active\ntap card:DEMO-KB-007\n";
    let (result, screens) = control(&test_tools, steps);

    result.unwrap();
    let details = &screens[2]["lines"];
    assert!(
        details
            .as_array()
            .unwrap()
            .contains(&Value::from(CRITERION))
    );
}

/// An agent that taps an id the screen does not show must learn which
/// step failed. Going on would show it screens that follow from a tap
/// that never happened.
#[test]
fn control_board_stops_at_unknown_id() {
    let test_tools = TestTools::new();

    let (result, screens) = control(&test_tools, "press j\ntap card:DEMO-KB-404\npress q\n");

    let error = result.unwrap_err();
    assert_eq!(
        error.to_string(),
        "line 2: the screen shows no card:DEMO-KB-404"
    );
    assert_eq!(error.exit_code(), 2);
    assert_eq!(screens.len(), 2);
}

/// The first screen's IN REVIEW heading says how many pull requests
/// fail a check, by the `prs` tool's rule, so the user sees a broken
/// one without opening page 5. A pull request with no checks yet, or
/// with a check still running, has not failed, so it must not raise
/// the count.
#[test]
fn print_board_counts_failing_prs_in_review() {
    let test_tools = TestTools::new();
    let make_pr = |number: u64, checks: Value| {
        json!({
            "number": number,
            "title": "Demo change",
            "repository": {"nameWithOwner": "demo/app"},
            "statusCheckRollup": checks,
        })
    };
    let check_run = |status: &str, conclusion: Option<&str>| json!([{"__typename": "CheckRun", "status": status, "conclusion": conclusion}]);
    let list = json!({"pullRequests": [
        make_pr(1, check_run("COMPLETED", Some("SUCCESS"))),
        make_pr(2, check_run("COMPLETED", Some("FAILURE"))),
        make_pr(3, check_run("IN_PROGRESS", None)),
        make_pr(4, Value::Null),
    ]});
    fs::create_dir_all(test_tools.pr_list.parent().unwrap()).unwrap();
    fs::write(&test_tools.pr_list, list.to_string()).unwrap();

    let outcome = board::print_board(
        &test_tools.tools(),
        &Hubs::Given(test_tools.run_path()),
        &TimeZone::UTC,
        PHONE,
    )
    .unwrap();

    let screen = outcome.lines.join("\n");
    let review = find_line_starting(&outcome.lines, "IN REVIEW");
    assert!(
        outcome.lines[review].starts_with("IN REVIEW  0 · 1 PR fails "),
        "{screen}"
    );
}
