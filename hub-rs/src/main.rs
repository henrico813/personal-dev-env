//! Runs one hub command and exits with its code. This is the one file
//! that reads environment variables, so tests pass in every value
//! instead. Read `run_command` to see every command in one place.

use std::env;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

use hub::board::{self, Hubs};
use hub::commands::at::{AtRequest, Recipients};
use hub::commands::auto::AutoApproval;
use hub::commands::cli::{self, AutoRunArg, AutoStep, Cli, Command, RunArg, WatchStep};
use hub::commands::launch::LaunchRequest;
use hub::commands::next::NextOptions;
use hub::commands::reports::{CardChoice, ReadRequest};
use hub::commands::send::SendRequest;
use hub::commands::{
    Outcome, alive, at, auto, brief, clean, flag, handoff, launch, next, reports, send, sidebar,
    status,
};
use hub::external::cards::CardCli;
use hub::external::herdr::HerdrCli;
use hub::external::opencode::OpenCodeDb;
use hub::external::requests::RequestFolder;
use hub::shared::parking::ParkedFolder;
use hub::shared::run_folder::RunFolder;
use hub::shared::{HubError, OwnedTools};
use hub::watch::wait::{self, WaitRequest};
use hub::watch::{self, WatchOptions, WatchStart};
use jiff::Zoned;

/// How often `hub next` checks for news, `hub watch` checks the
/// workers, and `hub handoff` checks for the next hub's first reply,
/// when `HUB_POLL_SECONDS` is not set.
const DEFAULT_POLL: Duration = Duration::from_secs(5);

/// The wait between reads of a reopened agent's session ID.
const REOPEN_POLL: Duration = Duration::from_secs(1);

/// Parses the command line, runs the command, prints its lines or its
/// error, and returns its exit code.
fn main() -> ExitCode {
    match cli::parse_command().and_then(|cli| run_command(&cli)) {
        Ok(outcome) => print_lines(&outcome.lines),
        Err(error) => {
            eprintln!("hub: {error}");
            ExitCode::from(error.exit_code())
        }
    }
}

/// Prints each line on standard output and returns success. A reader
/// that closes early, as `head` does, ends the printing with no error.
/// Any other write failure prints its reason and returns 1.
fn print_lines(lines: &[String]) -> ExitCode {
    use std::io::{ErrorKind, Write};

    let mut stdout = std::io::stdout().lock();
    for line in lines {
        match writeln!(stdout, "{line}") {
            Ok(()) => {}
            Err(error) if error.kind() == ErrorKind::BrokenPipe => break,
            Err(error) => {
                eprintln!("hub: cannot write output: {error}");
                return ExitCode::FAILURE;
            }
        }
    }
    ExitCode::SUCCESS
}

