//! This module talks to Herdr, the terminal program that holds the Review
//! Popup and the Review Tab, through the socket Herdr gives every program it
//! starts. Read the `open_from_herdr(herdr, kind, waiting_count)` and
//! `save_popup_record(path, workspace_id, name)` functions to understand the
//! module.

use std::env;
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process;
use std::time::Duration;

use serde_json::{Value, json};

use super::fit::{cut_middle, display_width, escape_control_characters};
use crate::commands::cli::ScreenKind;
use crate::shared::review_session::ReviewOrigin;

/// Names the Herdr plugin whose panes run the screen. It must match the plugin
/// manifest that chezmoi installs.
const PLUGIN_ID: &str = "pde.comment-review";
/// Labels the Review Tab, so a second Ctrl+b E can find it.
const REVIEW_TAB_LABEL: &str = "comment review";
/// Keeps a stuck Herdr from leaving Ctrl+b e waiting.
const HERDR_TIME_LIMIT: Duration = Duration::from_secs(5);
/// Says how many columns the popup record's name may take, since a refused PR
/// approval shows it on the phone.
const RECORD_NAME_COLUMNS: usize = 19;

/// Sends requests to the Herdr server that started this program.
pub(super) struct Herdr {
    socket: PathBuf,
}

/// Says why Herdr did not do what it was asked.
#[derive(Debug)]
pub(super) enum HerdrError {
    /// Holds Herdr's own refusal, such as `ui_busy` while another popup is open.
    Refused { code: String },
    /// Holds why the request did not reach Herdr or its answer could not be read.
    Failed(String),
}

impl fmt::Display for HerdrError {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Refused { code } => write!(formatter, "Herdr refused with {code}."),
            Self::Failed(reason) => write!(formatter, "Herdr could not be reached. {reason}"),
        }
    }
}

impl Herdr {
    /// Returns the Herdr that started this program, or `None` outside Herdr.
    pub(super) fn from_env() -> Option<Self> {
        let socket = env::var_os("HERDR_SOCKET_PATH")?;
        Some(Self::new(PathBuf::from(socket)))
    }

    pub(super) fn new(socket: PathBuf) -> Self {
        Self { socket }
    }

    /// Sends one request and returns Herdr's result. Herdr answers a refusal
    /// with an error object, so an answer alone is not success.
    fn send(&self, method: &str, params: Value) -> Result<Value, HerdrError> {
        let failed = |error: io::Error| HerdrError::Failed(error.to_string());
        let mut stream = UnixStream::connect(&self.socket).map_err(failed)?;
        stream
            .set_read_timeout(Some(HERDR_TIME_LIMIT))
            .map_err(failed)?;
        stream
            .set_write_timeout(Some(HERDR_TIME_LIMIT))
            .map_err(failed)?;
        let request = json!({"id": "comment-review", "method": method, "params": params});
        writeln!(stream, "{request}").map_err(failed)?;
        let mut answer = String::new();
        BufReader::new(stream)
            .read_line(&mut answer)
            .map_err(failed)?;
        let mut answer: Value = serde_json::from_str(&answer)
            .map_err(|error| HerdrError::Failed(format!("Herdr's answer is not JSON. {error}")))?;
        if let Some(code) = answer.pointer("/error/code").and_then(Value::as_str) {
            return Err(HerdrError::Refused {
                code: code.to_owned(),
            });
        }
        Ok(answer["result"].take())
    }

    pub(super) fn show_notification(&self, title: &str) -> Result<(), HerdrError> {
        self.send("notification.show", json!({"title": title}))
            .map(|_| ())
    }

    /// Closes the pane `pane_id`, such as the Review Tab when the user quits.
    /// Herdr ends the program in it, so call this last.
    pub(super) fn close_pane(&self, pane_id: &str) -> Result<(), HerdrError> {
        self.send("pane.close", json!({"pane_id": pane_id}))
            .map(|_| ())
    }

    /// Opens the plugin's `popup` or `tab` pane, which runs this program's
    /// `screen` command again, and returns the new pane's tab.
    fn open_screen_pane(&self, kind: ScreenKind) -> Result<String, HerdrError> {
        let (entrypoint, placement) = choose_screen_pane(kind);
        let opened = self.send(
            "plugin.pane.open",
            json!({
                "plugin_id": PLUGIN_ID,
                "entrypoint": entrypoint,
                "placement": placement,
                "focus": true,
                "env": {},
            }),
        )?;
        let tab_id = opened.pointer("/plugin_pane/pane/tab_id");
        Ok(tab_id
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned())
    }

