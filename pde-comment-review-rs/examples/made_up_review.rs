//! Opens the Review Screen on a made-up Review Session about the files in
//! `tests/samples/screen/`, to try the screen on a phone. Answers stay in
//! memory and are printed at the end, and nothing is saved.

use std::fs;
use std::path::Path;
use std::process::ExitCode;

use serde_json::{Value, json};

use pde_comment_review::review_screen::show_review_screen;
use pde_comment_review::shared::comments::find_comments;
use pde_comment_review::shared::review_session::ReviewSession;

fn main() -> ExitCode {
    let repo = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/samples/screen"));
    let mut review_session = make_review_session(repo);
    if let Err(error) = show_review_screen(repo, &mut review_session) {
        eprintln!("The review screen failed: {error}");
        return ExitCode::FAILURE;
    }
    for request in &review_session.lists[0].requests {
        let feedback = review_session.feedback.get(&request.id);
        println!("{}:{} {feedback:?}", request.file.display(), request.line);
    }
    ExitCode::SUCCESS
}

/// Returns a Review Session with one Feedback Request for each sample comment
/// below, found by a piece of its text.
fn make_review_session(repo: &Path) -> ReviewSession {
    let reword = |text: &str| json!({"reword": {"text": text}});
    let samples = [
        (
            "store.go",
            "new",
            "Saves the owner's answer",
            reword(
                "Saves one answer under the review's lock, so a second screen saving at the same moment loses none.\n\nThe file is written through a temporary file, so a crash leaves the old one whole.",
            ),
            "Says why the lock matters instead of how the file is written.",
            vec![],
        ),
        (
            "store.go",
            "new",
            "the saved name",
            json!("cut"),
            "Repeats the code. This one can only be kept, so a and c say why.",
            vec![],
        ),
        (
            "notes.py",
            "new",
            "Stores the note",
            reword("Strips the note before saving it."),
            "Says what the code does that its name does not.",
            vec![],
        ),
        (
            "notes.py",
            "was_cut",
            "Forgets every note",
            json!("cut"),
            "Repeats the name. It was cut before, and Python needs it as the body.",
            vec![],
        ),
        (
            "draw.rs",
            "new",
            "Draws the rows",
            json!("keep"),
            "Explains the clear before each row, which 面 shows matters.",
            vec!["src/screen.rs:40", "src/picker.rs:12"],
        ),
        (
            "draw.rs",
            "new",
            "Moves to the row",
            json!("cut"),
            "Repeats the escape code.",
            vec![],
        ),
    ];
    let mut requests = Vec::new();
    for (index, (file, reason, words, kind, reasoning, also_at)) in samples.into_iter().enumerate()
    {
        let source =
            fs::read_to_string(repo.join(file)).expect("the sample files ship with the crate");
        let comment = find_comments(Path::new(file), &source)
            .expect("the sample files parse")
            .into_iter()
            .find(|comment| comment.text.contains(words))
            .expect("each sample comment is in its file");
        requests.push(json!({
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
    let review_session: Value = json!({
        "status": "waiting_for_user_feedback",
        "origin": {
            "repo": "personal-dev-env",
            "branch": "feat/rust-review-screen-pieces",
            "agent_session": "made-up",
        },
        "lists": [{"base": "4f2a9c1e", "requests": requests}],
        "feedback": {"5": {"note": "Say what happens when the terminal is resized."}},
    });
    serde_json::from_value(review_session)
        .expect("the made-up Review Session matches the saved shape")
}
