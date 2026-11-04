//! This module loads and saves each worktree's Review Session under a lock,
//! so two screens lose no User Feedback, and it keeps the waiting list that
//! Ctrl+b e reads. Read the `add_list(dir, list)`,
//! `save_user_feedback(dir, shown_request, feedback)`, `lock(dir)`,
//! `load(dir)`, `add_to_waiting_list(dir)`, and `read_waiting_list()`
//! functions to understand the module.

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fmt::Display;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use super::comments;
use super::git::{self, CommitId};
use super::user_feedback::{UserAnswer, UserFeedback};
use super::{Error, ErrorCode};

const REVIEW_FILE: &str = "review.json";
const LOCK_FILE: &str = "review.json.lock";
const TEMP_FILE: &str = "review.json.tmp";
const UNREADABLE_FILE: &str = "review.json.unreadable";
const WAITING_LIST_FILE: &str = "waiting-list.json";
const WAITING_LIST_LOCK_FILE: &str = "waiting-list.json.lock";
const WAITING_LIST_TEMP_FILE: &str = "waiting-list.json.tmp";

/// Holds the folder that tests give the waiting list in place of the user's own.
static TEST_WAITING_LIST_FOLDER: OnceLock<PathBuf> = OnceLock::new();

/// Saves a sent Feedback Request List and sets `waiting_for_user_feedback`,
/// under the lock, then returns the list's number from 1. A new Review Session
/// starts unless one waits for User Notes, so a finished one's User Feedback
/// never answers the new request 1.
///
/// # Errors
///
/// Refuses with `WaitingForUserFeedback` while a list waits for the user, and
/// with `WriteNotFinished` while a write is unfinished. Refuses with
/// `ListChanged` when the Review Session is no longer as `seen`, such as after
/// another call sent a list.
pub(crate) fn add_list(
    dir: &Path,
    list: FeedbackRequestList,
    seen: SeenReviewSession,
) -> Result<usize, Error> {
    let root = git::find_root(dir)?;
    let mut lock = lock(&root)?;
    let status = lock
        .review_session
        .as_ref()
        .map(|review_session| review_session.status);
    if let Some(status) = status {
        refuse_busy_status(status)?;
    }
    // A list built before another call changed the Review Session could drop User Feedback given since.
    if SeenReviewSession::of(lock.review_session.as_ref()) != seen {
        return Err(Error {
            code: ErrorCode::ListChanged,
            message: "The Review Session changed after this list was built, such as when another call sent a list. Build the list again.".to_owned(),
        });
    }
    let list_number = if status == Some(ReviewStatus::UserNotesWaiting) {
        let review_session = lock
            .review_session
            .as_mut()
            .expect("the status came from the Review Session");
        add_list_after_user_notes(&root, review_session, list)?;
        review_session.lists.len()
    } else {
        // Past the refusal above, there is no Review Session or only a done one.
        lock.review_session = Some(ReviewSession {
            status: ReviewStatus::WaitingForUserFeedback,
            origin: find_review_origin(&root)?,
            lists: vec![list],
            feedback: BTreeMap::new(),
            write_error: None,
        });
        1
    };
    lock.save()?;
    Ok(list_number)
}

/// Returns the status and list count of the Review Session, for step 5 to
/// pass to `add_list` after it builds the list again.
pub(crate) fn see_review_session(dir: &Path) -> Result<SeenReviewSession, Error> {
    Ok(SeenReviewSession::of(load(dir)?.as_ref()))
}

/// Refuses to build or send a Feedback Request List while one waits for the
/// user or a write did not finish. Steps 3 and 5 share it.
///
/// # Errors
///
/// Refuses with `WaitingForUserFeedback` or `WriteNotFinished`.
pub(crate) fn refuse_busy_status(status: ReviewStatus) -> Result<(), Error> {
    match status {
        ReviewStatus::WaitingForUserFeedback => Err(Error {
            code: ErrorCode::WaitingForUserFeedback,
            message:
                "A Feedback Request List waits for the user. Tell them to press Ctrl+b e, then wait."
                    .to_owned(),
        }),
        ReviewStatus::WritingUserFeedback => Err(Error {
            code: ErrorCode::WriteNotFinished,
            message: "A write into the code did not finish. Ask the user to check git status."
                .to_owned(),
        }),
        ReviewStatus::UserNotesWaiting | ReviewStatus::Done => Ok(()),
    }
}