    /// Opens the Review Tab, or moves to the one already open, so a second
    /// Ctrl+b E does not open a second Review Screen.
    fn open_review_tab(&self) -> Result<(), HerdrError> {
        let listed = self.send("tab.list", json!({}))?;
        let open_tab = listed["tabs"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|tab| tab["label"] == REVIEW_TAB_LABEL);
        if let Some(tab) = open_tab {
            return self.focus_tab(&tab["tab_id"]);
        }
        let tab_id = Value::from(self.open_screen_pane(ScreenKind::Tab)?);
        self.send(
            "tab.rename",
            json!({"tab_id": tab_id, "label": REVIEW_TAB_LABEL}),
        )?;
        self.focus_tab(&tab_id)
    }

    /// Moves every screen attached to Herdr to the tab.
    fn focus_tab(&self, tab_id: &Value) -> Result<(), HerdrError> {
        self.send("tab.focus", json!({"tab_id": tab_id}))
            .map(|_| ())
    }
}

/// Opens the Review Popup or the Review Tab for Ctrl+b e or Ctrl+b E, or shows
/// a Herdr Notification that says why not and what to do. Herdr keeps the
/// action's own errors out of sight, so every failure also notifies.
pub(super) fn open_from_herdr(
    herdr: &Herdr,
    kind: ScreenKind,
    waiting_count: usize,
) -> Result<(), HerdrError> {
    let key = match kind {
        ScreenKind::Popup => "Ctrl+b e",
        ScreenKind::Tab => "Ctrl+b E",
    };
    if waiting_count == 0 {
        return herdr.show_notification(&format!(
            "No comment reviews are waiting. Press {key} once an agent sends one."
        ));
    }
    let opened = match kind {
        ScreenKind::Popup => herdr.open_screen_pane(kind).map(|_| ()),
        ScreenKind::Tab => herdr.open_review_tab(),
    };
    let Err(error) = opened else {
        return Ok(());
    };
    let title = match &error {
        HerdrError::Refused { code } if code == "ui_busy" => {
            format!("Another popup is open. Close it, then press {key}.")
        }
        _ => "The comment review did not open. Run herdr plugin log list to see why.".to_owned(),
    };
    // The notification is the user's only sign, so its own failure is dropped.
    let _ = herdr.show_notification(&title);
    Err(error)
}

/// Returns where the popup record lives, `$XDG_STATE_HOME/pde/pr-write/open-popup`.
pub(super) fn find_popup_record_path() -> Option<PathBuf> {
    // Tests never open a real popup, so one that reaches here must not touch the user's record.
    if cfg!(feature = "test-util") {
        panic!("tests must pass their own popup record path");
    }
    let state = crate::shared::review_session::find_pde_state_folder().ok()?;
    Some(state.join("pr-write/open-popup"))
}

/// Returns the Herdr workspace the popup opened in. Herdr gives a popup no
/// `HERDR_WORKSPACE_ID`, so it comes from `HERDR_PLUGIN_CONTEXT_JSON`.
pub(super) fn read_workspace_id() -> Option<String> {
    let context: Value = serde_json::from_str(&env::var("HERDR_PLUGIN_CONTEXT_JSON").ok()?).ok()?;
    let workspace = context["workspace_id"].as_str()?;
    (!workspace.is_empty()).then(|| workspace.to_owned())
}

/// Saves the popup record, in the format the PR approval tool reads to tell
/// the user which popup to close. Only the owner can read it.
pub(super) fn save_popup_record(path: &Path, workspace_id: &str, name: &str) -> io::Result<()> {
    if let Some(folder) = path.parent() {
        fs::create_dir_all(folder)?;
    }
    let record = json!({"pid": process::id(), "workspace_id": workspace_id, "name": name});
    OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)?
        .write_all(record.to_string().as_bytes())
}

