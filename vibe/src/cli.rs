use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "vibe",
    version,
    about = "Run one planner step in the Vibe Docker sandbox.",
    after_help = "Step lifecycle:\n  - Vibe refuses reruns of steps that already have result commits.\n  - Append follow-up plan steps for review feedback.\n\nPrereqs:\n  - run inside a git repo\n  - docker available locally\n  - planner available in ~/.claude/bin or PATH\n  - provider auth supplied via env vars or ~/.pi/agent/auth.json\n\nArtifacts:\n  ~/.local/state/vibe/<repo>/<branch>/runs/...\n\nOutput:\n  stdout prints one final JSON object"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    #[command(
        about = "Run one planner step inside the Vibe sandbox.",
        after_help = "Workflow:\n  1. Vibe resolves or creates the plan worktree.\n  2. If the step already has a result commit, Vibe returns refused_step_already_run.\n  3. Otherwise Vibe runs bundled Pi in Docker with only Vibe extensions.\n  4. stdout returns one final JSON object; logs land in ~/.local/state/vibe/...\n\nPrereqs:\n  - docker available locally\n  - planner available in ~/.claude/bin or PATH\n  - provider auth supplied via env vars or ~/.pi/agent/auth.json"
    )]
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
