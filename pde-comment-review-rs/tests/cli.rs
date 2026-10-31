mod common;

use std::process::Command;

use common::TempFolder;

/// Keeps install verification independent of a checkout or running screen.
#[test]
fn help_runs_without_review_services() {
    let folder = TempFolder::new();
    let output = Command::new(env!("CARGO_BIN_EXE_pde-comment-review-next"))
        .arg("help")
        .current_dir(folder.path())
        .env_clear()
        .env("HOME", folder.path())
        .env("XDG_STATE_HOME", folder.path())
        .env("PATH", "")
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let help = String::from_utf8(output.stdout).unwrap();
    assert!(help.contains("pde-comment-review-next"));
    assert!(help.contains("help"));
    assert_eq!(std::fs::read_dir(folder.path()).unwrap().count(), 0);
}

/// Stops scripts from mistaking an unavailable command for success.
#[test]
fn unknown_arguments_exit_two() {
    struct Case {
        args: &'static [&'static str],
    }
    let cases = [
        Case { args: &[] },
        Case {
            args: &["check-comments"],
        },
        Case {
            args: &["help", "extra"],
        },
    ];

    for case in cases {
        let output = Command::new(env!("CARGO_BIN_EXE_pde-comment-review-next"))
            .args(case.args)
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(2), "{:?}", case.args);
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8(output.stderr).unwrap().contains("Usage"));
    }
}
