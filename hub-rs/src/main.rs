//! Runs one hub command and exits with its code. This is the one file
//! that reads environment variables, so tests pass in every value
//! instead. Read `main` to see every command in one place.

use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

use hub::commands::cli::{self, Command};
use hub::commands::{flag, sidebar};
use hub::external::herdr::HerdrCli;
use hub::shared::{HubError, Tools};

/// Parses the command line, builds the external systems, runs the
/// command, prints its lines or its error, and returns its exit code.
fn main() -> ExitCode {
    let cli = match cli::parse_command() {
        Ok(cli) => cli,
        Err(error) => return report_error(&error),
    };
    if cli.machine.is_some() && !cli::takes_machine(&cli.command) {
        let name = cli.command.name();
        return report_error(&HubError::Usage(format!(
            "{name} works on this machine only"
        )));
    }
    let herdr = HerdrCli::new(PathBuf::from("herdr"), cli.machine, find_socket());
    let tools = Tools { herdr: &herdr };
    let result = match &cli.command {
        Command::Flag { name } => flag::run_flag(&tools, name),
        Command::Unflag { name } => flag::run_unflag(&tools, name),
        Command::Go => sidebar::run_go(&tools),
        Command::Gather => sidebar::run_gather(&tools),
    };
    match result {
        Ok(outcome) => print_lines(&outcome.lines),
        Err(error) => report_error(&error),
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

/// Returns Herdr's socket from `HERDR_SOCKET_PATH`, which Herdr sets in
/// its panes and in commands its key bindings run, else Herdr's own
/// default path.
fn find_socket() -> PathBuf {
    match env::var_os("HERDR_SOCKET_PATH") {
        Some(socket) => PathBuf::from(socket),
        None => {
            PathBuf::from(env::var_os("HOME").unwrap_or_default()).join(".config/herdr/herdr.sock")
        }
    }
}

/// Prints the error after `hub: ` on standard error and returns its
/// exit code.
fn report_error(error: &HubError) -> ExitCode {
    eprintln!("hub: {error}");
    ExitCode::from(error.exit_code())
}
