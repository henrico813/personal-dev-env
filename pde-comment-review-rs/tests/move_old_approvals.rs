mod common;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use common::{TempFolder, find_waiting_worktrees, make_program_command, run_git};
use pde_comment_review::shared::approvals::{self, DecisionMatch};
use pde_comment_review::shared::comments::{Comment, CommentStyle};
use pde_comment_review::shared::review_session::{self, ReviewStatus};
use pde_comment_review::shared::user_feedback::{UserAnswer, UserFeedback};
use serde_json::{Value, json};

/// Names the Go file the made-up clone holds.
const STORE: &str = "review_store.go";

/// Holds three commented Go functions, as main and the worktree hold them.
const STORE_SOURCE: &str = "package review

// Saves the answer under the lock.
func SaveAnswer() {}

// Loads the answer.
func LoadAnswer() {}

// Drops the answer.
func DropAnswer() {}
";

/// Holds a made-up clone with one worktree, and a folder for the old review tool's state.
struct Clone {
    _folder: TempFolder,
    main: PathBuf,
    worktree: PathBuf,
    state: PathBuf,
}

/// Makes a clone whose main checkout and one worktree hold `STORE_SOURCE`.
fn make_clone() -> Clone {
    let folder = TempFolder::new();
    let main = folder.path().join("main");
    let worktree = folder.path().join("worktree");
    let state = folder.path().join("state");
    fs::create_dir_all(&main).unwrap();
    run_git(&main, &["init", "-b", "main"]);
    fs::write(main.join(STORE), STORE_SOURCE).unwrap();
    run_git(&main, &["add", "--all"]);
    run_git(&main, &["commit", "-m", "Add the store"]);
    let worktree_arg = worktree.to_str().unwrap();
    run_git(
        &main,
        &["worktree", "add", "-b", "review-comments", worktree_arg],
    );
    Clone {
        _folder: folder,
        main,
        worktree,
        state,
    }
}

/// Writes an old checker's `.code-comments.json` at the top of `checkout`.
fn write_old_approvals(checkout: &Path, entries: &[Value], avoid: Value) {
    let file = json!({"approved": entries, "avoid": avoid});
    fs::write(checkout.join(".code-comments.json"), file.to_string()).unwrap();
}

/// Returns one old approvals entry for the store file, a cut when `text` is empty.
fn old_entry(name: &str, text: &str) -> Value {
    json!({"file": STORE, "name": name, "text": text})
}

/// Runs `move-old-approvals` in main with the clone's own state folder, and returns its exit code and reply.
fn run_move(clone: &Clone) -> (i32, Value) {
    let output = make_program_command(&clone.main)
        .arg("move-old-approvals")
        .env("XDG_STATE_HOME", &clone.state)
        .output()
        .unwrap();
    let reply = serde_json::from_slice(&output.stdout).unwrap();
    (output.status.code().unwrap(), reply)
}

/// Builds the store file's comment as the code holds it now.
fn store_comment(name: &str, text: &str) -> Comment {
    Comment {
        file: PathBuf::from(STORE),
        name: Some(name.to_owned()),
        line: 1,
        end_line: 1,
        style: CommentStyle::Line,
        text: text.to_owned(),
        byte_range: 0..text.len(),
        can_cut: true,
    }
}

/// Writes one file of the old review tool's review `name`, as it saves them.
fn write_old_review_file(clone: &Clone, name: &str, file: &str, contents: Value) {
    let folder = clone.state.join("pde/comment-review").join(name);
    fs::create_dir_all(&folder).unwrap();
    fs::write(folder.join(file), contents.to_string()).unwrap();
}

/// Writes an open old review of the worktree with three requests, one edited, one noted and one unanswered.
fn write_open_review(clone: &Clone) {
    let items = json!([
        {"id": 1, "file": STORE, "old": "// Saves the answer under the lock.\n", "proposal": "keep", "name": "SaveAnswer"},
        {"id": 2, "file": STORE, "old": "// Loads the answer.\n", "proposal": "reword", "new": "// Loads the saved answer.\n", "why": "Says what it loads.", "name": "LoadAnswer"},
        {"id": 3, "file": STORE, "old": "// Drops the answer.\n", "proposal": "cut"},
    ]);
    let round = json!({"version": 1, "repo": clone.worktree, "items": items});
    write_old_review_file(clone, "review-comments", "round-1.json", round);
    let answers = json!({"1": {"action": "edited", "text": "// Saves each answer under the lock.\n", "round": 1}});
    write_old_review_file(clone, "review-comments", "decisions.json", answers);
    let notes = json!({"2": {"text": "Say where it loads from.", "round": 1}});
    write_old_review_file(clone, "review-comments", "notes.json", notes);
}

