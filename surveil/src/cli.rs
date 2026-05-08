mod gather;
mod research;
mod schema;

use clap::{Args, Parser, Subcommand};
use std::error::Error;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "surveil")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Gather(GatherArgs),
    Research(ResearchArgs),
}

#[derive(Args)]
struct GatherArgs {
    #[arg(long)]
    repo: PathBuf,

    #[arg(long = "task-file")]
    task_file: PathBuf,
}

#[derive(Args)]
struct ResearchArgs {
    #[arg(long)]
    context: PathBuf,

    #[arg(long = "trace-out")]
    trace_out: PathBuf,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("{err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();

    match cli.command {
        Command::Gather(args) => gather::run(&args.repo, &args.task_file),
        Command::Research(args) => research::run(&args.context, &args.trace_out),
    }
}
