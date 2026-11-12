//! Reads the command line into a `Command`, and finds the run folder.
//! Read `parse_command` first.

use std::error::Error as _;
use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use clap::error::ErrorKind;
use clap::{ArgGroup, Parser};

use crate::board::ScreenSize;
use crate::external::cards::CardKey;
use crate::shared::run_folder::RunFolder;
use crate::shared::{AgentName, HubError};

// clap turns each `///` line on `Cli`, `Command`, `RunArg` and their
// fields into the help text users see, so those lines are written for
// the user. Notes for maintainers use `//`.
//
// A command's first `///` line is its line in `hub --help`. The
// paragraph after it shows in `hub COMMAND --help` and says what the
// command does and when to use it. `max_term_width` makes clap wrap
// help at 40 columns to fit a phone screen. clap splits a long example
// at any space, so wrap examples by hand with a shell `\`.

/// Watch, message and tidy a hub's sessions.
#[derive(Parser)]
#[command(
    name = "hub",
    // `build.rs` sets this from `RELEASE_VERSION`, the newest `hub-v` tag
    // or Cargo.toml.
    version = env!("HUB_BUILD_VERSION"),
    override_usage = "hub [--machine LABEL] COMMAND ...",
    max_term_width = 40,
    // clap cannot mark the commands that take a run folder, so this
    // list is written by hand. Each line repeats a command's first `///`
    // line, and `hub_help_list_matches_each_command` fails when they
    // differ. clap starts `{after-help}` with a blank line of its own.
    help_template = "\
usage: {usage}

{about-with-newline}
Commands marked * use --run DIR, else
$HUB_RUN, else the run folder of the
hub running on this machine, else the
one used last.

  status*  Each session and its state
  next*    Wait for news, print one line
  send*    Deliver, or hold until free
  read*    Print and archive a report
  report*  Write your report (workers)
  inbox*   Print held messages (workers)
  note*    Add a timed line to notes.md
  brief*   What waits on the user
  handoff* Start the next hub
  adopt*   Take over an older open hub
  clean*   Close and park finished tabs
  park*    Close one tab, keep its ID
  auto     The steps of an auto run
  flag     Mark a pane \"needs you\"
  unflag   Clear the \"needs you\" mark
  alive    Is an OpenCode session alive
  go       Show the next hub
  gather   Put hubs at the sidebar top
  desk     Make the user's desk
  board*   Cards by stage, for a phone{after-help}",
    after_help = "\
hub COMMAND --help says what one does,
when to use it, and gives an example.

Exit: 0 done, 1 a tool failed,
2 wrong usage, 3 not done."
)]
pub struct Cli {
    /// Send Herdr calls to the SSH machine saved in Herdr as LABEL
    #[arg(long, value_name = "LABEL")]
    pub machine: Option<String>,
    #[command(subcommand)]
    pub command: Command,
}

// The run folder, shared by every command that takes one. `None`
// falls back to `HUB_RUN`, then to the running hub's folder, then
// to the one used last, in `find_run_folder`.
#[derive(clap::Args)]
pub struct RunArg {
    /// The hub's run folder
    #[arg(long, value_name = "DIR")]
    pub run: Option<PathBuf>,
}

// The folder of an auto run, for each `hub auto` step. The user's
// usual hub runs these steps while its own `hub next` holds its own
// folder's lock, so a search would find the wrong folder. `None` falls
// back to `HUB_RUN` only, and in that hub's pane `HUB_RUN` names the
// same wrong folder, so the steps need `--run`.
#[derive(clap::Args)]
pub struct AutoRunArg {
    /// The auto run's folder
    #[arg(long, value_name = "DIR")]
    pub run: Option<PathBuf>,
}

