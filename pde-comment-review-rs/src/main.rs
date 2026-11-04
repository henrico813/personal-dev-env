//! Starts the review command, which prints its help or shows the Review Screen.

use std::io::{self, Write};
use std::process::ExitCode;

use pde_comment_review::review_screen::{
    ScreenKind, StartFailure, run_open_command, run_screen_command,
};

const HELP: &str = "pde-comment-review-next\n\nhelp           Show this help\nopen [NAME]    Show the review of branch NAME, or pick a waiting one\nscreen --popup Show the review in Herdr's popup, for Ctrl+b e\nscreen --tab   Show the review in a Herdr tab, for Ctrl+b E\n";

fn main() -> ExitCode {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let args: Vec<_> = args.iter().map(|arg| arg.to_str()).collect();
    let started = match args.as_slice() {
        [Some("help")] => {
            return match io::stdout().lock().write_all(HELP.as_bytes()) {
                Ok(()) => ExitCode::SUCCESS,
                Err(error) => {
                    eprintln!("Cannot print help: {error}");
                    ExitCode::FAILURE
                }
            };
        }
        [Some("screen"), Some("--popup")] => run_screen_command(ScreenKind::Popup),
        [Some("screen"), Some("--tab")] => run_screen_command(ScreenKind::Tab),
        [Some("open")] => run_open_command(None),
        [Some("open"), Some(branch)] => run_open_command(Some(branch)),
        _ => {
            eprintln!(
                "Usage: pde-comment-review-next help | open [NAME] | screen --popup | screen --tab"
            );
            return ExitCode::from(2);
        }
    };
    match started {
        Ok(()) => ExitCode::SUCCESS,
        Err(StartFailure(message)) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}