/// Saves User Feedback unless the Review Session stopped waiting for it or the
/// Feedback Request changed since the screen showed it. Once every Feedback
/// Request has User Feedback and any of it is a User Note, sets
/// `user_notes_waiting` and then takes the worktree off the waiting list, so a
/// waiting-list error comes after the save and the User Feedback is kept.
pub fn save_user_feedback(
    dir: &Path,
    shown_request: &FeedbackRequest,
    feedback: UserFeedback,
) -> Result<ReviewStatus, Error> {
    let mut lock = lock_shown_request(dir, shown_request)?;
    let review_session = lock
        .review_session_mut()
        .expect("lock_shown_request returns only a lock that holds a Review Session");
    review_session.feedback.insert(shown_request.id, feedback);
    let every_request_has_feedback = review_session
        .lists
        .iter()
        .flat_map(|list| &list.requests)
        .all(|request| review_session.feedback.contains_key(&request.id));
    let any_note = review_session
        .feedback
        .values()
        .any(|feedback| matches!(feedback, UserFeedback::Note(_)));
    if every_request_has_feedback && any_note {
        review_session.status = ReviewStatus::UserNotesWaiting;
        // The next list the user sees is the agent's new one, whose write never failed.
        review_session.write_error = None;
    }
    let status = review_session.status;
    lock.save()?;
    if status == ReviewStatus::UserNotesWaiting {
        // Saving first means a failed change to the waiting list loses no User Feedback.
        // The Review Session's lock is still held, so the worktree stays listed for the
        // agent's next Feedback Request List.
        remove_from_waiting_list(dir)?;
    }
    Ok(status)
}

/// Removes the User Note the screen showed on a Feedback Request, under the
/// same checks as `save_user_feedback`, so an empty note leaves the request
/// with no User Feedback.
pub(crate) fn remove_user_note(
    dir: &Path,
    shown_request: &FeedbackRequest,
) -> Result<ReviewStatus, Error> {
    let mut lock = lock_shown_request(dir, shown_request)?;
    let review_session = lock
        .review_session_mut()
        .expect("lock_shown_request returns only a lock that holds a Review Session");
    if matches!(
        review_session.feedback.get(&shown_request.id),
        Some(UserFeedback::Note(_))
    ) {
        review_session.feedback.remove(&shown_request.id);
    }
    let status = review_session.status;
    lock.save()?;
    Ok(status)
}

/// Takes the lock once the Review Session waits for User Feedback and still
/// holds `shown_request` as the newest version of its Feedback Request.
fn lock_shown_request(
    dir: &Path,
    shown_request: &FeedbackRequest,
) -> Result<ReviewSessionLock, Error> {
    let mut lock = lock(dir)?;
    let Some(review_session) = lock.review_session_mut() else {
        return Err(late_error("No Review Session takes User Feedback here."));
    };
    if review_session.status != ReviewStatus::WaitingForUserFeedback {
        return Err(late_error(
            "This Review Session no longer takes User Feedback.",
        ));
    }
    // A screen left open may still show an older version of this Feedback Request.
    let newest_request = review_session
        .lists
        .iter()
        .rev()
        .flat_map(|list| &list.requests)
        .find(|request| request.id == shown_request.id);
    if newest_request != Some(shown_request) {
        return Err(Error {
            code: ErrorCode::RequestChanged,
            message: "The Feedback Request changed after the screen showed it.".to_owned(),
        });
    }
    Ok(lock)
}

/// Takes the lock and reads `review.json` again under it. Waits while any
/// other holder has the lock, even one in this process.
pub fn lock(dir: &Path) -> Result<ReviewSessionLock, Error> {
    let folder = git::find_worktree_git_folder(dir)?.join("pde");
    fs::create_dir_all(&folder).map_err(|error| save_error(&folder, error))?;
    // Each save renames over review.json, so the lock sits on a file that stays.
    let lock_path = folder.join(LOCK_FILE);
    let lock_file = File::options()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&lock_path)
        .map_err(|error| save_error(&lock_path, error))?;
    lock_file
        .lock()
        .map_err(|error| save_error(&lock_path, error))?;
    let review_session = read_review_session(&folder)?;
    Ok(ReviewSessionLock {
        _lock_file: lock_file,
        folder,
        review_session,
    })
}

