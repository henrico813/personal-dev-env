use std::error::Error;
use std::io::{self, Write};
use std::process::ExitCode;

use rust_skill_examples::retry::parse_retry_count;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            // There is no further reporting destination if stderr also fails.
            let _ = writeln!(io::stderr().lock(), "error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let mut arguments = std::env::args_os().skip(1);
    let Some(input) = arguments.next() else {
        return Err("usage: retry-count COUNT".into());
    };
    if arguments.next().is_some() {
        return Err("usage: retry-count COUNT".into());
    }

    let input = input.to_str().ok_or("COUNT must be valid UTF-8")?;
    let count = parse_retry_count(input)?;
    writeln!(io::stdout().lock(), "{count}")?;
    Ok(())
}
