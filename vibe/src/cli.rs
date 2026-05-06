use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(clap::ValueEnum, Clone, Copy, Debug, Eq, PartialEq)]
pub enum StderrLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

impl StderrLevel {
    pub fn as_str(self) -> &'static str {
        match self {
            StderrLevel::Error => "error",
            StderrLevel::Warn => "warn",
            StderrLevel::Info => "info",
            StderrLevel::Debug => "debug",
            StderrLevel::Trace => "trace",
        }
    }
}

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
        after_help = "Example:\n  vibe run --key pdev-049-demo --prompt-file /tmp/vibe-task.txt --model openai-codex/gpt-5.4\n\nArtifacts:\n  ~/.local/state/vibe/<repo>/<slug>/runs/..."
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

    /// Stderr verbosity for streamed Vibe progress.
    #[arg(long, env = "VIBE_STDERR_LEVEL", value_enum, default_value_t = StderrLevel::Info)]
    pub stderr_level: StderrLevel,

    /// Allow insecure TLS connections in Docker.
    #[arg(long)]
    pub insecure_tls: bool,
}

pub fn parse() -> RunArgs {
    try_parse_from(std::env::args_os()).unwrap_or_else(|err| err.exit())
}

pub fn try_parse_from<I, T>(itr: I) -> Result<RunArgs, clap::Error>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    let cli = Cli::try_parse_from(itr)?;
    match cli.command {
        Command::Run(mut args) => {
            if args.prompt_file.is_relative() {
                args.prompt_file = std::env::current_dir()
                    .expect("cwd")
                    .join(&args.prompt_file);
            }
            Ok(args)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{try_parse_from, StderrLevel};
    use std::{
        ffi::OsString,
        sync::{Mutex, OnceLock},
    };

    fn stderr_env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn save_stderr_env() -> Option<OsString> {
        std::env::var_os("VIBE_STDERR_LEVEL")
    }

    fn restore_stderr_env(saved: Option<OsString>) {
        if let Some(value) = saved {
            std::env::set_var("VIBE_STDERR_LEVEL", value);
        } else {
            std::env::remove_var("VIBE_STDERR_LEVEL");
        }
    }

    #[test]
    fn parses_run_arguments() {
        let _guard = stderr_env_lock().lock().expect("lock stderr env");
        let saved = save_stderr_env();
        std::env::remove_var("VIBE_STDERR_LEVEL");

        let args = try_parse_from([
            "vibe",
            "run",
            "--key",
            "demo",
            "--prompt-file",
            "/tmp/prompt.txt",
            "--model",
            "openai-codex/gpt-5.4-mini",
            "--commit-message",
            "docs: update note",
        ])
        .expect("parse args");

        restore_stderr_env(saved);

        assert_eq!(args.key, "demo");
        assert_eq!(
            args.prompt_file,
            std::path::PathBuf::from("/tmp/prompt.txt")
        );
        assert_eq!(args.model, "openai-codex/gpt-5.4-mini");
        assert_eq!(args.commit_message.as_deref(), Some("docs: update note"));
        assert_eq!(args.stderr_level, StderrLevel::Info);
        assert!(!args.insecure_tls);
    }

    #[test]
    fn parses_explicit_insecure_tls() {
        let args = try_parse_from([
            "vibe",
            "run",
            "--key",
            "demo",
            "--prompt-file",
            "/tmp/prompt.txt",
            "--model",
            "model",
            "--insecure-tls",
        ])
        .expect("parse args");

        assert!(args.insecure_tls);
    }

    #[test]
    fn parses_explicit_stderr_level() {
        let args = try_parse_from([
            "vibe",
            "run",
            "--key",
            "demo",
            "--prompt-file",
            "/tmp/prompt.txt",
            "--model",
            "model",
            "--stderr-level",
            "debug",
        ])
        .expect("parse args");

        assert_eq!(args.stderr_level, StderrLevel::Debug);
    }

    #[test]
    fn reads_stderr_level_from_env() {
        let _guard = stderr_env_lock().lock().expect("lock stderr env");
        let saved = save_stderr_env();
        std::env::set_var("VIBE_STDERR_LEVEL", "warn");

        let args = try_parse_from([
            "vibe",
            "run",
            "--key",
            "demo",
            "--prompt-file",
            "/tmp/prompt.txt",
            "--model",
            "model",
        ])
        .expect("parse args");

        restore_stderr_env(saved);
        assert_eq!(args.stderr_level, StderrLevel::Warn);
    }

    #[test]
    fn normalizes_relative_prompt_file() {
        let args = try_parse_from([
            "vibe",
            "run",
            "--key",
            "demo",
            "--prompt-file",
            "prompt.txt",
            "--model",
            "model",
        ])
        .expect("parse args");

        assert_eq!(
            args.prompt_file,
            std::env::current_dir().expect("cwd").join("prompt.txt")
        );
    }

    #[test]
    fn rejects_old_planner_step_shape() {
        let err = try_parse_from(["vibe", "run", "plan.md", "--step", "1", "--model", "model"])
            .expect_err("old shape should fail");

        assert_eq!(err.kind(), clap::error::ErrorKind::UnknownArgument);
    }

    #[test]
    fn requires_key_and_prompt_file() {
        assert!(try_parse_from(["vibe", "run", "--model", "model"]).is_err());
    }
}