/// Reads every file under `folder`, by path, to see later whether any changed.
fn read_files_under(folder: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut files = BTreeMap::new();
    for entry in fs::read_dir(folder).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            files.extend(read_files_under(&path));
        } else {
            files.insert(path.clone(), fs::read(&path).unwrap());
        }
    }
    files
}

/// `move_old_approvals` must bring every decision and avoided word from main's and each worktree's old file.
/// Otherwise an approval or cut the user made in a worktree would stop counting after the switch-over.
#[test]
fn move_old_approvals_moves_every_worktree_copy() {
    let clone = make_clone();
    let main_entries = [old_entry(
        "SaveAnswer",
        "// Saves the answer under the lock.\n",
    )];
    write_old_approvals(
        &clone.main,
        &main_entries,
        json!({"client": "say customer"}),
    );
    write_old_approvals(&clone.worktree, &[old_entry("LoadAnswer", "")], json!({}));

    let (exit_code, _) = run_move(&clone);

    assert_eq!(exit_code, 0);
    let moved = approvals::load(&clone.main).unwrap();
    let saved = store_comment("SaveAnswer", "// Saves the answer under the lock.\n");
    let loaded = store_comment("LoadAnswer", "// Loads the answer.\n");
    assert_eq!(moved.compare(&saved), DecisionMatch::Approved);
    assert_eq!(moved.compare(&loaded), DecisionMatch::WasCut);
    let saved_file = fs::read(clone.main.join(".git/pde/code-comments.json")).unwrap();
    let saved_file: Value = serde_json::from_slice(&saved_file).unwrap();
    assert_eq!(saved_file["avoid"], json!({"client": "say customer"}));
}

