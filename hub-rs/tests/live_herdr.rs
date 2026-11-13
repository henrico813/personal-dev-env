//! Opt-in tests that call the real `herdr`, since no other test talks
//! to a Herdr server. Each is marked `#[ignore]`. Run them by hand only
//! inside a scratch Herdr session, with
//! `cargo test --test live_herdr -- --ignored --test-threads=1`. They
//! make workspaces, start OpenCode agents that each answer one short
//! prompt, and close the tabs they made.

mod common;

use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

use hub::external::herdr::{AgentStart, Herdr, HerdrCli, HerdrCode, HerdrError, NewPlace};

use common::TempFolder;

/// The real `herdr` on `PATH`, on this machine.
fn connect_herdr() -> HerdrCli {
    let socket = env::var_os("HERDR_SOCKET_PATH").map_or_else(
        || PathBuf::from(env::var_os("HOME").unwrap()).join(".config/herdr/herdr.sock"),
        PathBuf::from,
    );
    HerdrCli::new(PathBuf::from("herdr"), None, socket)
}

/// Makes an unfocused workspace in `folder` with `HUB_RUN` set to it,
/// and returns its root pane and tab.
fn make_workspace(herdr: &HerdrCli, folder: &Path, label: &str) -> (String, String) {
    let place = NewPlace {
        label: label.to_owned(),
        cwd: folder.to_owned(),
        env: vec![("HUB_RUN".to_owned(), folder.display().to_string())],
    };
    let pane_id = herdr.create_workspace(&place).unwrap().root_pane_id;
    let tab_id = herdr
        .list_panes()
        .unwrap()
        .into_iter()
        .find(|pane| pane.pane_id == pane_id)
        .unwrap()
        .tab_id;
    (pane_id, tab_id)
}

/// Waits up to 90 seconds until Herdr shows the agent's session ID,
/// and returns it.
fn wait_for_session(herdr: &HerdrCli, name: &str) -> String {
    let started = Instant::now();
    loop {
        if let Some(session) = herdr.get_agent(name).unwrap().session_id {
            return session;
        }
        assert!(
            started.elapsed() < Duration::from_secs(90),
            "Herdr never showed a session for {name}"
        );
        thread::sleep(Duration::from_secs(1));
    }
}

/// `hub handoff` and reopening make a tab and start OpenCode in it,
/// `hub send` prompts it, and the tidy pass closes its tab. Each call
/// works against the saved answers, and only a real server shows
/// whether the arguments still match Herdr's.
#[test]
#[ignore = "needs a scratch Herdr session"]
fn herdr_cli_starts_prompts_and_closes_tab() {
    let folder = TempFolder::new();
    let herdr = connect_herdr();
    let (pane_id, tab_id) = make_workspace(&herdr, folder.path(), "hub-live-1");

    herdr
        .start_agent(&AgentStart {
            name: "hub-live-1".to_owned(),
            pane_id,
            args: Vec::new(),
        })
        .unwrap();
    herdr
        .prompt("hub-live-1", "Reply with the word ok.")
        .unwrap();
    herdr.close_tab(&tab_id).unwrap();

    let error = herdr.get_agent("hub-live-1").unwrap_err();
    assert!(
        matches!(
            error,
            HerdrError::Refused {
                code: HerdrCode::AgentNotFound,
                ..
            }
        ),
        "{error}"
    );
}

/// Reopening starts OpenCode with `--session ID`. A launcher that drops
/// the flag starts an empty session, so Herdr must show the same ID
/// after the start. When run by hand, the user also compares the model
/// and effort in the reopened pane, which no test can see.
#[test]
#[ignore = "needs a scratch Herdr session"]
fn herdr_cli_resumes_session_by_id() {
    let folder = TempFolder::new();
    let herdr = connect_herdr();
    // Start a session and give it one turn, so OpenCode saves it.
    let (pane_id, tab_id) = make_workspace(&herdr, folder.path(), "hub-live-2");
    let start = |pane_id: String, args: Vec<String>| AgentStart {
        name: "hub-live-2".to_owned(),
        pane_id,
        args,
    };
    herdr.start_agent(&start(pane_id, Vec::new())).unwrap();
    herdr
        .prompt("hub-live-2", "Reply with the word ok.")
        .unwrap();
    let session = wait_for_session(&herdr, "hub-live-2");
    herdr.close_tab(&tab_id).unwrap();
    // Resume it in a new workspace, as reopening does.
    let (pane_id, tab_id) = make_workspace(&herdr, folder.path(), "hub-live-2");
    let args = vec!["--session".to_owned(), session.clone(), "--auto".to_owned()];

    herdr.start_agent(&start(pane_id, args)).unwrap();
    let resumed = wait_for_session(&herdr, "hub-live-2");

    herdr.close_tab(&tab_id).unwrap();
    assert_eq!(resumed, session);
}

/// Reopening into a new workspace sets `HUB_RUN` with
/// `workspace create --env`. Without it the reopened worker's
/// `hub report` cannot find the run folder, and its report never
/// reaches the hub.
#[test]
#[ignore = "needs a scratch Herdr session"]
fn herdr_cli_sets_env_in_new_workspace() {
    let folder = TempFolder::new();
    let herdr = connect_herdr();
    let (pane_id, tab_id) = make_workspace(&herdr, folder.path(), "hub-live-3");
    let expected = format!("hub-env-{}", folder.path().display());

    let ran = Command::new("herdr")
        .args(["pane", "run", &pane_id, "echo hub-env-$HUB_RUN"])
        .status()
        .unwrap();
    let seen = Command::new("herdr")
        .args(["pane", "wait-output", &pane_id, "--match", &expected])
        .args(["--timeout", "10000"])
        .status()
        .unwrap();

    herdr.close_tab(&tab_id).unwrap();
    assert!(ran.success());
    assert!(seen.success(), "the pane never printed {expected}");
}
