mod chunk;
mod gather;
mod index;
mod research;
mod schema;
mod source;
mod taskfile;

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
    New(NewArgs),
    Index(IndexArgs),
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
struct NewArgs {
    #[command(subcommand)]
    command: NewCommand,
}

#[derive(Subcommand)]
enum NewCommand {
    Task(NewTaskArgs),
}

#[derive(Args)]
struct NewTaskArgs {
    output_dir: PathBuf,
}

#[derive(Args)]
struct IndexArgs {
    #[arg(long)]
    repo: PathBuf,
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
        Command::New(args) => match args.command {
            NewCommand::Task(args) => taskfile::run(&args.output_dir).map_err(Into::into),
        },
        Command::Index(args) => index::run(&args.repo),
        Command::Research(args) => research::run(&args.context, &args.trace_out),
    }
}

#[cfg(test)]
mod tests {
    use super::{Cli, Command, NewCommand};
    use clap::error::ErrorKind;
    use clap::Parser;
    use std::path::PathBuf;

    #[test]
    fn parses_new_task_output_dir() {
        let cli = Cli::try_parse_from(["surveil", "new", "task", "/tmp/tasks"])
            .expect("parse new task command");

        match cli.command {
            Command::New(args) => match args.command {
                NewCommand::Task(args) => {
                    assert_eq!(args.output_dir, PathBuf::from("/tmp/tasks"));
                }
            },
            _ => panic!("expected new command"),
        }
    }

    #[test]
    fn new_task_requires_output_dir() {
        match Cli::try_parse_from(["surveil", "new", "task"]) {
            Ok(_) => panic!("expected missing argument error"),
            Err(err) => assert_eq!(err.kind(), ErrorKind::MissingRequiredArgument),
        }
    }
}
