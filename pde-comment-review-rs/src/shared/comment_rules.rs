//! Names the rules a comment can break, and where one broke.

use std::path::PathBuf;

/// Holds one comment that breaks a rule.
#[derive(Debug, PartialEq)]
pub struct Finding {
    pub file: PathBuf,
    pub name: Option<String>,
    pub line: usize,
    pub rule: Rule,
}

/// Names each rule a comment can break.
#[derive(Debug, PartialEq)]
pub enum Rule {
    /// Marks a changed comment whose name has approved texts, none equal to its text.
    Approved,
    /// Marks an approved comment the branch removed while its code stays.
    ApprovedRemoved,
}
