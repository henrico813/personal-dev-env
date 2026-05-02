use serde::Serialize;

/// Stable day-1 machine-readable outcome for `vibe run`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Completed,
    Noop,
    AgentFailed,
    CommitFailed,
    RefusedDirty,
    SetupError,
}

#[derive(Debug, Clone, Serialize)]
pub struct RunResult {
    pub status: Status,
    pub step: Option<u32>,
    pub branch: Option<String>,
    pub worktree: Option<String>,
    pub model: Option<String>,
    pub pre_step_commit: Option<String>,
    pub commit: Option<String>,
    pub snapshot_commits: Vec<String>,
    pub events_log_path: Option<String>,
    pub stderr_path: Option<String>,
    pub error_message: Option<String>,
}

impl RunResult {
    pub fn exit_code(&self) -> i32 {
        match self.status {
            Status::Completed => 0,
            Status::Noop => 1,
            Status::AgentFailed => 2,
            Status::CommitFailed => 3,
            Status::RefusedDirty => 4,
            Status::SetupError => 5,
        }
    }

    pub fn setup_error(message: impl Into<String>) -> Self {
        Self {
            status: Status::SetupError,
            step: None,
            branch: None,
            worktree: None,
            model: None,
            pre_step_commit: None,
            commit: None,
            snapshot_commits: Vec::new(),
            events_log_path: None,
            stderr_path: None,
            error_message: Some(message.into()),
        }
    }
}
