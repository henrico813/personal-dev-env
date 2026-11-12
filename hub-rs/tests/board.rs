mod common;

use hub::board::{self, ScreenSize};
use hub::external::cards::{CardNote, Stage};
use hub::shared::HubError;
use jiff::tz::TimeZone;
use serde_json::Value;

use common::TestTools;

/// The phone's screen size.
const PHONE: ScreenSize = ScreenSize {
    columns: 40,
    rows: 16,
};

/// Runs `hub board --control` with `steps` as its input, and returns
/// its result and each screen it printed.
fn control(test_tools: &TestTools, steps: &str) -> (Result<(), HubError>, Vec<Value>) {
    let mut output = Vec::new();
    let result = board::control_board(
        &test_tools.tools(),
        &test_tools.run(),
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

/// Without a card vault every `card` call fails. The board must still
/// draw, with the failure in a Could not read group on top, or the user
/// gets an error in place of the questions and sessions that wait.
#[test]
fn print_board_shows_card_failure_on_top() {
    let mut test_tools = TestTools::new();
    test_tools.cards.read_error = Some("card: no default vault".to_owned());

    let outcome = board::print_board(
        &test_tools.tools(),
        &test_tools.run(),
        &TimeZone::UTC,
        PHONE,
    )
    .unwrap();

    assert_eq!(outcome.lines[1], "> Could not read 1");
}

/// An agent checks the board by tapping what the user taps. Each step
/// must print the screen it leads to, and a ready card, whose note the
/// read skips, must show its criteria, or the agent reports a screen the
/// user never sees.
#[test]
fn control_board_prints_screen_after_each_step() {
    let mut test_tools = TestTools::new();
    test_tools
        .cards
        .stages
        .insert("DEMO-KB-007".to_owned(), Stage::Ready);
    test_tools.cards.card_notes.insert(
        "DEMO-KB-007".to_owned(),
        CardNote {
            title: "Retry failed uploads".to_owned(),
            criteria: vec!["- [ ] A failed upload retries.".to_owned()],
            ..CardNote::default()
        },
    );

    let steps = "tap group:ready\ntap card:DEMO-KB-007\npress q\npress q\n";
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
        ("tap group:ready", "list"),
        ("tap card:DEMO-KB-007", "details"),
        ("press q", "list"),
        ("press q", "quit"),
    ];
    assert_eq!(pages, expected);
    let details = &screens[2]["lines"];
    assert!(
        details
            .as_array()
            .unwrap()
            .contains(&Value::from("- [ ] A failed upload retries."))
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
