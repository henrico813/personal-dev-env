mod common;

use std::fs;
use std::io;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use hub::commands::cli::find_run_folder;
use hub::shared::HubError;

use common::{TempFolder, read_script_calls};

/// Scripts and hubs tell a tool failure (1), wrong usage (2), and a
/// stopped command (3) apart by exit code, and a tool failure also
/// shows Herdr's or `card`'s own words. The `no-report` case finds its
/// run folder through `HUB_RUN` alone, as a worker's pane gives it.
/// The `auto-other-reply` case also checks that `hub auto approve`
/// takes the run number before the reply. The `auto-no-run` case
/// checks that `hub auto` steps never search for a folder, since a
/// search would write into the user's usual hub's folder.
#[test]
fn hub_exit_code_follows_failure_kind() {
    struct Case {
        name: &'static str,
        herdr_stdout: &'static str,
        herdr_exit_code: i32,
        args: &'static [&'static str],
        /// Sets `HUB_RUN` to an empty run folder in the child.
        hub_run: bool,
        exit_code: i32,
        says: &'static str,
    }
    let cases = [
        Case {
            name: "herdr",
            herdr_stdout: r#"{"error":{"code":"server_unavailable","message":"herdr server is not running"}}"#,
            herdr_exit_code: 1,
            args: &["flag", "docs-fix"],
            hub_run: false,
            exit_code: 1,
            says: "hub: herdr server is not running\n",
        },
        Case {
            name: "status-herdr",
            herdr_stdout: r#"{"error":{"code":"server_unavailable","message":"herdr server is not running"}}"#,
            herdr_exit_code: 1,
            args: &["status"],
            hub_run: true,
            exit_code: 1,
            says: "hub: herdr server is not running\n",
        },
        Case {
            name: "no-hub",
            herdr_stdout: r#"{"result":{"agents":[],"workspaces":[]}}"#,
            herdr_exit_code: 0,
            args: &["go"],
            hub_run: false,
            exit_code: 3,
            says: "hub: no hub is running\n",
        },
        Case {
            name: "bad-flag",
            herdr_stdout: "",
            herdr_exit_code: 0,
            args: &["flag", "--bogus", "docs-fix"],
            hub_run: false,
            exit_code: 2,
            says: "--bogus",
        },
        Case {
            name: "bad-name",
            herdr_stdout: "",
            herdr_exit_code: 0,
            args: &["flag", "Docs/Fix"],
            hub_run: false,
            exit_code: 2,
            says: "hub: Docs/Fix is not an agent name\n",
        },
        Case {
            name: "machine-on-go",
            herdr_stdout: "",
            herdr_exit_code: 0,
            args: &["--machine", "laptop", "go"],
            hub_run: false,
            exit_code: 2,
            says: "hub: go works on this machine only\n",
        },
        Case {
            name: "no-report",
            herdr_stdout: "",
            herdr_exit_code: 0,
            args: &["read", "docs-fix"],
            hub_run: true,
            exit_code: 3,
            says: "hub: no unread report from docs-fix\n",
        },
        Case {
            name: "send-gone",
            herdr_stdout: r#"{"error":{"code":"agent_not_found","message":"agent target docs-fix not found"}}"#,
            herdr_exit_code: 1,
            args: &["send", "docs-fix", "Rebase on main."],
            hub_run: true,
            exit_code: 1,
            says: "hub: not sent docs-fix: the session is gone\n",
        },
        Case {
            name: "send-no-text",
            herdr_stdout: "",
            herdr_exit_code: 0,
            args: &["send", "docs-fix"],
            hub_run: true,
            exit_code: 2,
            says: "MESSAGE",
        },
        Case {
            name: "no-run-folder",
            herdr_stdout: "",
            herdr_exit_code: 0,
            args: &["note", "merged #9"],
            hub_run: false,
            exit_code: 2,
            says: "hub: no run folder; no hub here wrote events yet, so pass --run DIR\n",
        },
        Case {
            name: "auto-no-run",
            herdr_stdout: "",
            herdr_exit_code: 0,
            args: &["auto", "now"],
            hub_run: false,
            exit_code: 2,
            says: "hub: no run folder; pass --run DIR or set HUB_RUN\n",
        },
        Case {
            name: "auto-ready-card",
            herdr_stdout: "",
            herdr_exit_code: 0,
            args: &["auto", "ready"],
            hub_run: true,
            exit_code: 1,
            says: "hub: card: no default vault\n",
        },
        Case {
            name: "auto-other-reply",
            herdr_stdout: "",
            herdr_exit_code: 0,
            args: &["auto", "approve", "2", "go"],
            hub_run: true,
            exit_code: 3,
            says: "hub: the reply is not 'approve run 2'; nothing was approved\n",
        },
    ];

    for case in cases {
        let folder = TempFolder::new();
        folder.write_script("herdr", case.herdr_stdout, "", case.herdr_exit_code);
        // Every case gets a `card` that fails as it does without a vault.
        folder.write_script("card", "", "card: no default vault\n", 1);

        // The child gets only these variables, so the user's own Herdr
        // is never reached.
        let mut command = Command::new(env!("CARGO_BIN_EXE_hub"));
        command
            .args(case.args)
            .env_clear()
            .env("PATH", format!("{}:/usr/bin:/bin", folder.path().display()))
            .env("HOME", folder.path())
            .env("HERDR_SOCKET_PATH", folder.path().join("herdr.sock"));
        if case.hub_run {
            let run = folder.path().join("run");
            fs::create_dir(&run).unwrap();
            command.env("HUB_RUN", run);
        }
        let output = command.output().unwrap();

        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(case.exit_code), "{}", case.name);
        assert!(stderr.contains(case.says), "{}: {stderr}", case.name);
    }
}

