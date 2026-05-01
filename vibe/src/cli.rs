use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "vibe", version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Run one planner step inside the Vibe sandbox.
    Run(RunArgs),
}

#[derive(clap::Args, Clone, Debug)]
pub struct RunArgs {
    /// Absolute or relative path to the planner issue markdown.
    pub plan: PathBuf,

    /// 1-based implementation step number.
    #[arg(long)]
    pub step: u32,

    /// Pi model identifier, for example anthropic/claude-sonnet-4-6.
    #[arg(long)]
    pub model: String,
}

pub fn parse() -> RunArgs {
    let cli = Cli::parse();
    match cli.command {
        Command::Run(mut args) => {
            if args.plan.is_relative() {
                args.plan = std::env::current_dir().expect("cwd").join(&args.plan);
            }
            args
        }
    }
}
