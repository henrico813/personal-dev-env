//! Starts the review command without opening a review.

use std::io::{self, Write};
use std::process::ExitCode;

const HELP: &str = "pde-comment-review-next\n\nhelp    Show this help\n\nThe new review program is not ready for reviews yet.\n";

fn main() -> ExitCode {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() == 1 && args[0] == "help" {
        return match io::stdout().lock().write_all(HELP.as_bytes()) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("Cannot print help: {error}");
                ExitCode::FAILURE
            }
        };
    }
    eprintln!("Usage: pde-comment-review-next help");
    ExitCode::from(2)
}