/// A hub restarts `hub next` after every line, and a restart must take
/// over from any older `hub next` still waiting. An older one that
/// keeps running holds the watcher lock, so the new one never checks
/// and the hub misses every report.
#[test]
fn hub_next_newer_watcher_replaces_older() {
    let folder = TempFolder::new();
    folder.write_script("herdr", r#"{"result":{"agents":[]}}"#, "", 0);
    let run = folder.path().join("run");
    fs::create_dir(&run).unwrap();
    // The children get only these variables, so the user's own Herdr
    // is never reached.
    let start_next = |timeout: &str| {
        Command::new(env!("CARGO_BIN_EXE_hub"))
            .args(["next", "--timeout", timeout])
            .env_clear()
            .env("PATH", format!("{}:/usr/bin:/bin", folder.path().display()))
            .env("HOME", folder.path())
            .env("HUB_RUN", &run)
            .env("HUB_POLL_SECONDS", "0.1")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap()
    };

    // Start the first watcher and wait until it has claimed the run
    // folder; its timeout only ends the test if nothing replaces it.
    let older = start_next("20");
    let claim = run.join(".watcher-pid");
    let started = Instant::now();
    while fs::read_to_string(&claim).unwrap_or_default().trim() != older.id().to_string() {
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "older never claimed"
        );
        thread::sleep(Duration::from_millis(20));
    }
    // Start the second, which runs one check once it has the lock.
    let newer = start_next("0");
    let older = older.wait_with_output().unwrap();
    let newer = newer.wait_with_output().unwrap();

    assert_eq!(
        String::from_utf8_lossy(&older.stdout),
        "replaced by a newer hub next\n"
    );
    assert!(older.status.success());
    assert_eq!(String::from_utf8_lossy(&newer.stdout), "timeout (hub -)\n");
    assert!(newer.status.success());
}

/// A worker's `hub report` that writes to the wrong run folder never
/// reaches its hub. So `--run` wins over `HUB_RUN`, and both win over
/// the folder of the hub running on this machine.
#[test]
fn find_run_folder_prefers_flag_then_env() {
    let folder = TempFolder::new();
    let flag_run = folder.path().join("flag-run");
    let env_run = folder.path().join("env-run");
    fs::create_dir(&flag_run).unwrap();
    fs::create_dir(&env_run).unwrap();
    let local_state = folder.path().join("state");
    fs::create_dir(&local_state).unwrap();
    let _live = hold_watcher_lock(&local_state.join("herdr-hub.live"));
    let cases = [
        (
            "flag-and-env",
            Some(flag_run.as_path()),
            Some(env_run.as_os_str()),
            &flag_run,
        ),
        ("env-only", None, Some(env_run.as_os_str()), &env_run),
    ];

    for (case_name, given, hub_run, expected) in cases {
        let found = find_run_folder(given, hub_run, Some(&local_state))
            .unwrap_or_else(|error| panic!("{case_name}: {error}"));

        assert_eq!(
            found.path(),
            std::path::absolute(expected).unwrap(),
            "{case_name}"
        );
    }
}

/// A mistyped `--run` is wrong usage, so the command stops with exit 2
/// rather than use the folder of the hub running on this machine.
#[test]
fn find_run_folder_rejects_missing_folder() {
    let folder = TempFolder::new();
    let missing = folder.path().join("missing");
    let local_state = folder.path().join("state");
    fs::create_dir(&local_state).unwrap();
    let _live = hold_watcher_lock(&local_state.join("herdr-hub.live"));

    let found = find_run_folder(Some(&missing), None, Some(&local_state));

    assert!(
        matches!(found, Err(HubError::Usage(_))),
        "{:?}",
        found.map(|run| run.path().to_owned())
    );
}