/// Returns the saved Review Session, or `None` when there is none. Moves an
/// unreadable file aside, but leaves a newer version's file in place.
pub fn load(dir: &Path) -> Result<Option<ReviewSession>, Error> {
    Ok(lock(dir)?.review_session)
}

/// Returns the saved Review Session's status without taking the lock or
/// changing any file. Returns `None` when there is no Review Session, or when
/// its file cannot be read, since step 3 moves such a file aside.
pub(crate) fn read_status(dir: &Path) -> Result<Option<ReviewStatus>, Error> {
    /// Holds only the status, so a newer version's other fields still read.
    #[derive(Deserialize)]
    struct SavedStatus {
        status: ReviewStatus,
    }

    let path = git::find_worktree_git_folder(dir)?
        .join("pde")
        .join(REVIEW_FILE);
    // Each save renames a whole new file into place, so reading needs no lock.
    let Ok(saved) = fs::read(path) else {
        return Ok(None);
    };
    Ok(serde_json::from_slice::<SavedStatus>(&saved)
        .ok()
        .map(|saved| saved.status))
}

impl ReviewSessionLock {
    /// Returns the folder that holds `review.json`, in the worktree's own Git
    /// folder.
    pub(crate) fn folder(&self) -> &Path {
        &self.folder
    }

    /// Returns the Review Session to change, or `None` when there is none.
    pub fn review_session_mut(&mut self) -> Option<&mut ReviewSession> {
        self.review_session.as_mut()
    }

    /// Saves through a temporary file, so a crash leaves the old file whole.
    /// Saves nothing when there is no Review Session.
    pub fn save(&self) -> Result<(), Error> {
        let Some(review_session) = &self.review_session else {
            return Ok(());
        };
        let path = self.folder.join(REVIEW_FILE);
        let mut text =
            serde_json::to_vec_pretty(review_session).map_err(|error| save_error(&path, error))?;
        text.push(b'\n');
        // Only the lock holder writes this file, so one fixed name is enough.
        let temp_path = self.folder.join(TEMP_FILE);
        write_and_sync(&temp_path, &text).map_err(|error| save_error(&temp_path, error))?;
        fs::rename(&temp_path, &path).map_err(|error| save_error(&path, error))
    }
}

/// Adds the worktree that holds `dir` to the end of the waiting list, for
/// Ctrl+b e, unless it is listed already.
pub fn add_to_waiting_list(dir: &Path) -> Result<(), Error> {
    change_waiting_list(dir, |worktrees, worktree| {
        if !worktrees.contains(&worktree) {
            worktrees.push(worktree);
        }
    })
}

/// Takes the worktree that holds `dir` off the waiting list.
pub(crate) fn remove_from_waiting_list(dir: &Path) -> Result<(), Error> {
    change_waiting_list(dir, |worktrees, worktree| {
        worktrees.retain(|listed| *listed != worktree);
    })
}

/// Lists the worktrees on the waiting list in the order they were added,
/// skipping any that no longer exist.
pub fn read_waiting_list() -> Result<Vec<PathBuf>, Error> {
    let path = find_waiting_list_folder()?.join(WAITING_LIST_FILE);
    // Each change renames a whole new list into place, so reading needs no lock.
    let saved = read_saved_waiting_list(&path)?;
    Ok(saved
        .worktrees
        .into_iter()
        .filter(|worktree| worktree.exists())
        .collect())
}

/// Holds one worktree's Review Session, saved in its own Git folder.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewSession {
    pub status: ReviewStatus,
    pub origin: ReviewOrigin,
    pub lists: Vec<FeedbackRequestList>,
    pub feedback: BTreeMap<RequestId, UserFeedback>,
    /// Says why the last write failed, while the list waits for the user again.
    /// A missing field reads as no failed write.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub write_error: Option<WriteError>,
}

