use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "vibe",
    version,
    about = "Run an agent task in an isolated Vibe worktree.",
    after_help = "Goals:\n  - enforce worktree execution\n  - capture observable run artifacts\n  - record snapshot commits for rollback\n  - sandbox the agent in Docker\n\nOutput:\n  stdout prints one final JSON object"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    #[command(
        about = "Run one agent task inside a managed worktree.",
        after_help = "Example:\n  vibe run --key pdev-049-demo --prompt-file /tmp/vibe-task.txt --model openai-codex/gpt-5.4\n\nArtifacts:\n  ~/.local/state/vibe/<repo>/<key>/runs/..."
    )]
    Run(RunArgs),
}

#[derive(clap::Args, Clone, Debug)]
pub struct RunArgs {
    /// Stable identifier for the managed Vibe worktree.
    #[arg(long)]
    pub key: String,

    /// Prompt file to execute inside the managed worktree.
    #[arg(long)]
    pub prompt_file: PathBuf,

    /// Pi model identifier, for example openai-codex/gpt-5.4.
    #[arg(long)]
    pub model: String,

    /// Optional commit message for dirty runs.
    #[arg(long)]
    pub commit_message: Option<String>,
}

pub fn parse() -> RunArgs {
    let cli = Cli::parse();
    match cli.command {
        Command::Run(mut args) => {
            if args.prompt_file.is_relative() {
                args.prompt_file = std::env::current_dir()
                    .expect("cwd")
                    .join(&args.prompt_file);
            }
            args
        }
    }
}