/// The desk's `hub board` pane has no `HUB_RUN`. The search must take
/// the folder whose `hub next` holds the lock, even when another run
/// wrote events later. Between `hub next` runs no lock is held, and
/// the board must still show the hub that worked last, not an older
/// finished run or an error. The two no-lock cases swap the ages, so
/// the order the folders are listed in cannot decide the answer.
#[test]
fn find_run_folder_picks_running_else_newest() {
    const HOUR: u64 = 60 * 60;
    struct Case {
        name: &'static str,
        running: Option<&'static str>,
        a_age_secs: u64,
        b_age_secs: u64,
        expected: &'static str,
    }
    let cases = [
        Case {
            name: "one-running",
            running: Some("herdr-hub.live"),
            a_age_secs: HOUR,
            b_age_secs: 2 * HOUR,
            expected: "herdr-hub.live",
        },
        Case {
            name: "a-newest",
            running: None,
            a_age_secs: HOUR,
            b_age_secs: 2 * HOUR,
            expected: "herdr-hub.a",
        },
        Case {
            name: "b-newest",
            running: None,
            a_age_secs: 2 * HOUR,
            b_age_secs: HOUR,
            expected: "herdr-hub.b",
        },
    ];

    for case in cases {
        let local_state = TempFolder::new();
        make_finished_run(&local_state.path().join("herdr-hub.a"), case.a_age_secs);
        make_finished_run(&local_state.path().join("herdr-hub.b"), case.b_age_secs);
        let _lock = case
            .running
            .map(|name| hold_watcher_lock(&local_state.path().join(name)));

        let found = find_run_folder(None, None, Some(local_state.path()))
            .unwrap_or_else(|error| panic!("{}: {error}", case.name));

        assert_eq!(
            found.path(),
            local_state.path().join(case.expected),
            "{}",
            case.name
        );
    }
}

/// During an auto run two hubs run, each with its own folder, and
/// picking one could show the wrong run. So the search names both
/// folders and asks for `--run`, rather than taking the folder used
/// last.
#[test]
fn find_run_folder_rejects_two_running_hubs() {
    let local_state = TempFolder::new();
    make_finished_run(&local_state.path().join("herdr-hub.last"), 60 * 60);
    let _auto = hold_watcher_lock(&local_state.path().join("herdr-hub.auto"));
    let _front = hold_watcher_lock(&local_state.path().join("herdr-hub.front"));

    let found = find_run_folder(None, None, Some(local_state.path()));

    match found {
        Err(HubError::Usage(message)) => {
            assert!(message.contains("herdr-hub.auto"), "{message}");
            assert!(message.contains("herdr-hub.front"), "{message}");
        }
        found => panic!("{:?}", found.map(|run| run.path().to_owned())),
    }
}

/// Makes the run folder `path` and locks its `.watcher.lock` as a
/// running `hub next` does, until the returned file closes.
fn hold_watcher_lock(path: &Path) -> fs::File {
    fs::create_dir(path).unwrap();
    let lock = fs::File::create(path.join(".watcher.lock")).unwrap();
    lock.lock().unwrap();
    lock
}

/// Makes a finished run folder at `path`, with a lock file nothing
/// holds and an `events.jsonl` last changed `age_secs` ago.
fn make_finished_run(path: &Path, age_secs: u64) {
    fs::create_dir(path).unwrap();
    fs::File::create(path.join(".watcher.lock")).unwrap();
    fs::File::create(path.join("events.jsonl"))
        .unwrap()
        .set_modified(SystemTime::now() - Duration::from_secs(age_secs))
        .unwrap();
}

/// A remote hub's `hub --machine laptop flag NAME` must mark the
/// session on that machine. Without `--machine` Herdr looks the name up
/// on this machine, so a local session with the same name gets the mark
/// instead.
#[test]
fn hub_flag_sends_machine_to_herdr() {
    let folder = TempFolder::new();
    let answer = r#"{"result":{"agent":{"name":"docs-fix","agent":"opencode",
        "agent_status":"idle","pane_id":"w4:p2","workspace_id":"w4","tab_id":"w4:t2",
        "focused":false,"state_change_seq":1288}}}"#;
    let script = folder.write_script("herdr", answer, "", 0);

    // The child gets only these variables, so the user's own Herdr is
    // never reached.
    let output = Command::new(env!("CARGO_BIN_EXE_hub"))
        .args(["--machine", "laptop", "flag", "docs-fix"])
        .env_clear()
        .env("PATH", format!("{}:/usr/bin:/bin", folder.path().display()))
        .env("HOME", folder.path())
        .env("HERDR_SOCKET_PATH", folder.path().join("herdr.sock"))
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{stderr}");
    assert_eq!(
        read_script_calls(&script)[0],
        ["--machine", "laptop", "agent", "get", "docs-fix"]
    );
}

