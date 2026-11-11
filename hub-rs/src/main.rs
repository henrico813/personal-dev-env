//! Runs one hub command and exits with its code. This is the one file
//! that reads environment variables, so tests pass in every value
//! instead. Read `run_command` to see every command in one place.

use std::env;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

use hub::commands::auto::AutoApproval;
use hub::commands::cli::{self, AutoStep, Cli, Command, RunArg};
use hub::commands::next::NextOptions;
use hub::commands::reports::{CardChoice, ReadRequest};
use hub::commands::send::SendRequest;
use hub::commands::{Outcome, alive, auto, brief, flag, handoff, next, reports, send, sidebar};
use hub::outside::cards::CardCli;
use hub::outside::herdr::HerdrCli;
use hub::outside::opencode::OpenCodeDb;
use hub::shared::{HubError, Tools};
use jiff::Zoned;

/// How often `hub next` checks for news, and `hub handoff` for the
/// next hub's first reply, when `HUB_POLL_SECONDS` is not set.
const DEFAULT_POLL: Duration = Duration::from_secs(5);

/// Parses the command line, runs the command, prints its lines or its
/// error, and returns its exit code.
fn main() -> ExitCode {
    match cli::parse_command().and_then(|cli| run_command(&cli)) {
        Ok(outcome) => {
            for line in outcome.lines {
                println!("{line}");
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("hub: {error}");
            ExitCode::from(error.exit_code())
        }
    }
}

/// Checks `--machine`, builds the outside systems and runs the
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
    let herdr = HerdrCli::new(
        PathBuf::from("herdr"),
        cli.machine.clone(),
        find_socket(&home),
    );
    let cards = CardCli::new(PathBuf::from("card"));
    let opencode = OpenCodeDb::new(home.join(".local/share/opencode/opencode.db"));
    let own_pane = env::var("HERDR_PANE_ID").ok();
    let state_folder = home.join(".local/state/pde");
    let tools = Tools {
        herdr: &herdr,
        cards: &cards,
        opencode: &opencode,
        own_pane: own_pane.as_deref(),
        state_folder: &state_folder,
    };
    let hub_run = env::var_os("HUB_RUN");
    let find_run = |run: &RunArg| cli::find_run_folder(run.run.as_deref(), hub_run.as_deref());
    let now = Zoned::now();
    match &cli.command {
        Command::Next { run, timeout, json } => {
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
            };
            next::run_next(&tools, &run, &options)
        }
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
            reports::run_report(&run, name, &text)
        }
        Command::Note { run, text } => reports::run_note(&find_run(run)?, text, &now),
        Command::Brief { run } => brief::run_brief(&tools, &find_run(run)?),
        Command::Handoff { run, name, handoff } => {
            let run = find_run(run)?;
            let poll = read_poll_interval()?;
            let cwd = env::current_dir().map_err(|source| HubError::File {
                path: PathBuf::from("."),
                source,
            })?;
            handoff::run_handoff(&tools, &run, name, handoff, &cwd, &now, poll)
        }
        Command::Auto { step } => match step {
            AutoStep::Ready { run } => auto::run_auto_ready(&tools, &find_run(run)?),
            AutoStep::Check { run } => auto::run_auto_check(&tools, &find_run(run)?),
            AutoStep::Approve {
                run,
                given,
                number,
                reply,
            } => {
                let run = find_run(run)?;
                let approval = AutoApproval {
                    number: *number,
                    reply: reply.clone(),
                    given: *given,
                };
                auto::run_auto_approve(&tools, &run, &approval, &now)
            }
            AutoStep::Now { run } => auto::run_auto_now(&find_run(run)?),
        },
        Command::Flag { name } => flag::run_flag(&tools, name),
        Command::Unflag { name } => flag::run_unflag(&tools, name),
        Command::Alive { session } => alive::run_alive(&tools, session, &now),
        Command::Go => sidebar::run_go(&tools),
        Command::Gather => sidebar::run_gather(&tools),
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