/// Checks `--machine`, builds the external systems and runs the
/// command. A command finds its run folder before it reads anything
/// else, so a missing folder is reported first.
fn run_command(cli: &Cli) -> Result<Outcome, HubError> {
    if cli.machine.is_some() && !cli::takes_machine(&cli.command) {
        let name = cli.command.name();
        return Err(HubError::Usage(format!(
            "{name} works on this machine only"
        )));
    }
    let home = PathBuf::from(env::var_os("HOME").unwrap_or_default());
    let state_folder = home.join(".local/state/pde");
    let owned_tools = OwnedTools {
        herdr: HerdrCli::new(
            PathBuf::from("herdr"),
            cli.machine.clone(),
            find_socket(&home),
        ),
        cards: CardCli::new(PathBuf::from("card")),
        opencode: OpenCodeDb::new(home.join(".local/share/opencode/opencode.db")),
        requests: RequestFolder::new(find_request_folder(&home)),
        parked: ParkedFolder::new(state_folder.join("hub/parked")),
        own_pane: env::var("HERDR_PANE_ID").ok(),
        state_folder,
        reopen_poll: REOPEN_POLL,
    };
    let tools = owned_tools.lend();
    let hub_run = env::var_os("HUB_RUN");
    let local_state = home.join(".local/state");
    let find_run = |run: &RunArg| {
        cli::find_run_folder(run.run.as_deref(), hub_run.as_deref(), Some(&local_state))
    };
    let find_auto_run =
        |run: &AutoRunArg| cli::find_run_folder(run.run.as_deref(), hub_run.as_deref(), None);
    let now = Zoned::now();
    match &cli.command {
        Command::Status { run, json } => status::run_status(&tools, &find_run(run)?, *json, &now),
        Command::Next {
            run,
            timeout,
            json,
            start_reviews,
        } => {
            let run = find_run(run)?;
            let timeout = match timeout {
                Some(seconds) => Some(Duration::try_from_secs_f64(*seconds).map_err(|_| {
                    HubError::Usage(format!("--timeout {seconds} is not a number of seconds"))
                })?),
                None => None,
            };
            let options = NextOptions {
                timeout,
                poll: read_poll_interval()?,
                json: *json,
                process_id: std::process::id(),
                cwd: read_current_folder()?,
                start_reviews: *start_reviews,
            };
            next::run_next(&tools, &run, &options)
        }
        Command::Watch { step } => match step {
            WatchStep::Start { run } => {
                let run = find_run(run)?;
                let start = WatchStart {
                    program: find_this_program()?,
                    session_id: env::var("OPENCODE_SESSION_ID").ok(),
                    runs_folder: local_state.clone(),
                };
                watch::run_watch_start(&tools, &run, &start, &now)
            }
            WatchStep::Run => {
                let options = WatchOptions {
                    poll: read_poll_interval()?,
                    checks: None,
                    runs_folder: local_state.clone(),
                    notifier: PathBuf::from("moshi-hook"),
                };
                // The watch log, or systemd's journal, gets this output.
                watch::run_watch(&tools, &options, &mut io::stderr())
            }
        },
        Command::Send {
            run,
            name,
            message,
            file,
        } => {
            let run = find_run(run)?;
            // clap lets exactly one of MESSAGE and `--file` through.
            let text = match file {
                Some(path) => fs::read_to_string(path).map_err(|source| HubError::File {
                    path: path.clone(),
                    source,
                })?,
                None => message.clone().unwrap_or_default(),
            };
            let request = SendRequest {
                name: name.clone(),
                machine: cli.machine.clone(),
                text,
            };
            send::run_send(&tools, &run, &request, &now)
        }
        Command::Inbox { run, name } => send::run_inbox(&find_run(run)?, name, &now),
        Command::At {
            run,
            time,
            to,
            all,
            file,
        } => {
            let run = find_run(run)?;
            let text = fs::read_to_string(file).map_err(|source| HubError::File {
                path: file.clone(),
                source,
            })?;
            // clap lets exactly one of `--to` and `--all` through.
            let to = if *all {
                Recipients::AllWorkers
            } else {
                Recipients::Names(to.iter().map(|name| name.as_str().to_owned()).collect())
            };
            let request = AtRequest {
                time: *time,
                to,
                file: file.clone(),
                text,
            };
            at::run_at(&run, request, &now)
        }
        Command::Wait {
            run,
            name,
            pid,
            file,
            deadline,
        } => {
            let run = find_run(run)?;
            let request = WaitRequest {
                name: name.clone(),
                pid: *pid,
                file: file.clone(),
                deadline_minutes: *deadline,
            };
            let mut outcome = wait::run_wait(&run, &request, &now)?;
            outcome
                .lines
                .extend(restart_watch(&run, tools.state_folder));
            Ok(outcome)
        }
        Command::Read {
            run,
            name,
            note,
            card,
        } => {
            let run = find_run(run)?;
            let request = ReadRequest {
                name: name.clone(),
                note: note.clone(),
                card: match card {
                    None => CardChoice::NoCard,
                    Some(None) => CardChoice::FromName,
                    Some(Some(key)) => CardChoice::Given(key.clone()),
                },
            };
            reports::run_read(&tools, &run, &request, &now)
        }
        Command::Report { run, name, file } => {
            let run = find_run(run)?;
            let text = read_report_text(file.as_deref())?;
            let mut outcome = reports::run_report(&run, name, &text)?;
            outcome
                .lines
                .extend(restart_watch(&run, tools.state_folder));
            Ok(outcome)
        }
        Command::Note { run, text } => reports::run_note(&find_run(run)?, text, &now),
        Command::Brief { run } => brief::run_brief(&tools, &find_run(run)?),
        Command::Handoff { run, name, handoff } => {
            let run = find_run(run)?;
            let poll = read_poll_interval()?;
            let cwd = read_current_folder()?;
            handoff::run_handoff(&tools, &run, name, handoff, &cwd, &now, poll)
        }
        Command::Launch {
            run,
            name,
            handoff,
            model,
            workspace,
        } => {
            let run = find_run(run)?;
            let request = LaunchRequest {
                name: name.clone(),
                handoff: handoff.clone(),
                model: model.clone(),
                workspace: workspace.clone(),
                cwd: read_current_folder()?,
            };
            launch::run_launch(&tools, &run, &request, &now)
        }
        Command::Adopt { run, old } => handoff::run_adopt(&tools, &find_run(run)?, old, &now),
        Command::Clean { run, dry_run } => {
            clean::run_clean(&tools, &find_run(run)?, *dry_run, &now)
        }
        Command::Park { run, name } => clean::run_park(&tools, &find_run(run)?, name, &now),
        Command::Auto { step } => match step {
            AutoStep::Ready { run } => auto::run_auto_ready(&tools, &find_auto_run(run)?),
            AutoStep::Check { run } => auto::run_auto_check(&tools, &find_auto_run(run)?),
            AutoStep::Approve {
                run,
                given,
                number,
                reply,
            } => {
                let run = find_auto_run(run)?;
                let approval = AutoApproval {
                    number: *number,
                    reply: reply.clone(),
                    given: *given,
                };
                auto::run_auto_approve(&tools, &run, &approval, &now)
            }
            AutoStep::Now { run } => auto::run_auto_now(&tools, &find_auto_run(run)?),
        },
        Command::Flag { name } => flag::run_flag(&tools, name),
        Command::Unflag { name } => flag::run_unflag(&tools, name),
        Command::Alive { session } => alive::run_alive(&tools, session, &now),
        Command::Go => sidebar::run_go(&tools),
        Command::Gather => sidebar::run_gather(&tools),
        Command::Desk => sidebar::run_desk(&tools, &home),
        Command::Board {
            only_run,
            print,
            dump,
            control,
            listen,
            send,
            size,
        } => {
            // The board shows every hub, so it ignores `HUB_RUN`, which
            // names one.
            let hubs = match only_run {
                Some(path) => Hubs::Given(path.clone()),
                None => Hubs::Live(local_state.clone()),
            };
            let time_zone = now.time_zone().clone();
            if let Some(step) = send {
                board::send_step(tools.state_folder, step)
            } else if *print {
                board::print_board(&tools, &hubs, &time_zone, *size)
            } else if *dump {
                board::dump_board(&tools, &hubs, &time_zone, *size)
            } else if *control {
                let mut input = std::io::stdin().lock();
                let mut output = std::io::stdout().lock();
                board::control_board(&tools, &hubs, &time_zone, *size, &mut input, &mut output)
            } else {
                board::run_board(owned_tools, hubs, time_zone, *listen)
            }
        }
    }
}

