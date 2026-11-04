//! Builds two made-up repositories, each with a Review Session about the files
//! in `tests/samples/screen/`, and a waiting list that names both, all inside
//! the folder given as its one argument. `make try-screen` then opens them
//! with the real `open` command, so the screen can be tried on a phone without
//! touching real reviews or the real waiting list.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use serde_json::{Value, json};

use pde_comment_review::shared::comments::find_comments;

/// Holds one made-up review's folder, branch, agent session and requests.
type MadeUpReview = (&'static str, &'static str, &'static str, Vec<Value>);

fn main() -> ExitCode {
    let Some(folder) = env::args_os().nth(1).map(PathBuf::from) else {
        eprintln!("Usage: made_up_review FOLDER");
        return ExitCode::from(2);
    };
    let samples = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/samples/screen"));
    let mut worktrees = Vec::new();
    for (name, branch, agent_session, requests) in list_made_up_reviews() {
        let repo = folder.join(name);
        make_repo(samples, &repo, branch);
        save_review_session(&repo, branch, agent_session, requests);
        worktrees.push(fs::canonicalize(&repo).expect("the made-up repository exists"));
    }
    // The real open command reads this list, from the XDG_STATE_HOME that make try-screen sets.
    let waiting_list = folder.join("state/pde/comment-review");
    fs::create_dir_all(&waiting_list).expect("the made-up folder can be written");
    let saved = json!({"worktrees": worktrees});
    fs::write(
        waiting_list.join("waiting-list.json"),
        format!("{saved:#}\n"),
    )
    .expect("the made-up folder can be written");
    ExitCode::SUCCESS
}

/// Returns two made-up reviews from two agent sessions, so the Review Session
/// Picker shows first. Each request is `(file, reason, words, suggestion,
/// reasoning, also_at)`.
fn list_made_up_reviews() -> Vec<MadeUpReview> {
    let reword = |text: &str| json!({"reword": {"text": text}});
    vec![
        (
            "store-review",
            "feat/rust-review-screen-pieces",
            "ses_made_up_store",
            vec![
                json!([
                    "store.go",
                    "new",
                    "Saves the owner's answer",
                    reword(
                        "Saves one answer under the review's lock, so a second screen saving at the same moment loses none.\n\nThe file is written through a temporary file, so a crash leaves the old one whole."
                    ),
                    "Says why the lock matters instead of how the file is written.",
                    []
                ]),
                json!([
                    "store.go",
                    "new",
                    "the saved name",
                    "cut",
                    "Repeats the code. This one can only be kept, so a and c say why.",
                    []
                ]),
                json!([
                    "notes.py",
                    "new",
                    "Stores the note",
                    reword("Strips the note before saving it."),
                    "Says what the code does that its name does not.",
                    []
                ]),
                json!([
                    "notes.py",
                    "was_cut",
                    "Forgets every note",
                    "cut",
                    "Repeats the name. It was cut before, and Python needs it as the body.",
                    []
                ]),
            ],
        ),
        (
            "draw-review",
            "feat/draw-rows",
            "ses_made_up_draw",
            vec![
                json!([
                    "draw.rs",
                    "new",
                    "Draws the rows",
                    "keep",
                    "Explains the clear before each row, which 面 shows matters.",
                    ["src/screen.rs:40", "src/picker.rs:12"]
                ]),
                json!([
                    "draw.rs",
                    "new",
                    "Moves to the row",
                    "cut",
                    "Repeats the escape code.",
                    []
                ]),
            ],
        ),
    ]
}

/// Makes a Git repository at `repo` on `branch` with the sample files
/// committed, so `d` shows what the answers change.
fn make_repo(samples: &Path, repo: &Path, branch: &str) {
    fs::create_dir_all(repo).expect("the made-up folder can be written");
    for sample in ["store.go", "notes.py", "draw.rs"] {
        fs::copy(samples.join(sample), repo.join(sample))
            .expect("the sample files ship with the crate");
    }
    run_git(repo, &["init", "--quiet", "-b", branch]);
    run_git(repo, &["add", "--all"]);
    run_git(repo, &["commit", "--quiet", "-m", "Add the sample files"]);
}

/// Runs Git without the machine's own settings.
fn run_git(repo: &Path, args: &[&str]) {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_AUTHOR_NAME", "Made-up Review")
        .env("GIT_AUTHOR_EMAIL", "review@example.invalid")
        .env("GIT_COMMITTER_NAME", "Made-up Review")
        .env("GIT_COMMITTER_EMAIL", "review@example.invalid")
        .output()
        .expect("git runs");
    assert!(output.status.success(), "git {args:?} failed: {output:?}");
}

/// Saves a Review Session waiting for User Feedback, with one Feedback Request
/// for each sample comment, found by a piece of its text.
fn save_review_session(repo: &Path, branch: &str, agent_session: &str, requests: Vec<Value>) {
    let mut sent = Vec::new();
    for (index, request) in requests.into_iter().enumerate() {
        let [file, reason, words, kind, reasoning, also_at] = <[Value; 6]>::try_from(
            request
                .as_array()
                .expect("each made-up request is an array")
                .clone(),
        )
        .expect("each made-up request has six parts");
        let file = file.as_str().expect("the file is text");
        let words = words.as_str().expect("the words are text");
        let source = fs::read_to_string(repo.join(file)).expect("the sample file was copied");
        let comment = find_comments(Path::new(file), &source)
            .expect("the sample files parse")
            .into_iter()
            .find(|comment| comment.text.contains(words))
            .expect("each sample comment is in its file");
        sent.push(json!({
            "id": index + 1,
            "file": file,
            "name": comment.name,
            "reason": reason,
            "text": comment.text,
            "can_reword": comment.style.can_reword(),
            "line": comment.line,
            "suggestion": {"kind": kind, "reasoning": reasoning, "also_at": also_at},
        }));
    }
    let review_session = json!({
        "status": "waiting_for_user_feedback",
        "origin": {"repo": "personal-dev-env", "branch": branch, "agent_session": agent_session},
        "lists": [{"base": "4f2a9c1e", "requests": sent}],
        "feedback": {},
    });
    let folder = repo.join(".git/pde");
    fs::create_dir_all(&folder).expect("the made-up repository can be written");
    fs::write(folder.join("review.json"), format!("{review_session:#}\n"))
        .expect("the made-up repository can be written");
}
