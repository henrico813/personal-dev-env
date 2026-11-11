mod common;

use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};

use hub::commands::cli::find_run_folder;
use hub::shared::HubError;

use common::{TempFolder, read_script_calls};

/// Scripts and hubs tell a tool failure (1), wrong usage (2), and a
/// stopped command (3) apart by exit code, and a tool failure also
/// shows Herdr's own words. The `no-report` case finds its run folder
/// through `HUB_RUN` alone, as a worker's pane gives it.
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
            name: "no-run-folder",
            herdr_stdout: "",
            herdr_exit_code: 0,
            args: &["note", "merged #9"],
            hub_run: false,
            exit_code: 2,
            says: "hub: no run folder; pass --run DIR or set HUB_RUN\n",
        },
    ];

    for case in cases {
        let folder = TempFolder::new();
        folder.write_script("herdr", case.herdr_stdout, "", case.herdr_exit_code);

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

/// A worker's `hub report` that writes to the wrong run folder never
/// reaches its hub, and a silent default would hide that. `--run` wins
/// over `HUB_RUN`, and a missing or wrong folder is wrong usage.
#[test]
fn find_run_folder_prefers_flag_over_env() {
    let folder = TempFolder::new();
    let flag_run = folder.path().join("flag-run");
    let env_run = folder.path().join("env-run");
    fs::create_dir(&flag_run).unwrap();
    fs::create_dir(&env_run).unwrap();
    let missing = folder.path().join("missing");
    let cases = [
        (
            "flag-and-env",
            Some(flag_run.as_path()),
            Some(env_run.as_os_str()),
            Some(&flag_run),
        ),
        ("env-only", None, Some(env_run.as_os_str()), Some(&env_run)),
        ("neither", None, None, None),
        ("missing-folder", Some(missing.as_path()), None, None),
    ];

    for (case_name, given, hub_run, expected) in cases {
        let found = find_run_folder(given, hub_run);

        match (found, expected) {
            (Ok(run), Some(expected)) => {
                assert_eq!(
                    run.path(),
                    std::path::absolute(expected).unwrap(),
                    "{case_name}"
                );
            }
            (Err(HubError::Usage(_)), None) => {}
            (found, _) => panic!("{case_name}: {:?}", found.map(|run| run.path().to_owned())),
        }
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
        "pane_id":"w4:p2","workspace_id":"w4","tab_id":"w4:t2","focused":false}}}"#;
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