/// `move_old_approvals` must report each old file's entries and how many it moved, repeated or found already there.
/// Otherwise the user could not check by counting that no approval was lost.
#[test]
fn move_old_approvals_reports_counts_to_compare() {
    // Main and the worktree share one entry, and a second worktree's folder is gone.
    let clone = make_clone();
    let shared = old_entry("SaveAnswer", "// Saves the answer under the lock.\n");
    write_old_approvals(
        &clone.main,
        &[shared.clone(), old_entry("DropAnswer", "")],
        json!({}),
    );
    write_old_approvals(
        &clone.worktree,
        &[shared, old_entry("LoadAnswer", "")],
        json!({}),
    );
    let gone = clone.main.parent().unwrap().join("gone");
    run_git(
        &clone.main,
        &["worktree", "add", "-b", "gone", gone.to_str().unwrap()],
    );
    fs::remove_dir_all(&gone).unwrap();

    let (_, reply) = run_move(&clone);

    let result = &reply["result"];
    let entries: Vec<&Value> = result["old_files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|file| &file["entries"])
        .collect();
    assert_eq!(entries, [&json!(2), &json!(2)]);
    assert!(
        result["missing_worktrees"][0]
            .as_str()
            .unwrap()
            .ends_with("gone")
    );
    let counts = &result["approvals"];
    assert_eq!(counts["entries_before"], 0);
    assert_eq!(counts["entries_moved"], 3);
    assert_eq!(counts["entries_repeated"], 1);
    assert_eq!(counts["entries_already_there"], 0);
    assert_eq!(counts["entries_after"], 3);
}

/// `move_old_approvals` run a second time must add no entry, leave the approvals file as it was and say it wrote nothing.
/// Otherwise a repeated move would fill the file with copies, or its undo would delete the approvals file.
#[test]
fn move_old_approvals_twice_writes_nothing() {
    let clone = make_clone();
    write_old_approvals(
        &clone.main,
        &[old_entry(
            "SaveAnswer",
            "// Saves the answer under the lock.\n",
        )],
        json!({}),
    );
    run_move(&clone);
    let approvals_file = clone.main.join(".git/pde/code-comments.json");
    let after_first_move = fs::read(&approvals_file).unwrap();

    let (_, reply) = run_move(&clone);

    assert_eq!(reply["result"]["approvals"]["entries_moved"], 0);
    assert_eq!(reply["result"]["approvals"]["entries_already_there"], 1);
    assert_eq!(fs::read(&approvals_file).unwrap(), after_first_move);
    assert_eq!(reply["result"]["approvals"]["backup"], "nothing_written");
}

/// `move_old_approvals` must stop with `saved_file_unreadable` on an old file it cannot read, and change nothing.
/// Otherwise the approvals in that file would be dropped without the user knowing.
#[test]
fn move_old_approvals_stops_unchanged_on_damaged_copy() {
    let clone = make_clone();
    common::add_decision(&clone.main, STORE, "SaveAnswer", "");
    let approvals_file = clone.main.join(".git/pde/code-comments.json");
    let before = fs::read(&approvals_file).unwrap();
    write_old_approvals(&clone.main, &[old_entry("LoadAnswer", "")], json!({}));
    fs::write(
        clone.worktree.join(".code-comments.json"),
        "{\"approved\": [",
    )
    .unwrap();

    let (exit_code, reply) = run_move(&clone);

    assert_eq!(exit_code, 1);
    assert_eq!(reply["error"]["code"], "saved_file_unreadable");
    assert_eq!(fs::read(&approvals_file).unwrap(), before);
}

/// `move_old_approvals` must save the approvals file as it was before the move, and name that copy.
/// Otherwise the user could not undo a move that went wrong.
#[test]
fn move_old_approvals_backs_up_file_before_move() {
    let clone = make_clone();
    common::add_decision(&clone.main, STORE, "SaveAnswer", "");
    let before = fs::read(clone.main.join(".git/pde/code-comments.json")).unwrap();
    write_old_approvals(&clone.main, &[old_entry("LoadAnswer", "")], json!({}));

    let (_, reply) = run_move(&clone);

    let backup = reply["result"]["approvals"]["backup"]["saved_at"]
        .as_str()
        .unwrap();
    assert_eq!(fs::read(backup).unwrap(), before);
}

/// `move_old_approvals` must say `no_file_before` when it made the approvals file.
/// Otherwise the undo, which deletes the file only then, would leave every moved entry in place on the user's first run.
#[test]
fn move_old_approvals_reports_no_file_before() {
    let clone = make_clone();
    write_old_approvals(&clone.main, &[old_entry("LoadAnswer", "")], json!({}));

    let (_, reply) = run_move(&clone);

    assert_eq!(reply["result"]["approvals"]["backup"], "no_file_before");
}

/// `move_old_approvals` must name the backup and each review it could not save, and exit 1, when a review fails after the approvals were written.
/// Otherwise the user could not tell which backup undoes the move or which reviews moved.
#[test]
fn move_old_approvals_fails_but_names_backup() {
    // The approvals file exists, and the worktree's Review Session file cannot be read.
    let clone = make_clone();
    common::add_decision(&clone.main, STORE, "SaveAnswer", "");
    write_old_approvals(&clone.main, &[old_entry("LoadAnswer", "")], json!({}));
    write_open_review(&clone);
    fs::create_dir_all(clone.main.join(".git/worktrees/worktree/pde/review.json")).unwrap();

    let (exit_code, reply) = run_move(&clone);

    assert_eq!(exit_code, 1);
    let result = &reply["result"];
    assert!(result["approvals"]["backup"]["saved_at"].is_string());
    let outcome = &result["reviews"][0]["outcome"];
    assert_eq!(
        outcome["not_saved"]["error"]["code"],
        "saved_file_unreadable"
    );
}

/// `move_old_approvals` must leave the old approvals files and the old review tool's files as they were.
/// Otherwise today's tools, which keep running until the switch-over, would lose their own approvals and reviews.
#[test]
fn move_old_approvals_leaves_old_files_unchanged() {
    let clone = make_clone();
    write_old_approvals(
        &clone.main,
        &[old_entry("SaveAnswer", "")],
        json!({"client": "say customer"}),
    );
    write_old_approvals(&clone.worktree, &[old_entry("LoadAnswer", "")], json!({}));
    write_open_review(&clone);
    let old_main = fs::read(clone.main.join(".code-comments.json")).unwrap();
    let old_worktree = fs::read(clone.worktree.join(".code-comments.json")).unwrap();
    let old_review = read_files_under(&clone.state);
    common::use_test_waiting_list();

    run_move(&clone);

    assert_eq!(
        fs::read(clone.main.join(".code-comments.json")).unwrap(),
        old_main
    );
    assert_eq!(
        fs::read(clone.worktree.join(".code-comments.json")).unwrap(),
        old_worktree
    );
    assert_eq!(read_files_under(&clone.state), old_review);
}

/// `move_old_approvals` must give each moved Feedback Request the answer or User Note the user gave in the old tool.
/// Otherwise the user would have to answer an open review again after the switch-over.
#[test]
fn move_old_approvals_keeps_open_review_user_feedback() {
    let clone = make_clone();
    write_open_review(&clone);
    common::use_test_waiting_list();

    run_move(&clone);

    let review = review_session::load(&clone.worktree).unwrap().unwrap();
    assert_eq!(review.lists[0].requests.len(), 3);
    let saved = review.lists[0]
        .requests
        .iter()
        .find(|request| request.name.as_deref() == Some("SaveAnswer"))
        .unwrap();
    let edited = match review.feedback.get(&saved.id) {
        Some(UserFeedback::Answer(UserAnswer::Edited { text })) => text.clone(),
        other => panic!("SaveAnswer has {other:?}"),
    };
    assert_eq!(edited, "Saves each answer under the lock.\n");
    let notes: Vec<&str> = review
        .feedback
        .values()
        .filter_map(|feedback| match feedback {
            UserFeedback::Note(note) => Some(note.0.as_str()),
            UserFeedback::Answer(_) => None,
        })
        .collect();
    assert_eq!(notes, ["Say where it loads from."]);
    assert_eq!(review.feedback.len(), 2);
}

/// `move_old_approvals` must leave a moved review that waits for the user on the waiting list.
/// Otherwise Ctrl+b e would not show the review, and the agent would wait forever.
#[test]
fn move_old_approvals_puts_waiting_review_on_list() {
    let clone = make_clone();
    write_open_review(&clone);
    common::use_test_waiting_list();

    run_move(&clone);

    let review = review_session::load(&clone.worktree).unwrap().unwrap();
    assert_eq!(review.status, ReviewStatus::WaitingForUserFeedback);
    let waiting = find_waiting_worktrees(clone.main.parent().unwrap());
    assert_eq!(waiting, [fs::canonicalize(&clone.worktree).unwrap()]);
}

/// `move_old_approvals` must not move an old review whose newest round was already written.
/// Otherwise answers the old tool wrote would wait in Ctrl+b e again and be written a second time.
#[test]
fn move_old_approvals_skips_applied_old_review() {
    let clone = make_clone();
    write_open_review(&clone);
    write_old_review_file(
        &clone,
        "review-comments",
        "applied.json",
        json!({"round": 1}),
    );
    common::use_test_waiting_list();

    run_move(&clone);

    assert!(review_session::load(&clone.worktree).unwrap().is_none());
}

/// `move_old_approvals` must leave out, and report with its User Feedback, a request whose comment changed since it was sent.
/// Otherwise an old answer could be written over text the user never saw.
#[test]
fn move_old_approvals_leaves_out_changed_comments() {
    let clone = make_clone();
    let items = json!([
        {"id": 1, "file": STORE, "old": "// Saved the answer.\n", "proposal": "keep", "name": "SaveAnswer"},
        {"id": 2, "file": STORE, "old": "// Loads the answer.\n", "proposal": "keep", "name": "LoadAnswer"},
    ]);
    let round = json!({"version": 1, "repo": clone.worktree, "items": items});
    write_old_review_file(&clone, "review-comments", "round-1.json", round);
    let answers = json!({"1": {"action": "keep", "round": 1}});
    write_old_review_file(&clone, "review-comments", "decisions.json", answers);
    common::use_test_waiting_list();

    let (_, reply) = run_move(&clone);

    let review = review_session::load(&clone.worktree).unwrap().unwrap();
    let names: Vec<_> = review.lists[0]
        .requests
        .iter()
        .map(|request| request.name.as_deref())
        .collect();
    assert_eq!(names, [Some("LoadAnswer")]);
    let left_out = &reply["result"]["reviews"][0]["outcome"]["moved"]["left_out"];
    assert_eq!(
        left_out,
        &json!([{"id": 1, "file": STORE, "name": "SaveAnswer", "user_feedback": {"answer": "keep"}}])
    );
}

/// `move_old_approvals` must keep a Review Session the worktree already has, and report that it did.
/// Otherwise the move would replace User Feedback the user gave in the new program.
#[test]
fn move_old_approvals_keeps_existing_review_session() {
    let clone = make_clone();
    let review_file = clone.main.join(".git/worktrees/worktree/pde/review.json");
    let review = json!({
        "status": "user_notes_waiting",
        "origin": {"repo": "main", "branch": "review-comments", "agent_session": ""},
        "lists": [],
        "feedback": {},
    });
    fs::create_dir_all(review_file.parent().unwrap()).unwrap();
    fs::write(&review_file, review.to_string()).unwrap();
    let before = fs::read(&review_file).unwrap();
    write_open_review(&clone);
    common::use_test_waiting_list();

    let (_, reply) = run_move(&clone);

    assert_eq!(fs::read(&review_file).unwrap(), before);
    assert_eq!(
        reply["result"]["reviews"][0]["outcome"],
        "already_has_review_session"
    );
}