impl ReviewSession {
    /// Returns the newest version of each Feedback Request, in ID order.
    pub(crate) fn newest_requests(&self) -> Vec<&FeedbackRequest> {
        let mut newest = BTreeMap::new();
        // Lists are kept in the order they were sent, so a later version replaces an older one.
        for request in self.lists.iter().flat_map(|list| &list.requests) {
            newest.insert(request.id, request);
        }
        newest.into_values().collect()
    }

    /// Returns every Answered Request, or `None` while any Feedback Request in
    /// its newest version is not answered.
    pub(crate) fn answered_requests(&self) -> Option<Vec<AnsweredRequest<'_>>> {
        self.newest_requests()
            .into_iter()
            .map(|request| match self.feedback.get(&request.id) {
                Some(UserFeedback::Answer(answer)) => Some(AnsweredRequest { request, answer }),
                _ => None,
            })
            .collect()
    }
}

/// Holds an Answered Request, a Feedback Request in its newest version with the
/// User Feedback that answers it.
pub(crate) struct AnsweredRequest<'review> {
    pub(crate) request: &'review FeedbackRequest,
    pub(crate) answer: &'review UserAnswer,
}

/// Says where the Review Session is.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewStatus {
    WaitingForUserFeedback,
    UserNotesWaiting,
    /// Marks a write into the code that is running, crashed or could not be undone.
    WritingUserFeedback,
    Done,
}

/// Names the repository, branch and agent session the Review Session came from.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewOrigin {
    pub repo: String,
    pub branch: String,
    pub agent_session: String,
}

/// Holds one Feedback Request List as it was sent.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FeedbackRequestList {
    /// Names the commit the list's Feedback Requests came from.
    pub base: CommitId,
    pub requests: Vec<FeedbackRequest>,
}

/// Asks the user about one comment.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FeedbackRequest {
    pub id: RequestId,
    pub file: PathBuf,
    /// Holds `None` for an unnamed comment, which saves no approval.
    pub name: Option<String>,
    pub reason: RequestReason,
    pub text: String,
    pub can_reword: bool,
    /// Finds an unnamed comment again when its Feedback Request is sent back.
    pub line: usize,
    pub suggestion: Option<AgentSuggestion>,
}

/// Numbers a Feedback Request the same way in every Feedback Request List.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct RequestId(pub(crate) u32);

/// Says why the comment needs User Feedback.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequestReason {
    New,
    /// Uses a name whose comment the user once cut.
    WasCut,
    /// Differs from the text the user approved for this name.
    DiffersFromApproved,
    /// Asks to reword a comment the user approved.
    AgentWantsChange,
}

/// Gives the agent's advice on one comment and why.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentSuggestion {
    pub kind: SuggestionKind,
    pub reasoning: String,
    pub also_at: Vec<String>,
}

/// Names what the agent suggests doing with the comment.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum SuggestionKind {
    Keep,
    Cut,
    Reword { text: String },
}

/// Holds why a write failed, as its error code and the message it refused
/// with.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WriteError {
    pub code: ErrorCode,
    pub detail: String,
}

/// Holds the lock until dropped, with the Review Session read under it.
pub struct ReviewSessionLock {
    _lock_file: File,
    folder: PathBuf,
    review_session: Option<ReviewSession>,
}

fn read_review_session(folder: &Path) -> Result<Option<ReviewSession>, Error> {
    let path = folder.join(REVIEW_FILE);
    let saved = match fs::read(&path) {
        Ok(saved) => saved,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        // Only a file with broken JSON is moved aside, so the user must fix a file
        // that cannot be read at all.
        Err(error) => {
            return Err(read_error(
                &path,
                format!("Tell the user to fix the file. {error}"),
            ));
        }
    };
    let error = match serde_json::from_slice(&saved) {
        Ok(review_session) => return Ok(Some(review_session)),
        Err(error) => error,
    };
    // Moving a newer version's file aside would lose its Review Session.
    if is_from_newer_version(&error) {
        return Err(Error {
            code: ErrorCode::UnknownSavedField,
            message: format!(
                "{} holds a field or value this version does not know. Run pde-installer to update it. {error}",
                path.display()
            ),
        });
    }
    let unreadable_path = find_free_unreadable_path(folder);
    fs::rename(&path, &unreadable_path).map_err(|error| save_error(&path, error))?;
    Err(read_error(
        &path,
        format!(
            "Moved it to {}. Build the Feedback Request List again. {error}",
            unreadable_path.display()
        ),
    ))
}

