mod common;

use hub::commands::alive::run_alive;
use hub::external::opencode::OpenCodeError;
use hub::shared::HubError;
use serde_json::json;

use common::{OpenCodeRow, TestTools, make_time, write_opencode_db};

/// The hub reads this line to tell the user whether a session still
/// works. The newest row decides the time, and the last tool comes
/// from the newest step that called one, even when newer rows called
/// none.
#[test]
fn run_alive_prints_time_and_last_tool() {
    let test_tools = TestTools::new();
    let now = make_time("2026-11-11T22:14:00+00:00[UTC]");
    let now_ms = now.timestamp().as_millisecond();
    write_opencode_db(
        &test_tools.opencode_path(),
        &[
            OpenCodeRow {
                session: "ses_a",
                kind: "assistant",
                updated_ms: now_ms - 20 * 60 * 1000,
                data: json!({"content": [{"type": "tool", "name": "shell"}]}),
            },
            OpenCodeRow {
                session: "ses_a",
                kind: "user",
                updated_ms: now_ms - 30 * 1000,
                data: json!({}),
            },
        ],
    );

    let outcome = run_alive(&test_tools.tools(), "ses_a", &now).unwrap();

    assert_eq!(
        outcome.lines,
        ["2 messages, newest 22:13:30, last tool shell, working"]
    );
}

/// A mistyped session ID must not read as a stopped session, or the
/// hub would tell the user a working session died.
#[test]
fn run_alive_unknown_session_fails() {
    let test_tools = TestTools::new();
    let now = make_time("2026-11-11T22:14:00+00:00[UTC]");
    write_opencode_db(
        &test_tools.opencode_path(),
        &[OpenCodeRow {
            session: "ses_a",
            kind: "user",
            updated_ms: 1,
            data: json!({}),
        }],
    );

    let error = run_alive(&test_tools.tools(), "ses_b", &now).unwrap_err();

    assert!(
        matches!(error, HubError::OpenCode(OpenCodeError::NoSession(_))),
        "{error:?}"
    );
}