/// Returns the popup record's name for a review, its label after a "Review"
/// prefix, or only its branch when the label is too long.
pub(super) fn build_record_name(origin: &ReviewOrigin) -> String {
    const PREFIX: &str = "Review: ";
    let room = RECORD_NAME_COLUMNS - PREFIX.len();
    let branch = origin.branch.rsplit('/').next().unwrap_or_default();
    let mut name = escape_control_characters(&format!("{} · {branch}", origin.repo));
    if display_width(&name) > room {
        name = escape_control_characters(branch);
    }
    format!("{PREFIX}{}", cut_middle(&name, room))
}

/// Returns the plugin pane that shows `kind`, as its entrypoint and its
/// placement. The Herdr plugin file must have a pane with that entrypoint.
fn choose_screen_pane(kind: ScreenKind) -> (&'static str, &'static str) {
    match kind {
        ScreenKind::Popup => ("popup", "popup"),
        ScreenKind::Tab => ("tab", "tab"),
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::net::UnixListener;
    use std::process::Command;
    use std::thread::{self, JoinHandle};
    use std::time::Instant;

    use super::*;
    use crate::commands::cli::{self, parse_command};

    /// A temporary folder for a made-up Herdr socket, deleted when dropped.
    struct SocketFolder(PathBuf);

    impl Drop for SocketFolder {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    /// Makes an empty temporary folder.
    fn make_folder() -> SocketFolder {
        let output = Command::new("mktemp").arg("-d").output().unwrap();
        SocketFolder(PathBuf::from(
            String::from_utf8(output.stdout).unwrap().trim_end(),
        ))
    }

    /// Starts a made-up Herdr server that gives each request the next of
    /// `answers`, and returns the requests it got once the answers run out or
    /// two seconds pass.
    fn start_fake_herdr(answers: Vec<Value>) -> (Herdr, SocketFolder, JoinHandle<Vec<Value>>) {
        let folder = make_folder();
        let socket = folder.0.join("herdr.sock");
        let listener = UnixListener::bind(&socket).unwrap();
        listener.set_nonblocking(true).unwrap();
        let server = thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(2);
            let mut requests = Vec::new();
            let mut answers = answers.into_iter();
            while Instant::now() < deadline {
                let Ok((stream, _)) = listener.accept() else {
                    thread::sleep(Duration::from_millis(10));
                    continue;
                };
                stream.set_nonblocking(false).unwrap();
                let mut line = String::new();
                BufReader::new(&stream).read_line(&mut line).unwrap();
                requests.push(serde_json::from_str::<Value>(&line).unwrap());
                let Some(answer) = answers.next() else { break };
                writeln!(&stream, "{answer}").unwrap();
                if answers.len() == 0 {
                    break;
                }
            }
            requests
        });
        (Herdr::new(socket), folder, server)
    }

    /// Returns each request's method.
    fn list_methods(requests: &[Value]) -> Vec<&str> {
        requests
            .iter()
            .map(|request| request["method"].as_str().unwrap())
            .collect()
    }

    /// `open_from_herdr` must show a Herdr Notification, and open nothing,
    /// when no review waits. Otherwise Ctrl+b e opens an empty popup, or does
    /// nothing and the user cannot tell why.
    #[test]
    fn open_from_herdr_notifies_when_none_waits() {
        let (herdr, _folder, server) = start_fake_herdr(vec![json!({"result": {}})]);

        open_from_herdr(&herdr, ScreenKind::Popup, 0).unwrap();

        let requests = server.join().unwrap();
        assert_eq!(list_methods(&requests), ["notification.show"]);
        assert_eq!(
            requests[0]["params"]["title"],
            "No comment reviews are waiting. Press Ctrl+b e once an agent sends one."
        );
    }

    /// `open_from_herdr` must tell the user to close the other popup when
    /// Herdr refuses with `ui_busy`. Otherwise Ctrl+b e does nothing and the
    /// user cannot tell why.
    #[test]
    fn open_from_herdr_asks_to_close_popup() {
        let busy = json!({"error": {"code": "ui_busy", "message": "A popup is open."}});
        let (herdr, _folder, server) = start_fake_herdr(vec![busy, json!({"result": {}})]);

        let opened = open_from_herdr(&herdr, ScreenKind::Popup, 1);

        let requests = server.join().unwrap();
        assert!(opened.is_err());
        assert_eq!(
            requests[1]["params"]["title"],
            "Another popup is open. Close it, then press Ctrl+b e."
        );
    }

    /// `open_from_herdr` must move to the Review Tab already open on a second
    /// Ctrl+b E. Otherwise a second Review Screen opens on the same review.
    #[test]
    fn open_from_herdr_moves_to_review_tab() {
        let listed = json!({"result": {"tabs": [
            {"tab_id": "t1", "label": "shell"},
            {"tab_id": "t7", "label": "comment review"},
        ]}});
        let (herdr, _folder, server) = start_fake_herdr(vec![listed, json!({"result": {}})]);

        open_from_herdr(&herdr, ScreenKind::Tab, 1).unwrap();

        let requests = server.join().unwrap();
        assert_eq!(list_methods(&requests), ["tab.list", "tab.focus"]);
        assert_eq!(requests[1]["params"]["tab_id"], "t7");
    }

    /// `choose_screen_pane` and `parse_command` must match the Herdr plugin file's plugin id,
    /// panes and keys. Otherwise Ctrl+b e or Ctrl+b E opens no review.
    #[test]
    fn plugin_manifest_matches_screen_panes_and_keys() {
        let manifest_path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../chezmoi/dot_local/share/pde/herdr-plugins/pde-comment-review/herdr-plugin.toml.tmpl"
        );
        let manifest = fs::read_to_string(manifest_path).unwrap();
        let entries: Vec<&str> = manifest.split("\n\n").collect();

        // The plugin id is the one this program asks Herdr for.
        assert_eq!(read_field(entries[0], "id"), format!("\"{PLUGIN_ID}\""));
        // Each pane this program opens runs the screen it was opened for.
        for kind in [ScreenKind::Popup, ScreenKind::Tab] {
            let (entrypoint, placement) = choose_screen_pane(kind);
            let pane = find_entry(&entries, "[[panes]]", entrypoint);
            assert_eq!(read_field(pane, "placement"), format!("\"{placement}\""));
            assert_eq!(read_screen_kind(pane), kind, "pane {entrypoint}");
        }
        // Herdr's keys run the `open` action for Ctrl+b e and `open-tab` for Ctrl+b E.
        for (action, kind) in [("open", ScreenKind::Popup), ("open-tab", ScreenKind::Tab)] {
            let entry = find_entry(&entries, "[[actions]]", action);
            assert_eq!(read_screen_kind(entry), kind, "action {action}");
        }
    }

    /// Returns the `[[panes]]` or `[[actions]]` entry of the plugin file whose id is `id`.
    fn find_entry<'a>(entries: &[&'a str], table: &str, id: &str) -> &'a str {
        let id_line = format!("\nid = \"{id}\"\n");
        entries
            .iter()
            .find(|entry| entry.starts_with(table) && entry.contains(&id_line))
            .unwrap_or_else(|| panic!("no {table} entry with id {id}"))
    }

    /// Returns the value of the `key = value` line in a plugin file entry, quotes included.
    fn read_field<'a>(entry: &'a str, key: &str) -> &'a str {
        let prefix = format!("{key} = ");
        entry
            .lines()
            .find_map(|line| line.strip_prefix(prefix.as_str()))
            .unwrap_or_else(|| panic!("no {key} in {entry}"))
    }

    /// Returns the screen a plugin file entry's command opens, as `parse_command` reads it.
    fn read_screen_kind(entry: &str) -> ScreenKind {
        let command = read_field(entry, "command");
        let args: Vec<OsString> = command
            .trim_start_matches('[')
            .trim_end_matches(']')
            .split(", ")
            .map(|arg| OsString::from(arg.trim_matches('"')))
            .collect();
        let program = args[0].to_string_lossy();
        assert!(
            program.ends_with("/.local/bin/pde-comment-review"),
            "{command}"
        );
        match parse_command(&args) {
            Ok(cli::Command::Screen { kind }) => kind,
            other => panic!("{command} is not a screen command, {other:?}"),
        }
    }

    /// `save_popup_record` must save the `pid`, `workspace_id` and `name` the
    /// PR approval tool reads, in a file only the owner can read. Otherwise the
    /// tool cannot say which popup to close, or other users can read it.
    #[test]
    fn save_popup_record_writes_private_popup_details() {
        let folder = make_folder();
        let path = folder.0.join("pr-write/open-popup");

        save_popup_record(&path, "w1G", "Review: pde · one").unwrap();

        let saved: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(
            saved,
            json!({"pid": process::id(), "workspace_id": "w1G", "name": "Review: pde · one"})
        );
        let mode = fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }
}