/// Treats an unknown field or value as a newer version's file. A later version
/// may only add those, or this version moves its file aside as damaged.
fn is_from_newer_version(error: &serde_json::Error) -> bool {
    // serde_json gives these errors no kind of their own, so this reads the message.
    let message = error.to_string();
    error.is_data()
        && (message.starts_with("unknown field") || message.starts_with("unknown variant"))
}

/// Returns the first unused backup name, so an earlier backup is never replaced.
fn find_free_unreadable_path(folder: &Path) -> PathBuf {
    let mut path = folder.join(UNREADABLE_FILE);
    let mut number = 2;
    while path.exists() {
        path = folder.join(format!("{UNREADABLE_FILE}.{number}"));
        number += 1;
    }
    path
}

fn write_and_sync(path: &Path, contents: &[u8]) -> io::Result<()> {
    let mut file = File::create(path)?;
    file.write_all(contents)?;
    file.sync_all()
}

fn read_error(path: &Path, error: impl Display) -> Error {
    Error {
        code: ErrorCode::SavedFileUnreadable,
        message: format!(
            "Cannot read the Review Session in {}. {error}",
            path.display()
        ),
    }
}

fn save_error(path: &Path, error: impl Display) -> Error {
    Error {
        code: ErrorCode::SaveFailed,
        message: format!(
            "Cannot save the Review Session in {}. {error}",
            path.display()
        ),
    }
}

fn late_error(message: &str) -> Error {
    Error {
        code: ErrorCode::LateUserFeedback,
        message: message.to_owned(),
    }
}

/// Adds the agent's list to a Review Session that waited for User Notes. Each
/// request the list sends again loses its User Note, each earlier request
/// whose User Feedback no longer fits its comment leaves every list, and each
/// kept one takes its comment's line now.
fn add_list_after_user_notes(
    root: &Path,
    review_session: &mut ReviewSession,
    list: FeedbackRequestList,
) -> Result<(), Error> {
    let left_out = match_left_out_requests(root, review_session, &list)?;
    for earlier in &mut review_session.lists {
        earlier
            .requests
            .retain(|request| left_out.get(&request.id) != Some(&LeftOut::Dropped));
        for request in &mut earlier.requests {
            // Step 7 finds a comment with no name only at its saved line, so a kept answer moves with its comment.
            if let Some(LeftOut::Kept { line }) = left_out.get(&request.id) {
                request.line = *line;
            }
        }
    }
    for (id, outcome) in &left_out {
        if *outcome == LeftOut::Dropped {
            review_session.feedback.remove(id);
        }
    }
    for request in &list.requests {
        review_session.feedback.remove(&request.id);
    }
    review_session.lists.push(list);
    review_session.status = ReviewStatus::WaitingForUserFeedback;
    Ok(())
}

/// Returns what happens to each earlier request that `list` leaves out. Step 3
/// leaves out a noted request only when its comment is gone, and an answered
/// request keeps its answer while its comment holds the text the user answered.
fn match_left_out_requests(
    root: &Path,
    review_session: &ReviewSession,
    list: &FeedbackRequestList,
) -> Result<BTreeMap<RequestId, LeftOut>, Error> {
    let sent: BTreeSet<RequestId> = list.requests.iter().map(|request| request.id).collect();
    let newest = review_session.newest_requests();
    let files: BTreeSet<&Path> = newest
        .iter()
        .filter(|request| !sent.contains(&request.id))
        .map(|request| request.file.as_path())
        .collect();
    let mut left_out = BTreeMap::new();
    for file in files {
        // Matching every request in the file, as step 3 does, gives each one the comment step 3 gave it.
        let in_file: Vec<&FeedbackRequest> = newest
            .iter()
            .copied()
            .filter(|request| request.file == file)
            .collect();
        let now = comments::read_comments_on_disk(root, file)?;
        for (request, comment) in in_file.iter().zip(now.match_requests(&in_file)) {
            if sent.contains(&request.id) {
                continue;
            }
            let is_answered = matches!(
                review_session.feedback.get(&request.id),
                Some(UserFeedback::Answer(_))
            );
            let outcome = match comment {
                Some(comment) if is_answered && comment.text == request.text => {
                    LeftOut::Kept { line: comment.line }
                }
                _ => LeftOut::Dropped,
            };
            left_out.insert(request.id, outcome);
        }
    }
    Ok(left_out)
}

