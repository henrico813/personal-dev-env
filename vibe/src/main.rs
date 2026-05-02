mod cli;
mod docker;
mod git;
mod output;
mod paths;
mod planner;
mod prompt;
mod run;

use std::process;

fn main() {
    let args = cli::parse();
    let result = run::execute(args);
    let json = serde_json::to_string_pretty(&result).expect("serialize result");
    println!("{}", json);
    process::exit(result.exit_code());
}
