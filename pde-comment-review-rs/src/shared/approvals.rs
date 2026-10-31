//! Describes why a comment disagrees with a previous decision.

/// Distinguishes a restored cut from a different approved wording.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Conflict {
    Cut,
    OtherText,
}
