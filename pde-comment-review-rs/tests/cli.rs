mod common;

use common::{REWORDED, TempFolder, make_program_command, make_repo, write_store};
use pde_comment_review::shared::git::resolve_commit;
use serde_json::{Value, json};

/// `make-feedback-request-list` must print the whole reply as JSON, with each Feedback Request's fields under `result.list`.
/// Otherwise every agent that reads the list, fills in its Agent Suggestions and sends it back would break.
#[test]
fn list_prints_feedback_requests_as_json() {
    let repo = make_repo();
    let dir = repo.path();
    write_store(dir, REWORDED, "");
    let base = resolve_commit(dir, "main").unwrap();

    let output = make_program_command(dir)
        .arg("make-feedback-request-list")
        .output()
        .unwrap();

    let reply: Value = serde_json::from_slice(&output.stdout).unwrap();
    let expected = json!({
        "status": null,
        "result": {
            "list": {
                "base": base.as_str(),
                "requests": [{
                    "id": 1,
                    "file": "review_store.go",
                    "name": "SaveAnswer",
                    "reason": "new",
                    "text": REWORDED,
                    "can_reword": true,
                    "line": 3,
                    "suggestion": null,
                }],
            },
            "unsupported": [],
        },
        "error": null,
        "next": "send_agent_suggestions",
    });
    assert_eq!(reply, expected);
}

/// `main` must exit with 0 without an error, 1 with an error code and 2 for bad arguments, and run `help` outside a Git checkout.
/// Otherwise scripts would read a refusal as success, and the installer would roll back every install, since it runs `help` after each one.
#[test]
fn exit_code_follows_error_and_arguments() {
    let repo = make_repo();
    let outside = TempFolder::new();
    let cases = [
        ("help-outside-checkout", outside.path(), vec!["help"], 0),
        ("no-error", repo.path(), vec!["show-status"], 0),
        ("error-code", outside.path(), vec!["show-status"], 1),
        ("unknown-command", repo.path(), vec!["approve"], 2),
        (
            "missing-file",
            repo.path(),
            vec!["send-agent-suggestions"],
            2,
        ),
    ];

    for (case, dir, args, exit_code) in cases {
        let output = make_program_command(dir).args(&args).output().unwrap();

        assert_eq!(output.status.code(), Some(exit_code), "{case}: {output:?}");
    }
}
