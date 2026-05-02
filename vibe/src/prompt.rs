use serde_json::Value;

/// Keep day-1 prompts deterministic so artifacts explain exactly what ran.
pub fn compose(step_json: &Value) -> Result<String, String> {
    let step =
        serde_json::to_string_pretty(step_json).map_err(|e| format!("render step JSON: {e}"))?;
    Ok(format!(
        "Implement exactly this planner step in the current worktree.\n\nFollow the step literally. Keep changes inside the mounted repository. Do not create tests in this MVP unless the step explicitly requires them. Leave the worktree in a committable state.\n\nStep JSON:\n{step}",
    ))
}

#[cfg(test)]
mod tests {
    use super::compose;
    use serde_json::json;

    #[test]
    fn prompt_includes_step_json() {
        let prompt = compose(&json!({
            "title": "Create generated.txt",
            "summary": "Write one file"
        }))
        .expect("compose prompt");

        assert!(prompt.contains("Implement exactly this planner step"));
        assert!(prompt.contains("Do not create tests in this MVP"));
        assert!(prompt.contains("Create generated.txt"));
        assert!(prompt.contains("Write one file"));
    }
}
