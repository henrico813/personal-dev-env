mod adapters;
mod app;
mod cli;
mod observe;
mod result;
mod sandbox;
mod snapshot;
mod worktree;

use std::process;

fn main() {
    let args = cli::parse();
    let result = app::execute(args);
    let json = serde_json::to_string_pretty(&result).expect("serialize result");
    println!("{}", json);
    process::exit(result.exit_code());
}
