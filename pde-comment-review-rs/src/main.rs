//! This module sends each command to its step of the Review Process, in step
//! order, so this one file shows the whole process. `commands/mcp.rs` must send
//! each MCP tool to the same step, or MCP and the CLI give agents different
//! replies. Read the `main()` function to understand the module.

use std::ffi::OsString;
use std::path::Path;
use std::process::ExitCode;

use pde_comment_review::commands::cli::{self, Command};
use pde_comment_review::commands::move_old_approvals::move_old_approvals;
use pde_comment_review::commands::{Reply, mcp, resolve_base};
use pde_comment_review::review_screen::{StartFailure, run_open_command, run_screen_command};
use pde_comment_review::steps::check_comments::check_comments;
use pde_comment_review::steps::make_feedback_request_list::make_feedback_request_list;
use pde_comment_review::steps::send_agent_suggestions::send_agent_suggestions;
use pde_comment_review::steps::show_status::show_status;

/// Runs the command, which is usually one step of the Review Process.
fn main() -> ExitCode {
    #[cfg(feature = "test-util")]
    pde_comment_review::shared::review_session::use_test_waiting_list_from_environment();
    let args: Vec<OsString> = std::env::args_os().collect();
    let command = match cli::parse_command(&args) {
        Ok(command) => command,
        Err(usage) => {
            eprint!("{usage}");
            return ExitCode::from(2);
        }
    };
    // The command line works on the folder it runs in.
    let dir = Path::new(".");
    let reply = match command {
        Command::CheckComments { base } => Reply::from_outcome(
            resolve_base(dir, base.as_deref()).and_then(|base| check_comments(dir, base)),
        ),
        Command::MakeFeedbackRequestList { base, input_file } => {
            Reply::from_outcome(resolve_base(dir, base.as_deref()).and_then(|base| {
                let change_requests = cli::read_change_requests(input_file.as_deref())?;
                make_feedback_request_list(dir, base, &change_requests)
            }))
        }
        Command::SendAgentSuggestions { file } => Reply::from_outcome(
            cli::read_list_file(&file).and_then(|list| send_agent_suggestions(dir, list)),
        ),
        Command::Screen { kind } => return report_screen_outcome(run_screen_command(kind)),
        Command::Open { name } => return report_screen_outcome(run_open_command(name.as_deref())),
        Command::MoveOldApprovals => Reply::from_move_outcome(move_old_approvals(dir)),
        Command::ShowStatus => Reply::from_outcome(show_status(dir)),
        Command::Mcp => {
            return match mcp::serve_mcp() {
                Ok(()) => ExitCode::SUCCESS,
                Err(error) => {
                    eprintln!("The MCP server stopped. {error}");
                    ExitCode::FAILURE
                }
            };
        }
        Command::Help => return cli::print_help(),
    };
    cli::print_reply(&reply)
}

/// Returns 0 once the screen closes, or prints why it could not open and
/// returns 1.
fn report_screen_outcome(shown: Result<(), StartFailure>) -> ExitCode {
    match shown {
        Ok(()) => ExitCode::SUCCESS,
        Err(StartFailure(message)) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}