#[derive(clap::Subcommand)]
pub enum Command {
    /// Each session and its state
    ///
    /// Prints the hub's own line, the board line, a count per state,
    /// the unread reports, then one row per session. STATE says where
    /// it stands, such as hub when it waits on the hub, or you when it
    /// waits on the user. WHY says why. Run it at the start of each hub
    /// turn. It works with --machine.
    #[command(after_help = "Example: hub status --json")]
    Status {
        #[command(flatten)]
        run: RunArg,
        /// Print the same facts as JSON
        #[arg(long)]
        json: bool,
    },
    /// Wait for news, print one line
    ///
    /// Waits until something needs the hub, such as a new report, a
    /// blocked session or a finished one. Then it prints one line and
    /// exits. A hub keeps one running in the background and starts it
    /// again after each line. A newer one makes an older one exit.
    #[command(after_help = "Example: hub next --timeout 600")]
    Next {
        #[command(flatten)]
        run: RunArg,
        /// Give up after this many seconds
        #[arg(long, value_name = "SECONDS")]
        timeout: Option<f64>,
        /// Print the news as JSON
        #[arg(long)]
        json: bool,
        /// Start old hubs' session reviews, which are off by default
        #[arg(long)]
        start_reviews: bool,
    },
    /// Deliver, or hold until free
    ///
    /// Sends a message to a session at once when it can take it.
    /// Otherwise the hub holds it, and hub next sends it later, oldest
    /// first. A parked session first reopens in a new tab. Use it for
    /// every message to a session, and --file for long text. It works
    /// with --machine.
    #[command(after_help = "Example: hub send docs-fix \\\n  --file /tmp/msg.md")]
    Send {
        #[command(flatten)]
        run: RunArg,
        /// The session's agent name
        name: AgentName,
        /// The message text
        #[arg(required_unless_present = "file", conflicts_with = "file")]
        message: Option<String>,
        /// Read the message from this file
        #[arg(long, value_name = "PATH")]
        file: Option<PathBuf>,
    },
    /// Print and archive a report
    ///
    /// Prints a worker's report and archives it, so it no longer counts
    /// as unread. Then it lists the other unread reports. Run it when
    /// hub next prints a report line. --note also adds a line to
    /// notes.md, and --card adds that line to the card's Log.
    #[command(after_help = "Example: hub read docs-fix \\\n  --note \"merged #9\"")]
    Read {
        #[command(flatten)]
        run: RunArg,
        /// The session's agent name
        name: AgentName,
        /// Add this line to notes.md
        #[arg(long)]
        note: Option<String>,
        /// Also add the note to the card, from the name or this key
        // `None` without `--card`, `Some(None)` for `--card` alone.
        #[arg(long, value_name = "KEY", requires = "note")]
        card: Option<Option<CardKey>>,
    },
    /// Write your report (workers)
    ///
    /// A worker runs it to give its report to the hub. It writes
    /// report-NAME.md in the run folder in one step, so the hub never
    /// reads half of it. FILE or standard input gives the text, and hub
    /// next tells the hub. Empty text writes nothing.
    #[command(after_help = "Example: hub report docs-fix \\\n  /tmp/report.md")]
    Report {
        #[command(flatten)]
        run: RunArg,
        /// Your agent name
        name: AgentName,
        /// The report file, or - for standard input
        // `None` reads standard input too.
        file: Option<PathBuf>,
    },
    /// Print held messages (workers)
    ///
    /// Prints every message the hub holds for you and counts each as
    /// delivered, so hub next never sends it. A worker runs it before
    /// it writes its handoff.
    #[command(after_help = "Example: hub inbox docs-fix")]
    Inbox {
        #[command(flatten)]
        run: RunArg,
        /// Your agent name
        name: AgentName,
    },
    /// Add a timed line to notes.md
    ///
    /// Adds TEXT to the run's notes.md as a line that starts with the
    /// local time. The first line of each day gets a date heading. A
    /// hub runs it to log what happened, such as a merge.
    #[command(after_help = "Example: hub note \"merged #9\"")]
    Note {
        #[command(flatten)]
        run: RunArg,
        /// The line to add
        text: String,
    },
    /// What waits on the user
    ///
    /// Prints the board line, reviews.md and each entry of decisions.md
    /// in full, so the user can answer without asking what a question
    /// was. Run it when the user asks what waits on them. A problem in
    /// decisions.md, such as an entry with a part missing, stops it
    /// with an error.
    #[command(after_help = "Example: hub brief")]
    Brief {
        #[command(flatten)]
        run: RunArg,
    },
    /// Start the next hub
    ///
    /// Starts the next hub in a new tab, on the model rules.md names,
    /// and gives it the handoff file. Then it checks the reasoning
    /// effort of its first reply. Run it when hub next or hub status
    /// says hand off now.
    #[command(after_help = "Example: hub handoff kanban-hub-14 \\\n  /tmp/hub-14.md")]
    Handoff {
        #[command(flatten)]
        run: RunArg,
        /// The next hub's agent name
        name: AgentName,
        /// The handoff file for the next hub
        handoff: PathBuf,
    },
    /// Take over an older open hub
    ///
    /// Run it when hub next prints no hub handoff for OLD. OLD is an
    /// older hub of this run that is still open, and no hub handoff
    /// started this hub from it. It writes the handoff for OLD's
    /// session review, so hub clean closes OLD once it is idle.
    #[command(after_help = "Example: hub adopt kanban-hub-3")]
    Adopt {
        #[command(flatten)]
        run: RunArg,
        /// The older hub's agent name
        old: AgentName,
    },
    /// Close and park finished tabs
    ///
    /// Closes the tab of each to-close session and parks each to-park
    /// one, the states hub status shows. Kept sessions stay. Run it
    /// after hub next names to-close or to-park sessions. --dry-run
    /// shows what it would do.
    #[command(after_help = "Example: hub clean --dry-run")]
    Clean {
        #[command(flatten)]
        run: RunArg,
        /// Print what it would do, and change nothing
        #[arg(long)]
        dry_run: bool,
    },
    /// Close one tab, keep its ID
    ///
    /// Parks one to-park session now, rather than waiting for hub
    /// clean. It saves the session's ID, adds it to its card and closes
    /// its tab. A later hub send reopens it. A session in another state
    /// stays, and the reason prints.
    #[command(after_help = "Example: hub park docs-fix")]
    Park {
        #[command(flatten)]
        run: RunArg,
        /// The session's agent name
        name: AgentName,
    },
    /// The steps of an auto run
    ///
    /// An auto run works a list of cards the user approved, without the
    /// user, until each stops at a pull request or a question. Run its
    /// steps in the order ready, check, approve, now. Each step needs
    /// --run DIR, the auto run's folder. In a hub's pane, $HUB_RUN is
    /// that hub's own folder.
    #[command(after_help = "Example: hub auto check --run /tmp/run2")]
    Auto {
        #[command(subcommand)]
        step: AutoStep,
    },
    /// Mark a pane "needs you"
    ///
    /// Shows "needs you" in the sidebar in place of the session's
    /// state. Run it when a session waits on the user. A blocked
    /// session's mark clears once it works again. It works with
    /// --machine.
    #[command(after_help = "Example: hub flag docs-fix")]
    Flag {
        /// The session's agent name
        name: AgentName,
    },
    /// Clear the "needs you" mark
    ///
    /// Removes the mark that hub flag set. Run it when you send the
    /// session the user's answer. It works with --machine.
    #[command(after_help = "Example: hub unflag docs-fix")]
    Unflag {
        /// The session's agent name
        name: AgentName,
    },
    /// Is an OpenCode session alive
    ///
    /// Reads this machine's OpenCode database and prints the message
    /// count, the newest time, the last tool, and working, finished or
    /// stopped. It sees subagents that Herdr does not list. Run it
    /// before you call a session dead.
    #[command(after_help = "Example: hub alive ses_0a1b2c3d4e5f")]
    Alive {
        /// The OpenCode session ID
        session: String,
    },
    /// Show the next hub
    ///
    /// Shows the hub after the one you look at, in sidebar order,
    /// wrapping to the first. A hub is an agent whose name ends in
    /// -hub-N. In Herdr, the prefix key then shift+h runs it.
    #[command(after_help = "Example: hub go")]
    Go,
    /// Put hubs at the sidebar top
    ///
    /// Moves every hub's workspace on this machine to the top of the
    /// sidebar. Hubs named kanban-hub-N come first, then the desk, then
    /// the other hubs. It also cuts the agent list to those hubs and
    /// the sessions that wait on the user. A hub runs it when it
    /// starts.
    #[command(after_help = "Example: hub gather")]
    Gather,
    /// Make the user's desk
    ///
    /// Makes the desk workspace when it is missing, with hub board in
    /// its first tab and a shell in its second. Then it moves the desk
    /// below the hubs named kanban-hub-N, as hub gather does. Run it to
    /// set up the workspace you watch from a phone.
    #[command(after_help = "Example: hub desk")]
    Desk,
    // `verbatim_doc_comment` keeps the step list's columns, so every
    // line below is wrapped by hand at 40 columns.
    /// Cards by stage, for a phone
    ///
    /// Shows your cards in groups that fit a
    /// 40x16 phone screen. The top line counts
    /// the cards in progress and in review, and
    /// what waits on you. Below it are five
    /// groups, and Could not read on top when
    /// a read fails. In progress, In review,
    /// Ready and Draft hold cards, and Needs
    /// you holds the questions in decisions.md
    /// and the sessions that wait on you. Ready
    /// and Draft start folded. It reads again
    /// every 10 seconds, or when you press r.
    ///
    /// Tap a count to jump to its group, a
    /// group to fold or open it, + N more to
    /// see the rest, or a row to see the card
    /// in full. j and k move the choice, and
    /// Enter or space taps it. q quits from the
    /// list. On a card, q, Esc or any tap goes
    /// back. --print prints one screen as text,
    /// and --dump prints its elements as JSON.
    /// Both draw a new board from the run
    /// folder, not the board that listens.
    ///
    /// To let an agent drive the board you
    /// watch, start it with --listen. Then
    /// hub board --send STEP, from another
    /// terminal on the same run folder, does
    /// one step on your screen and prints the
    /// screen that follows as JSON. --send
    /// exits 2 when the board refuses the step
    /// and 3 when no board listens. They talk
    /// through board.sock in the run folder,
    /// which works on Linux and macOS but not
    /// Windows. Until its first read ends, the
    /// board shows no cards and its hint says
    /// reading. Tests use the hidden --control,
    /// which reads the same steps one per line.
    ///
    /// Each step is one of these.
    ///   tap ID       tap an element by the id
    ///                --send prints for it
    ///   tap COL ROW  tap a place; 0 0 is the
    ///                top left
    ///   press KEY    press a key
    ///   wheel up     scroll up
    ///   wheel down   scroll down
    /// KEY is enter, esc, space, up, down,
    /// pageup, pagedown, ctrl-c, or one
    /// character, such as q, r, j or k.
    ///
    /// The JSON has step, the step sent, and
    /// page, which is list, details or quit.
    /// lines holds the screen as text, one
    /// string per row. elements lists all the
    /// page holds, top first. Each has id,
    /// kind, text and hidden, and tap when a
    /// tap does something. kind is count,
    /// heading, item, more, back or text. tap
    /// is jump, fold, unfold, show all, open
    /// or back. A drawn element also has
    /// column, row and width, and chosen is
    /// true on the row Enter acts on.
    ///
    /// Rows scrolled off, cards in a folded
    /// group and cards behind + N more have
    /// hidden true, hidden_by (scroll, fold or
    /// more) and show, the step to send next,
    /// such as tap group:ready or press
    /// pagedown. Send show, then find the
    /// element in the reply, until hidden is
    /// false. That can take many sends, and
    /// show may change on the way. tap ID on a
    /// hidden element fails and names show.
    #[command(verbatim_doc_comment)]
    #[command(after_help = "\
Example: hub board --listen
Steps, keys and JSON are in --help.")]
    #[command(after_long_help = "\
Example: hub board --listen
Then: hub board --send 'press j'")]
    #[command(group(ArgGroup::new("output").args(["print", "dump", "control"])))]
    Board {
        #[command(flatten)]
        run: RunArg,
        /// Print one screen as text and exit
        #[arg(long)]
        print: bool,
        /// Print one screen's elements as JSON and exit
        #[arg(long)]
        dump: bool,
        /// Read steps, one per line, and print JSON after each
        #[arg(long, hide = true)]
        control: bool,
        /// Also take steps from --send
        #[arg(long, conflicts_with = "output")]
        listen: bool,
        /// Do one step on the board that listens, and print its screen as JSON
        #[arg(
            long,
            value_name = "STEP",
            conflicts_with_all = ["output", "listen"]
        )]
        send: Option<String>,
        /// The screen size --print, --dump and --control draw
        #[arg(
            long,
            value_name = "COLSxROWS",
            default_value = "40x16",
            requires = "output"
        )]
        size: ScreenSize,
    },
}

