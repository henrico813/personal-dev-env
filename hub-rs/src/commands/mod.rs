//! Holds one module per hub command. Each command module runs one
//! command and returns an `Outcome` or a `HubError`, and never
//! prints, so tests read plain values. `cli.rs` is the exception.
//! It reads the command line, and clap prints usage and errors
//! there.

pub mod alive;
pub mod at;
pub mod auto;
pub mod brief;
pub mod clean;
pub mod cli;
pub mod flag;
pub mod handoff;
pub mod next;
pub mod reports;
pub mod send;
pub mod sidebar;
pub mod status;

/// The lines a command prints on standard output when it did its
/// work. The program then exits 0.
#[derive(Debug)]
pub struct Outcome {
    pub lines: Vec<String>,
}

impl Outcome {
    pub(crate) fn line(text: impl Into<String>) -> Outcome {
        Outcome {
            lines: vec![text.into()],
        }
    }
}