/// Returns Herdr's socket from `HERDR_SOCKET_PATH`, which Herdr sets in
/// its panes and in commands its key bindings run, else Herdr's own
/// default path.
fn find_socket(home: &Path) -> PathBuf {
    match env::var_os("HERDR_SOCKET_PATH") {
        Some(socket) => PathBuf::from(socket),
        None => home.join(".config/herdr/herdr.sock"),
    }
}

/// Returns the folder pde-gh-write writes its requests to. Like
/// pde-gh-write, it uses `XDG_STATE_HOME` when that is set and not
/// empty, else `~/.local/state`.
fn find_request_folder(home: &Path) -> PathBuf {
    let state_home = env::var_os("XDG_STATE_HOME")
        .filter(|folder| !folder.is_empty())
        .map_or_else(|| home.join(".local/state"), PathBuf::from);
    state_home.join("pde/pr-write")
}

/// Returns the report text from FILE, or from standard input when FILE
/// is `-` or left out.
fn read_report_text(file: Option<&Path>) -> Result<String, HubError> {
    match file {
        Some(path) if path != Path::new("-") => {
            fs::read_to_string(path).map_err(|source| HubError::File {
                path: path.to_owned(),
                source,
            })
        }
        _ => {
            let mut text = String::new();
            io::stdin()
                .read_to_string(&mut text)
                .map_err(|source| HubError::File {
                    path: PathBuf::from("standard input"),
                    source,
                })?;
            Ok(text)
        }
    }
}

/// Returns the folder this command runs in, where the sessions it
/// starts open.
fn read_current_folder() -> Result<PathBuf, HubError> {
    env::current_dir().map_err(|source| HubError::File {
        path: PathBuf::from("."),
        source,
    })
}

/// Returns this program's path, which `hub watch start` runs again in
/// the background as `hub watch run`.
fn find_this_program() -> Result<PathBuf, HubError> {
    env::current_exe().map_err(|source| HubError::File {
        path: PathBuf::from("hub"),
        source,
    })
}

/// Starts a dead watch again for the run, as `hub report` and
/// `hub wait` do after their work. A program path that cannot be read
/// only skips the restart, since that work is already saved.
fn restart_watch(run: &RunFolder, state_folder: &Path) -> Option<String> {
    let program = env::current_exe().ok()?;
    watch::restart_stopped_watch(run, state_folder, &program)
}

/// Returns `HUB_POLL_SECONDS` as a duration, such as `0.5` for half a
/// second, else five seconds.
fn read_poll_interval() -> Result<Duration, HubError> {
    let Some(text) = env::var_os("HUB_POLL_SECONDS") else {
        return Ok(DEFAULT_POLL);
    };
    let text = text.to_string_lossy();
    text.parse::<f64>()
        .ok()
        .and_then(|seconds| Duration::try_from_secs_f64(seconds).ok())
        .ok_or_else(|| {
            HubError::Usage(format!(
                "HUB_POLL_SECONDS is {text}, not a number of seconds"
            ))
        })
}