// The steps run in the order the `auto-run` skill gives.
#[derive(clap::Subcommand)]
pub enum AutoStep {
    /// Write the ready cards to ready.md
    ///
    /// Writes each ready card's title, project and criteria to ready.md
    /// in the run folder, and prints it. Run it first, then write
    /// run.md from it.
    #[command(after_help = "Example: hub auto ready --run /tmp/run2")]
    Ready {
        #[command(flatten)]
        run: AutoRunArg,
    },
    /// Compare run.md against the ready cards
    ///
    /// Checks that run.md names every ready card once, and prints how
    /// many cards the run works, or one line per problem. Run it after
    /// each change to run.md.
    #[command(after_help = "Example: hub auto check --run /tmp/run2")]
    Check {
        #[command(flatten)]
        run: AutoRunArg,
    },
    /// Save run.md with the user's reply
    ///
    /// Saves the checked run.md with the user's reply, once. The reply
    /// has to be approve run N, word for word, unless --given says the
    /// user's own words name exactly the run's cards. Run it when the
    /// user answers the run's question.
    #[command(after_help = "Example: hub auto approve 2 \\\n  \"approve run 2\" --run /tmp/run2")]
    Approve {
        #[command(flatten)]
        run: AutoRunArg,
        /// The reply is the user's own words naming exactly the run's
        /// cards, not `approve run N`
        #[arg(long)]
        given: bool,
        /// The run's number
        number: u32,
        /// The user's reply, word for word
        reply: String,
    },
    /// Write and print brief-now.md
    ///
    /// Writes brief-now.md, one line per card of the run from its
    /// worker's status file, and prints it. A card waiting on a
    /// question says (needs you). Run it when the user asks for a
    /// status update during the run.
    #[command(after_help = "Example: hub auto now --run /tmp/run2")]
    Now {
        #[command(flatten)]
        run: AutoRunArg,
    },
}

