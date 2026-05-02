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

#[cfg(test)]
mod tests {
    use super::{RunResult, Status};

    fn sample_result(status: Status) -> RunResult {
        RunResult {
            status,
            step: Some(1),
            branch: Some("branch".to_string()),
            worktree: Some("/tmp/worktree".to_string()),
            model: Some("openai-codex/gpt-5.5".to_string()),
            pre_step_commit: Some("abc".to_string()),
            commit: Some("def".to_string()),
            snapshot_commits: vec!["snap".to_string()],
            events_log_path: Some("/tmp/events.jsonl".to_string()),
            stderr_path: Some("/tmp/agent.stderr.log".to_string()),
            error_message: None,
        }
    }

    #[test]
    fn exit_codes_match_status() {
        let cases = [
            (Status::Completed, 0),
            (Status::Noop, 1),
            (Status::AgentFailed, 2),
            (Status::CommitFailed, 3),
            (Status::RefusedDirty, 4),
            (Status::SetupError, 5),
        ];

        for (status, code) in cases {
            assert_eq!(sample_result(status).exit_code(), code);
        }
    }

    #[test]
    fn status_serializes_snake_case() {
        let value =
            serde_json::to_value(sample_result(Status::AgentFailed)).expect("serialize result");

        assert_eq!(value["status"], "agent_failed");
        assert_eq!(value["worktree"], "/tmp/worktree");
        assert!(value.get("snapshot_commits").is_some());
        assert!(value.get("events_log_path").is_some());
    }

    #[test]
    fn setup_error_uses_null_fields() {
        let result = RunResult::setup_error("planner not found");
        let value = serde_json::to_value(&result).expect("serialize setup error");

        assert_eq!(result.exit_code(), 5);
        assert!(matches!(result.status, Status::SetupError));
        assert!(value["step"].is_null());
        assert!(value["branch"].is_null());
        assert!(value["worktree"].is_null());
        assert!(value["commit"].is_null());
        assert_eq!(value["error_message"], "planner not found");
    }
}
