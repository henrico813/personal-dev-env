mod common;

use std::fs;

use hub::board::{self, Hubs, ScreenSize};
use hub::external::cards::{CardNote, Stage};
use hub::external::herdr::AgentStatus;
use hub::shared::HubError;
use jiff::tz::TimeZone;
use jiff::{SignedDuration, Timestamp};
use serde_json::Value;

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
/// draw, with the failure as the QUEUE block's first row, or the user
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

    assert_eq!(outcome.lines[5], "QUEUE                                [2]");
    assert!(outcome.lines[6].starts_with(" !    cards: card: no default"));
}

/// The user opens the board from any shell, with no run folder named,
/// while two hubs work. QUEUE must hold the questions of every hub
/// Herdr lists, each tagged with its hub, and none from the folder of a
/// hub that is gone. A worker that reported to the second hub has
/// reported, so it must not wait on the user as one that never
/// reported.
#[test]
fn print_board_gathers_and_tags_live_hubs() {
    let mut test_tools = TestTools::new();
    let runs = [
        ("herdr-hub.one", "kanban-hub-1", "Deploy tonight?"),
        ("herdr-hub.two", "auto-hub-2", "Merge the docs PR?"),
        ("herdr-hub.gone", "kanban-hub-0", "Rename the bucket?"),
    ];
    for (folder, hub, question) in runs {
        let run = test_tools.path().join(folder);
        fs::create_dir(&run).unwrap();
        let started =
            format!(r#"{{"at":"2026-11-12T08:00:00Z","event":"watch_started","hub":"{hub}"}}"#);
        fs::write(run.join("events.jsonl"), started + "\n").unwrap();
        fs::write(
            run.join("decisions.md"),
            format!("## {question}\n\nPick one.\n"),
        )
        .unwrap();
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
    let hubs = Hubs::Live(test_tools.path().to_owned());

    let outcome = board::print_board(&test_tools.tools(), &hubs, &TimeZone::UTC, PHONE).unwrap();

    let screen = outcome.lines.join("\n");
    assert!(outcome.lines[5].starts_with("QUEUE  2 you"), "{screen}");
    let row = |question: &str| {
        let line = outcome.lines.iter().find(|line| line.contains(question));
        line.map(|line| line.split_whitespace().last().unwrap_or_default())
    };
    assert_eq!(row("Deploy tonight?"), Some("kanban"), "{screen}");
    assert_eq!(row("Merge the docs PR?"), Some("auto-h"), "{screen}");
    assert_eq!(row("Rename the bucket?"), None, "{screen}");
}

/// A `notes.md` entry from `ago` before now, under its UTC day's
/// heading, such as `## 2026-11-15` and `- 0905 TEXT`.
fn note_entry(ago: SignedDuration, text: &str) -> String {
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
/// AWAY must start at the time the live board saved, and the top line
/// must say how long the user was away. A board that skipped the saved
/// time would show the agent old news as new.
#[test]
fn print_board_away_starts_at_saved_time() {
    let test_tools = TestTools::new();
    let notes = note_entry(SignedDuration::from_mins(120), "parked old-search")
        + &note_entry(SignedDuration::from_mins(30), "launched api-build");
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
    assert!(outcome.lines[0].starts_with("away 1h30m · "), "{screen}");
    assert!(outcome.lines[1].starts_with("AWAY  1 new "), "{screen}");
    assert!(
        outcome.lines[2].ends_with(" launched api-build"),
        "{screen}"
    );
}

/// A hub that ended, or handed off to a hub in a new run folder, is gone
/// from Herdr, but what it did while the user was away is still news.
/// AWAY must list its notes beside the live hub's once a board saved a
/// time. With none, AWAY shows the newest events, which a hub that
/// ended long ago must not fill.
#[test]
fn print_board_reads_gone_hubs_once_saved() {
    let mut test_tools = TestTools::new();
    let runs = [
        ("herdr-hub.live", "kanban-hub-2", 20, "launched api-build"),
        ("herdr-hub.gone", "kanban-hub-1", 40, "parked old-search"),
    ];
    for (folder, hub, minutes_ago, note) in runs {
        let run = test_tools.path().join(folder);
        fs::create_dir(&run).unwrap();
        let started =
            format!(r#"{{"at":"2026-11-12T08:00:00Z","event":"watch_started","hub":"{hub}"}}"#);
        fs::write(run.join("events.jsonl"), started + "\n").unwrap();
        let entry = note_entry(SignedDuration::from_mins(minutes_ago), note);
        fs::write(run.join("notes.md"), entry).unwrap();
    }
    test_tools
        .herdr
        .agents
        .push(make_agent("kanban-hub-2", AgentStatus::Idle));
    let hubs = Hubs::Live(test_tools.path().to_owned());

    let unsaved = board::print_board(&test_tools.tools(), &hubs, &TimeZone::UTC, PHONE).unwrap();
    let seen = Timestamp::now()
        .checked_sub(SignedDuration::from_mins(60))
        .unwrap();
    let hub_state = test_tools.state_folder().join("hub");
    fs::create_dir_all(&hub_state).unwrap();
    fs::write(hub_state.join("board-seen"), format!("{seen}\n")).unwrap();
    let saved = board::print_board(&test_tools.tools(), &hubs, &TimeZone::UTC, PHONE).unwrap();

    let unsaved_screen = unsaved.lines.join("\n");
    assert!(
        unsaved.lines[2].ends_with(" launched api-build"),
        "{unsaved_screen}"
    );
    assert!(
        !unsaved_screen.contains("parked old-search"),
        "{unsaved_screen}"
    );
    let screen = saved.lines.join("\n");
    assert!(saved.lines[1].starts_with("AWAY  2 new "), "{screen}");
    assert!(saved.lines[2].ends_with(" launched api-build"), "{screen}");
    assert!(saved.lines[3].ends_with(" parked old-search"), "{screen}");
}

/// With no hub running, as on a fresh login, the board must still show
/// the cards from the vault, and NEXT must say no hub runs. An error
/// there leaves the user with no board at all, and an empty line reads
/// as if nothing waits.
#[test]
fn print_board_hubless_shows_cards_and_note() {
    let test_tools = tools_with_todo_card();
    let hubs = Hubs::Live(test_tools.path().to_owned());

    let outcome = board::print_board(&test_tools.tools(), &hubs, &TimeZone::UTC, PHONE).unwrap();

    let next = outcome
        .lines
        .iter()
        .position(|line| line.starts_with("NEXT"));
    let next = next.unwrap();
    assert_eq!(outcome.lines[next + 1], " you: no hub running");
    assert_eq!(outcome.lines[next + 3], " hub: start DEMO-KB-007");
}

/// The one criterion in the note of `tools_with_todo_card`.
const CRITERION: &str = "- [ ] A failed upload retries.";

/// A hub with one card ready to start, DEMO-KB-007, whose note holds
/// `CRITERION`.
fn tools_with_todo_card() -> TestTools {
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
    let test_tools = tools_with_todo_card();

    let steps =
        "tap block:queue\ntap group:todo\ntap card:DEMO-KB-007\npress q\npress q\npress q\n";
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
        ("tap block:queue", "list"),
        ("tap group:todo", "list"),
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
    let test_tools = tools_with_todo_card();

    let steps = "tap block:queue\ntap group:todo\ntap card:DEMO-KB-007\n";
    let (result, screens) = control(&test_tools, steps);

    result.unwrap();
    let details = &screens[3]["lines"];
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
