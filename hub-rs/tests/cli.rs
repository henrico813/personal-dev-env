mod common;

use std::io;
use std::process::{Command, Stdio};

use common::{TempFolder, read_script_calls};

/// Scripts and hubs act on the exit code. They must tell a tool failure
/// (1) from wrong usage (2) and from a command that stopped for a
/// reason (3). A tool failure must also show Herdr's own words.
#[test]
fn hub_exit_code_follows_failure_kind() {
    struct Case {
        name: &'static str,
        herdr_stdout: &'static str,
        herdr_exit_code: i32,
        args: &'static [&'static str],
        exit_code: i32,
        says: &'static str,
    }
    let cases = [
        Case {
            name: "herdr",
            herdr_stdout: r#"{"error":{"code":"server_unavailable","message":"herdr server is not running"}}"#,
            herdr_exit_code: 1,
            args: &["flag", "docs-fix"],
            exit_code: 1,
            says: "hub: herdr server is not running\n",
        },
        Case {
            name: "no-hub",
            herdr_stdout: r#"{"result":{"agents":[],"workspaces":[]}}"#,
            herdr_exit_code: 0,
            args: &["go"],
            exit_code: 3,
            says: "hub: no hub is running\n",
        },
        Case {
            name: "bad-flag",
            herdr_stdout: "",
            herdr_exit_code: 0,
            args: &["flag", "--bogus", "docs-fix"],
            exit_code: 2,
            says: "--bogus",
        },
        Case {
            name: "bad-name",
            herdr_stdout: "",
            herdr_exit_code: 0,
            args: &["flag", "Docs/Fix"],
            exit_code: 2,
            says: "hub: Docs/Fix is not an agent name\n",
        },
        Case {
            name: "machine-on-go",
            herdr_stdout: "",
            herdr_exit_code: 0,
            args: &["--machine", "laptop", "go"],
            exit_code: 2,
            says: "hub: go works on this machine only\n",
        },
    ];

    for case in cases {
        let folder = TempFolder::new();
        folder.write_script("herdr", case.herdr_stdout, "", case.herdr_exit_code);

        // The child gets only these variables, so the user's own Herdr
        // is never reached.
        let output = Command::new(env!("CARGO_BIN_EXE_hub"))
            .args(case.args)
            .env_clear()
            .env("PATH", format!("{}:/usr/bin:/bin", folder.path().display()))
            .env("HOME", folder.path())
            .env("HERDR_SOCKET_PATH", folder.path().join("herdr.sock"))
            .output()
            .unwrap();

        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(case.exit_code), "{}", case.name);
        assert!(stderr.contains(case.says), "{}: {stderr}", case.name);
    }
}

/// A remote hub's `hub --machine laptop flag NAME` must mark the
/// session on that machine. Without `--machine` Herdr looks the name up
/// on this machine, so a local session with the same name gets the mark
/// instead.
#[test]
fn hub_flag_sends_machine_to_herdr() {
    let folder = TempFolder::new();
    let answer = r#"{"result":{"agent":{"name":"docs-fix","agent_status":"idle",
        "pane_id":"w4:p2","workspace_id":"w4","focused":false}}}"#;
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
    let answer = r#"{"result":{"agent":{"name":"docs-fix","agent_status":"idle",
        "pane_id":"w4:p2","workspace_id":"w4","focused":false}}}"#;
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