impl Command {
    /// Returns the name the user typed, such as `go`.
    pub fn name(&self) -> &'static str {
        match self {
            Command::Status { .. } => "status",
            Command::Next { .. } => "next",
            Command::Send { .. } => "send",
            Command::Read { .. } => "read",
            Command::Report { .. } => "report",
            Command::Inbox { .. } => "inbox",
            Command::Note { .. } => "note",
            Command::Brief { .. } => "brief",
            Command::Handoff { .. } => "handoff",
            Command::Adopt { .. } => "adopt",
            Command::Clean { .. } => "clean",
            Command::Park { .. } => "park",
            Command::Auto { .. } => "auto",
            Command::Flag { .. } => "flag",
            Command::Unflag { .. } => "unflag",
            Command::Alive { .. } => "alive",
            Command::Go => "go",
            Command::Gather => "gather",
            Command::Desk => "desk",
            Command::Board { .. } => "board",
        }
    }
}

/// Parses the arguments after `hub`. clap prints its own message and
/// exits for help, the version and bad flags.
///
/// # Errors
///
/// `HubError::Usage` for a bad agent name or card key, with the
/// message its parser gave, such as `Docs/Fix is not an agent name`.
pub fn parse_command() -> Result<Cli, HubError> {
    Cli::try_parse().map_err(|error| match error.kind() {
        // clap keeps the `from_str` error as the source.
        ErrorKind::ValueValidation => HubError::Usage(
            error
                .source()
                .map_or_else(|| error.to_string(), ToString::to_string),
        ),
        _ => error.exit(),
    })
}