/// Holds the status and list count step 5 saw before it built the list again,
/// which every list `add_list` saves changes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct SeenReviewSession {
    status: Option<ReviewStatus>,
    list_count: usize,
}

impl SeenReviewSession {
    /// Returns what the Review Session, or `None` for none, shows now.
    fn of(review_session: Option<&ReviewSession>) -> Self {
        Self {
            status: review_session.map(|review_session| review_session.status),
            list_count: review_session.map_or(0, |review_session| review_session.lists.len()),
        }
    }
}

/// Says what `add_list` does with an earlier request the new list leaves out.
#[derive(Debug, PartialEq)]
enum LeftOut {
    /// Keeps its answer, with the line its comment starts at now.
    Kept { line: usize },
    /// Leaves every list with its User Feedback, which no longer fits its comment.
    Dropped,
}

/// Returns the Review Session's origin from Git and the agent's environment.
/// The repository takes its main checkout's folder name, and the agent session
/// is `OPENCODE_SESSION_ID`, empty when unset.
fn find_review_origin(root: &Path) -> Result<ReviewOrigin, Error> {
    let shared = git::find_shared_git_folder(root)?;
    // A worktree's folder is often named after its branch, so the main checkout names the repository.
    let main_checkout = shared
        .parent()
        .filter(|_| shared.ends_with(".git"))
        .unwrap_or(root);
    let repo = main_checkout
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    Ok(ReviewOrigin {
        repo,
        branch: git::read_current_branch(root)?,
        agent_session: env::var("OPENCODE_SESSION_ID").unwrap_or_default(),
    })
}

/// Holds the worktrees on the waiting list, saved in `waiting-list.json`.
#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SavedWaitingList {
    worktrees: Vec<PathBuf>,
}

/// Takes the waiting list's lock, reads the list again under it, changes it
/// for the worktree that holds `dir`, and saves it through a synced temporary
/// file. Keeps every other entry, even one it cannot reach now, such as a
/// worktree on a drive that is not mounted.
fn change_waiting_list(
    dir: &Path,
    change: impl FnOnce(&mut Vec<PathBuf>, PathBuf),
) -> Result<(), Error> {
    // A worktree's subfolders share its Review Session, so the list holds the top folder.
    let worktree = git::find_root(dir)?;
    let folder = find_waiting_list_folder()?;
    // `_lock` holds the lock until this function returns, after the save.
    let _lock = lock_waiting_list(&folder)?;
    let mut saved = read_saved_waiting_list(&folder.join(WAITING_LIST_FILE))?;
    change(&mut saved.worktrees, worktree);
    write_saved_waiting_list(&folder, &saved)
}

/// Returns `$XDG_STATE_HOME/pde/comment-review`, or
/// `~/.local/state/pde/comment-review` without it.
fn find_waiting_list_folder() -> Result<PathBuf, Error> {
    if cfg!(feature = "test-util") {
        // Failing here stops a test that forgot its folder from changing the user's list.
        let folder = TEST_WAITING_LIST_FOLDER.get().expect(
            "a test must call set_test_waiting_list_folder before it reaches the waiting list",
        );
        return Ok(folder.clone());
    }
    Ok(find_pde_state_folder()?.join("comment-review"))
}

