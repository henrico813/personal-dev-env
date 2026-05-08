mod schema;

use std::env;
use std::error::Error;
use std::io;

fn main() {
    if let Err(err) = run() {
        eprintln!("{err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let mut args = env::args().skip(1);
    let command = args.next().ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "expected a command"))?;

    match command.as_str() {
        "gather" => Err(io::Error::new(io::ErrorKind::Unsupported, "gather is not implemented yet").into()),
        "research" => Err(io::Error::new(io::ErrorKind::Unsupported, "research is not implemented yet").into()),
        "trace" => Err(io::Error::new(io::ErrorKind::Unsupported, "trace is not implemented yet").into()),
        _ => Err(io::Error::new(io::ErrorKind::InvalidInput, format!("unknown command: {command}")).into()),
    }
}