/// Returns the run folder from `--run`, else from `HUB_RUN`, else the
/// hub's folder in `local_state`, which is `~/.local/state`. `hub auto`
/// steps pass `None` as `local_state`, since the user's usual hub runs
/// them and a search would find that hub's own folder. That hub's
/// `HUB_RUN` names the same folder, so those steps need `--run`.
///
/// # Errors
///
/// `HubError::Usage` when the folder does not exist. Without `--run`
/// and `HUB_RUN`, also when `local_state` is `None`, when two or more
/// hubs run and skipping auto runs does not leave exactly one, or when
/// none runs and no folder has an `events.jsonl`.
/// `HubError::File` when `local_state`, a lock file, an `approved.md`
/// or an `events.jsonl` cannot be read.
pub fn find_run_folder(
    given: Option<&Path>,
    hub_run: Option<&OsStr>,
    local_state: Option<&Path>,
) -> Result<RunFolder, HubError> {
    let named = given
        .map(Path::to_path_buf)
        .or_else(|| hub_run.filter(|value| !value.is_empty()).map(PathBuf::from));
    match (named, local_state) {
        (Some(path), _) => RunFolder::open(&path),
        (None, Some(local_state)) => find_hub_folder(local_state),
        (None, None) => Err(HubError::Usage(
            "no run folder; pass --run DIR or set HUB_RUN".to_owned(),
        )),
    }
}