/// Returns `$XDG_STATE_HOME/pde`, or `~/.local/state/pde` without it. The
/// waiting list and the popup record live under it.
pub(crate) fn find_pde_state_folder() -> Result<PathBuf, Error> {
    // An empty or relative folder counts as unset, as the XDG rules say.
    let absolute_folder = |name| {
        env::var_os(name)
            .map(PathBuf::from)
            .filter(|folder| folder.is_absolute())
    };
    let state = absolute_folder("XDG_STATE_HOME")
        .or_else(|| absolute_folder("HOME").map(|home| home.join(".local/state")))
        .ok_or_else(|| Error {
            code: ErrorCode::SaveFailed,
            message: "Cannot find the state folder, because neither XDG_STATE_HOME nor HOME is an absolute path.".to_owned(),
        })?;
    Ok(state.join("pde"))
}

/// Locks a file beside the waiting list, since each save replaces the list
/// and a lock on the old one guards nothing.
fn lock_waiting_list(folder: &Path) -> Result<File, Error> {
    fs::create_dir_all(folder).map_err(|error| waiting_list_save_error(folder, error))?;
    let lock_path = folder.join(WAITING_LIST_LOCK_FILE);
    let lock_file =
        File::create(&lock_path).map_err(|error| waiting_list_save_error(&lock_path, error))?;
    lock_file
        .lock()
        .map_err(|error| waiting_list_save_error(&lock_path, error))?;
    Ok(lock_file)
}

/// Reads the waiting list, or an empty one when it is missing.
fn read_saved_waiting_list(path: &Path) -> Result<SavedWaitingList, Error> {
    let saved = match fs::read(path) {
        Ok(saved) => saved,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(SavedWaitingList::default());
        }
        Err(error) => return Err(waiting_list_read_error(path, error)),
    };
    serde_json::from_slice(&saved).map_err(|error| {
        if !is_from_newer_version(&error) {
            return waiting_list_read_error(path, error);
        }
        Error {
            code: ErrorCode::UnknownSavedField,
            message: format!(
                "{} holds a field or value this version does not know. Run pde-installer to update it. {error}",
                path.display()
            ),
        }
    })
}

/// Replaces the waiting list with a synced temporary file, so a crash leaves
/// a whole list, old or new.
fn write_saved_waiting_list(folder: &Path, saved: &SavedWaitingList) -> Result<(), Error> {
    let path = folder.join(WAITING_LIST_FILE);
    let mut text =
        serde_json::to_vec_pretty(saved).map_err(|error| waiting_list_save_error(&path, error))?;
    text.push(b'\n');
    let temp_path = folder.join(WAITING_LIST_TEMP_FILE);
    write_and_sync(&temp_path, &text)
        .map_err(|error| waiting_list_save_error(&temp_path, error))?;
    fs::rename(&temp_path, &path).map_err(|error| waiting_list_save_error(&path, error))
}

fn waiting_list_read_error(path: &Path, error: impl Display) -> Error {
    Error {
        code: ErrorCode::SavedFileUnreadable,
        message: format!(
            "Cannot read the waiting list in {}. Tell the user to fix the file. {error}",
            path.display()
        ),
    }
}

fn waiting_list_save_error(path: &Path, error: impl Display) -> Error {
    Error {
        code: ErrorCode::SaveFailed,
        message: format!(
            "Cannot save the waiting list in {}. {error}",
            path.display()
        ),
    }
}

/// Makes every waiting-list call in this process use `folder`, so tests never
/// change the user's own list. The first call wins.
#[cfg(feature = "test-util")]
#[doc(hidden)]
pub fn set_test_waiting_list_folder(folder: &Path) {
    TEST_WAITING_LIST_FOLDER.get_or_init(|| folder.to_path_buf());
}

/// Names the variable that gives the test build's binary its waiting-list
/// folder, since a child process cannot see the test's own setting.
#[cfg(feature = "test-util")]
#[doc(hidden)]
pub const TEST_WAITING_LIST_VARIABLE: &str = "PDE_COMMENT_REVIEW_TEST_WAITING_LIST";

/// Makes this process use the waiting-list folder that
/// `TEST_WAITING_LIST_VARIABLE` names, when it is set.
#[cfg(feature = "test-util")]
#[doc(hidden)]
pub fn use_test_waiting_list_from_environment() {
    if let Some(folder) = env::var_os(TEST_WAITING_LIST_VARIABLE) {
        set_test_waiting_list_folder(Path::new(&folder));
    }
}
