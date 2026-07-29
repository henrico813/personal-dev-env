mod chunk;
mod gather;
mod index;
mod merge;
mod research;
mod schema;
mod source;
mod taskfile;

use clap::{ArgGroup, Args, Parser, Subcommand};
use std::convert::TryFrom;
use std::error::Error;
use std::io;
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
    Merge(MergeArgs),
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
#[command(group(
    ArgGroup::new("task_destination")
        .required(true)
        .multiple(false)
        .args(["output_dir", "task"])
))]
struct NewTaskArgs {
    output_dir: Option<PathBuf>,

    #[arg(long, conflicts_with = "output_dir")]
    task: Option<String>,

    #[arg(long, requires = "task", conflicts_with = "output_dir")]
    root: Option<PathBuf>,
}

#[derive(Debug, PartialEq, Eq)]
enum NewTaskMode {
    Explicit { output_dir: PathBuf },
    ManagedNew { task: String },
    ManagedAppend { root: PathBuf, task: String },
}

impl TryFrom<NewTaskArgs> for NewTaskMode {
    type Error = io::Error;

    fn try_from(args: NewTaskArgs) -> Result<Self, Self::Error> {
        match (args.output_dir, args.root, args.task) {
            (Some(output_dir), None, None) => Ok(Self::Explicit { output_dir }),
            (None, None, Some(task)) => Ok(Self::ManagedNew { task }),
            (None, Some(root), Some(task)) => Ok(Self::ManagedAppend { root, task }),
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "use <output-dir>, --task <name>, or --root <root> --task <name>",
            )),
        }
    }
}

#[derive(Args)]
struct IndexArgs {
    #[arg(long)]
    repo: PathBuf,
}

#[derive(Args)]
struct MergeArgs {
    #[arg(required = true, num_args = 1.., value_name = "TASK_REPORT")]
    reports: Vec<PathBuf>,
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
            NewCommand::Task(args) => match NewTaskMode::try_from(args)? {
                NewTaskMode::Explicit { output_dir } => {
                    taskfile::run(&output_dir).map_err(Into::into)
                }
                NewTaskMode::ManagedNew { task } => {
                    let root = taskfile::create_managed_task(&taskfile::state_root()?, &task)?;
                    println!("{}", root.display());
                    Ok(())
                }
                NewTaskMode::ManagedAppend { root, task } => {
                    taskfile::append_managed_task(&root, &task)?;
                    println!("{}", root.display());
                    Ok(())
                }
            },
        },
        Command::Index(args) => index::build_chunk_index(&args.repo),
        Command::Merge(args) => merge::run(&args.reports),
        Command::Research(args) => research::run(&args.context, &args.trace_out),
    }
}

#[cfg(test)]
mod tests {
    use super::{Cli, Command, NewCommand, NewTaskMode};
    use clap::error::ErrorKind;
    use clap::Parser;
    use std::convert::TryFrom;
    use std::path::PathBuf;

    #[test]
    fn parses_new_task_modes() {
        let cases = [
            (
                vec!["surveil", "new", "task", "/tmp/tasks"],
                Some(NewTaskMode::Explicit {
                    output_dir: PathBuf::from("/tmp/tasks"),
                }),
                None,
            ),
            (
                vec!["surveil", "new", "task", "--task", "architecture"],
                Some(NewTaskMode::ManagedNew {
                    task: "architecture".to_string(),
                }),
                None,
            ),
            (
                vec![
                    "surveil",
                    "new",
                    "task",
                    "--root",
                    "/state/run",
                    "--task",
                    "tests",
                ],
                Some(NewTaskMode::ManagedAppend {
                    root: PathBuf::from("/state/run"),
                    task: "tests".to_string(),
                }),
                None,
            ),
            (
                vec!["surveil", "new", "task"],
                None,
                Some(ErrorKind::MissingRequiredArgument),
            ),
            (
                vec![
                    "surveil",
                    "new",
                    "task",
                    "/tmp/tasks",
                    "--task",
                    "architecture",
                ],
                None,
                Some(ErrorKind::ArgumentConflict),
            ),
        ];

        for (args, expected, error) in cases {
            match Cli::try_parse_from(args) {
                Ok(Cli {
                    command: Command::New(args),
                }) => match args.command {
                    NewCommand::Task(args) => assert_eq!(
                        NewTaskMode::try_from(args).expect("task mode"),
                        expected.expect("expected mode"),
                    ),
                },
                Ok(_) => panic!("expected new task command"),
                Err(err) => assert_eq!(err.kind(), error.expect("expected error")),
            }
        }
    }

    #[test]
    fn parses_merge_reports() {
        let cli = Cli::try_parse_from(["surveil", "merge", "/tmp/architecture.json", "/tmp/tests.json"])
            .expect("parse merge command");

        match cli.command {
            Command::Merge(args) => assert_eq!(
                args.reports,
                vec![
                    PathBuf::from("/tmp/architecture.json"),
                    PathBuf::from("/tmp/tests.json"),
                ]
            ),
            _ => panic!("expected merge command"),
        }
    }

    #[test]
    fn merge_requires_report() {
        match Cli::try_parse_from(["surveil", "merge"]) {
            Ok(_) => panic!("expected missing argument error"),
            Err(err) => assert_eq!(err.kind(), ErrorKind::MissingRequiredArgument),
        }
    }
}