/// Returns the one `herdr-hub.*` folder in `local_state` whose watcher
/// lock a `hub next` holds now. The herdr-hub skill makes each run
/// folder there with `mktemp`, and old ones stay after their hub ends,
/// so only a held lock tells the running hub apart.
///
/// A hub has no `hub next` running while it handles a line. With no
/// lock held, the search takes the folder whose `events.jsonl` changed
/// last, since `hub next` writes there, and names it on stderr.
///
/// During an auto run the user's hub and the auto hub both run. The
/// search then skips folders with an `approved.md`, which only auto
/// runs have, and names the one left on stderr. When skipping them
/// leaves no folder or more than one, the search stops with an error
/// that names each running folder. The skip needs two locks held, so
/// while the user's hub handles a line the search takes the auto run.
fn find_hub_folder(local_state: &Path) -> Result<RunFolder, HubError> {
    let file_error = |source| HubError::File {
        path: local_state.to_path_buf(),
        source,
    };
    let paths: Vec<PathBuf> = match fs::read_dir(local_state) {
        Ok(entries) => entries
            .map(|entry| entry.map(|entry| entry.path()))
            .collect::<Result<_, _>>()
            .map_err(file_error)?,
        // No state folder means no hub ever ran here.
        Err(source) if source.kind() == io::ErrorKind::NotFound => Vec::new(),
        Err(source) => return Err(file_error(source)),
    };

    let mut running = Vec::new();
    let mut newest: Option<(SystemTime, RunFolder)> = None;
    for path in paths {
        let is_hub_name = path
            .file_name()
            .is_some_and(|name| name.to_string_lossy().starts_with("herdr-hub."));
        if !is_hub_name {
            continue;
        }
        // A file with that name is not a run folder.
        let Ok(run) = RunFolder::open(&path) else {
            continue;
        };
        if run.is_watcher_running()? {
            running.push(run);
        } else if let Some(modified) = read_events_time(&run)?
            && newest.as_ref().is_none_or(|(time, _)| modified > *time)
        {
            newest = Some((modified, run));
        }
    }

    match running.len() {
        0 => {
            let Some((_, run)) = newest else {
                return Err(HubError::Usage(
                    "no run folder; no hub here wrote events yet, so pass --run DIR".to_owned(),
                ));
            };
            eprintln!(
                "hub: no hub next is running, so using {}",
                run.path().display()
            );
            Ok(run)
        }
        1 => Ok(running.remove(0)),
        count => {
            let mut own_hubs = Vec::new();
            let mut auto_runs = Vec::new();
            for run in running {
                if run.read_file_if_exists("approved.md")?.is_some() {
                    auto_runs.push(run);
                } else {
                    own_hubs.push(run);
                }
            }
            if own_hubs.len() == 1 {
                let run = own_hubs.remove(0);
                eprintln!("hub: skipping auto runs, so using {}", run.path().display());
                return Ok(run);
            }
            let mut folders: Vec<String> = own_hubs
                .iter()
                .chain(&auto_runs)
                .map(|run| run.path().display().to_string())
                .collect();
            folders.sort();
            Err(HubError::Usage(format!(
                "no run folder; {count} hubs run here, so pass --run with one of {}",
                folders.join(", ")
            )))
        }
    }
}

