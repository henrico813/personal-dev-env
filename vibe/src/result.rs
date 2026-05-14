use serde::{Deserialize, Serialize};

/// Stable machine-readable outcome for `vibe run`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Completed,
    Noop,
    AgentFailed,
    CommitFailed,
    RefusedDirty,
    SnapshotFailed,
    WrapperFailed,
    SetupError,
}

#[derive(Debug, Clone, Serialize)]
pub struct RunResult {
    pub run_id: Option<String>,
    pub status: Status,
    pub branch: Option<String>,
    pub worktree: Option<String>,
    pub model: Option<String>,
    pub pre_run_commit: Option<String>,
    pub commit: Option<String>,
    pub snapshot_commits: Vec<String>,
    pub artifacts_dir: Option<String>,
    pub events_log_path: Option<String>,
    pub stderr_path: Option<String>,
    pub summary_path: Option<String>,
    pub changed_files: Vec<String>,
    pub persistence_error: Option<String>,
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
            Status::SnapshotFailed => 5,
            Status::WrapperFailed => 6,
            Status::SetupError => 7,
        }
    }

    pub fn setup_error(message: impl Into<String>) -> Self {
        Self {
            run_id: None,
            status: Status::SetupError,
            branch: None,
            worktree: None,
            model: None,
            pre_run_commit: None,
            commit: None,
            snapshot_commits: Vec::new(),
            artifacts_dir: None,
            events_log_path: None,
            stderr_path: None,
            summary_path: None,
            changed_files: Vec::new(),
            persistence_error: None,
            error_message: Some(message.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{RunResult, Status};

    fn sample_result(status: Status) -> RunResult {
        RunResult {
            run_id: Some("run-id".to_string()),
            status,
            branch: Some("vibe/pdev-049-demo".to_string()),
            worktree: Some("/tmp/worktree".to_string()),
            model: Some("openai-codex/gpt-5.4".to_string()),
            pre_run_commit: Some("abc".to_string()),
            commit: Some("def".to_string()),
            snapshot_commits: vec!["snap".to_string()],
            artifacts_dir: Some("/tmp/run".to_string()),
            events_log_path: Some("/tmp/run/events.jsonl".to_string()),
            stderr_path: Some("/tmp/run/agent.stderr.log".to_string()),
            summary_path: Some("/tmp/run/summary.json".to_string()),
            changed_files: Vec::new(),
            persistence_error: None,
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
            (Status::SnapshotFailed, 5),
            (Status::WrapperFailed, 6),
            (Status::SetupError, 7),
        ];

        for (status, code) in cases {
            assert_eq!(sample_result(status).exit_code(), code);
        }
    }

    #[test]
    fn result_serializes_snake_case() {
        let value =
            serde_json::to_value(sample_result(Status::AgentFailed)).expect("serialize result");

        assert_eq!(value["status"], "agent_failed");
        assert_eq!(value["run_id"], "run-id");
        assert_eq!(value["artifacts_dir"], "/tmp/run");
        assert_eq!(value["pre_run_commit"], "abc");
        assert_eq!(value["summary_path"], "/tmp/run/summary.json");
    }

    #[test]
    fn setup_error_uses_null_fields_for_early_failures() {
        let value = serde_json::to_value(RunResult::setup_error("boom")).expect("serialize");

        assert_eq!(value["status"], "setup_error");
        assert!(value["run_id"].is_null());
        assert!(value["branch"].is_null());
        assert!(value["artifacts_dir"].is_null());
        assert!(value["summary_path"].is_null());
        assert_eq!(value["changed_files"], serde_json::json!([]));
        assert!(value["persistence_error"].is_null());
        assert_eq!(value["error_message"], "boom");
    }
}