/// A reader that closes early, as `head` does, must leave the exit code
/// at 0 with no panic text. The hub used to panic there and exit 101,
/// which is none of the hub's exit codes, so scripts saw a failure.
#[test]
fn hub_output_stops_quietly_when_reader_closes() {
    let folder = TempFolder::new();
    let answer = r#"{"result":{"agent":{"name":"docs-fix","agent":"opencode",
        "agent_status":"idle","pane_id":"w4:p2","workspace_id":"w4","tab_id":"w4:t2",
        "focused":false,"state_change_seq":1288}}}"#;
    folder.write_script("herdr", answer, "", 0);
    // Dropping the reading end first makes the hub's first write fail.
    let (reader, writer) = io::pipe().unwrap();
    drop(reader);

    let output = Command::new(env!("CARGO_BIN_EXE_hub"))
        .args(["flag", "docs-fix"])
        .env_clear()
        .env("PATH", format!("{}:/usr/bin:/bin", folder.path().display()))
        .env("HOME", folder.path())
        .env("HERDR_SOCKET_PATH", folder.path().join("herdr.sock"))
        .stdout(Stdio::from(writer))
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(0), "{stderr}");
    assert!(!stderr.contains("panicked"), "{stderr}");
}

/// The hub sends long messages with `hub send NAME --file PATH`. If
/// the file's text stops reaching Herdr whole, the worker gets an empty
/// or wrong message.
#[test]
fn hub_send_reads_message_from_file() {
    let folder = TempFolder::new();
    let answer = r#"{"result":{"agent":{"name":"docs-fix","agent":"opencode",
        "agent_status":"idle","pane_id":"w4:p2","workspace_id":"w4","tab_id":"w4:t2",
        "focused":false,"state_change_seq":1288}}}"#;
    let script = folder.write_script("herdr", answer, "", 0);
    let run = folder.path().join("run");
    fs::create_dir(&run).unwrap();
    let file = folder.path().join("msg.md");
    fs::write(&file, "Rebase on main.\nThen push.\n").unwrap();

    // The child gets only these variables, so the user's own Herdr is
    // never reached.
    let output = Command::new(env!("CARGO_BIN_EXE_hub"))
        .args(["send", "docs-fix", "--file"])
        .arg(&file)
        .env_clear()
        .env("PATH", format!("{}:/usr/bin:/bin", folder.path().display()))
        .env("HOME", folder.path())
        .env("HUB_RUN", &run)
        .env("HERDR_SOCKET_PATH", folder.path().join("herdr.sock"))
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{stderr}");
    let calls = read_script_calls(&script);
    let prompt = calls
        .iter()
        .find(|call| call.get(1).is_some_and(|word| word == "prompt"))
        .unwrap();
    assert_eq!(prompt[2..4], ["docs-fix", "Rebase on main.\nThen push.\n"]);
}

/// Runs `hub report` with these arguments and `input` on standard
/// input, with `HUB_RUN` set to `run`.
fn run_hub_report(home: &Path, run: &Path, args: &[&str], input: &str) -> Output {
    // The child gets only these variables, so the user's own run folder
    // is never reached.
    let mut child = Command::new(env!("CARGO_BIN_EXE_hub"))
        .arg("report")
        .args(args)
        .env_clear()
        .env("HOME", home)
        .env("HUB_RUN", run)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

/// Workers send a report as `hub report NAME FILE` or pipe it into
/// `hub report NAME -`. When either way stops reaching the run folder,
/// the worker's report never reaches its hub.
#[test]
fn hub_report_reads_named_file_and_stdin() {
    let folder = TempFolder::new();
    let run = folder.path().join("run");
    fs::create_dir(&run).unwrap();
    let file = folder.path().join("report.md");
    fs::write(&file, "From a file.\n").unwrap();
    let report = run.join("report-docs-fix.md");

    let from_file = run_hub_report(
        folder.path(),
        &run,
        &["docs-fix", file.to_str().unwrap()],
        "",
    );

    assert!(
        from_file.status.success(),
        "{}",
        String::from_utf8_lossy(&from_file.stderr)
    );
    assert_eq!(fs::read_to_string(&report).unwrap(), "From a file.\n");

    let from_stdin = run_hub_report(
        folder.path(),
        &run,
        &["docs-fix", "-"],
        "From standard input.\n",
    );

    assert!(
        from_stdin.status.success(),
        "{}",
        String::from_utf8_lossy(&from_stdin.stderr)
    );
    assert_eq!(
        fs::read_to_string(&report).unwrap(),
        "From standard input.\n"
    );
}