/// Returns when the run's `events.jsonl` last changed, or `None` when
/// the run has none.
fn read_events_time(run: &RunFolder) -> Result<Option<SystemTime>, HubError> {
    let path = run.path().join("events.jsonl");
    match fs::metadata(&path).and_then(|metadata| metadata.modified()) {
        Ok(modified) => Ok(Some(modified)),
        Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(HubError::File { path, source }),
    }
}

/// Tells whether a command works with `--machine`, which only
/// `status`, `send`, `flag` and `unflag` do. The rest read this
/// machine's files and database, or its own Herdr sidebar.
pub fn takes_machine(command: &Command) -> bool {
    match command {
        Command::Status { .. }
        | Command::Send { .. }
        | Command::Flag { .. }
        | Command::Unflag { .. } => true,
        Command::Next { .. }
        | Command::Read { .. }
        | Command::Report { .. }
        | Command::Inbox { .. }
        | Command::Note { .. }
        | Command::Brief { .. }
        | Command::Handoff { .. }
        | Command::Adopt { .. }
        | Command::Clean { .. }
        | Command::Park { .. }
        | Command::Auto { .. }
        | Command::Alive { .. }
        | Command::Go
        | Command::Gather
        | Command::Desk
        | Command::Board { .. } => false,
    }
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory;

    use super::*;

    /// Each command and `hub auto` step needs a paragraph for its
    /// `--help` and an example of itself. Without them the user guesses
    /// from a one-line summary. An example copied from another command
    /// would show the wrong usage.
    #[test]
    fn command_help_has_long_text_and_example() {
        let hub = Cli::command();
        let auto = hub.find_subcommand("auto").unwrap();
        let commands = hub
            .get_subcommands()
            .map(|command| (format!("hub {}", command.get_name()), command))
            .chain(
                auto.get_subcommands()
                    .map(|step| (format!("hub auto {}", step.get_name()), step)),
            );

        for (path, command) in commands {
            let example = command
                .get_after_help()
                .map(ToString::to_string)
                .unwrap_or_default();
            assert!(
                command.get_long_about().is_some(),
                "{path} has no long help"
            );
            assert!(
                example.starts_with(&format!("Example: {path}")),
                "{path} has the example {example:?}"
            );
        }
    }

    /// Help is read on a 40-column phone, where a longer line wraps
    /// mid-word. clap wraps the text to 40 columns but splits a long
    /// example at any space, so examples must wrap by hand with a
    /// shell `\`. clap never wraps the usage line, so it is left out.
    #[test]
    fn command_help_fits_phone_width() {
        let mut hub = Cli::command();
        hub.build();
        let mut pages = vec![("hub".to_owned(), hub.render_long_help().to_string())];
        for command in hub.get_subcommands_mut() {
            let path = format!("hub {}", command.get_name());
            pages.push((path.clone(), help_and_example(command)));
            for step in command.get_subcommands_mut() {
                let step_path = format!("{path} {}", step.get_name());
                pages.push((step_path, help_and_example(step)));
            }
        }

        for (path, page) in pages {
            for line in page.lines().filter(|line| !line.starts_with("Usage:")) {
                assert!(line.chars().count() <= 40, "{path}: {line:?}");
            }
        }
    }

    /// One command's long help, then its example as written in the source.
    fn help_and_example(command: &mut clap::Command) -> String {
        let example = command
            .get_after_help()
            .map(ToString::to_string)
            .unwrap_or_default();
        format!("{}\n{example}", command.render_long_help())
    }

    /// `hub --help` prints a command list written by hand, which once
    /// kept `board` as "check the mouse in a pane" long after it became
    /// the phone board. Each line must match its command's first help
    /// line, and carry `*` exactly when the command takes `--run`.
    #[test]
    fn hub_help_list_matches_each_command() {
        let expected: Vec<(String, String)> = Cli::command()
            .get_subcommands()
            .map(|command| {
                let takes_run = command.get_arguments().any(|arg| arg.get_id() == "run");
                let mark = if takes_run { "*" } else { "" };
                let about = command.get_about().map(ToString::to_string);
                (
                    format!("{}{mark}", command.get_name()),
                    about.unwrap_or_default(),
                )
            })
            .collect();

        let help = Cli::command().render_help().to_string();
        let listed: Vec<(String, String)> = help
            .lines()
            .filter_map(|line| line.strip_prefix("  ")?.split_once(' '))
            .map(|(name, about)| (name.to_owned(), about.trim_start().to_owned()))
            .collect();

        assert_eq!(listed, expected);
    }

    /// A remote hub lists, flags and messages sessions on another
    /// machine with `--machine`, so refusing it there leaves those
    /// sessions unseen, unmarked or without their message. Allowing it
    /// on `go` would make `go` jump, in this machine's sidebar, to a hub
    /// found on another machine, and on `alive` would read this
    /// machine's database for another machine's session. `clean` and
    /// `park` check this machine's worktrees with git. With `--machine`,
    /// they would read this machine's git facts but close tabs on
    /// another machine.
    #[test]
    fn takes_machine_refuses_local_only_commands() {
        let name: AgentName = "docs-fix".parse().unwrap();
        let cases = [
            (
                "status",
                Command::Status {
                    run: RunArg { run: None },
                    json: false,
                },
                true,
            ),
            (
                "send",
                Command::Send {
                    run: RunArg { run: None },
                    name: name.clone(),
                    message: Some("Rebase on main.".to_owned()),
                    file: None,
                },
                true,
            ),
            ("flag", Command::Flag { name: name.clone() }, true),
            ("unflag", Command::Unflag { name: name.clone() }, true),
            (
                "alive",
                Command::Alive {
                    session: "ses_0a1b2c3d4e5f".to_owned(),
                },
                false,
            ),
            (
                "clean",
                Command::Clean {
                    run: RunArg { run: None },
                    dry_run: false,
                },
                false,
            ),
            (
                "park",
                Command::Park {
                    run: RunArg { run: None },
                    name: name.clone(),
                },
                false,
            ),
            ("go", Command::Go, false),
            ("gather", Command::Gather, false),
            ("desk", Command::Desk, false),
        ];

        for (case_name, command, expected) in cases {
            assert_eq!(takes_machine(&command), expected, "{case_name}");
        }
    }
}
