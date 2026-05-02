use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Prefer the shared planner binary so Vibe follows the same issue schema.
pub fn locate_planner() -> Option<PathBuf> {
    let preferred = PathBuf::from(std::env::var("HOME").ok()?).join(".claude/bin/planner");
    if preferred.exists() {
        return Some(preferred);
    }
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths).find_map(|dir| {
            let candidate = dir.join("planner");
            candidate.exists().then_some(candidate)
        })
    })
}

pub fn inspect(planner_bin: &Path, plan: &Path) -> Result<Value, String> {
    let out = Command::new(planner_bin)
        .args(["inspect", plan.to_str().unwrap_or("")])
        .output()
        .map_err(|e| format!("spawn planner: {e}"))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    serde_json::from_slice(&out.stdout).map_err(|e| format!("parse planner JSON: {e}"))
}

pub fn extract_step(plan_json: &Value, step: u32) -> Result<Value, String> {
    let steps = plan_json
        .get("implementation")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "planner output missing implementation[]".to_string())?;
    let index = step
        .checked_sub(1)
        .ok_or_else(|| "step must be >= 1".to_string())? as usize;
    steps
        .get(index)
        .cloned()
        .ok_or_else(|| format!("step {step} out of range"))
}

pub fn step_title(step_json: &Value, step: u32) -> String {
    step_json
        .get("title")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("step {step}"))
}

#[cfg(test)]
mod tests {
    use super::{extract_step, step_title};
    use serde_json::json;

    #[test]
    fn step_extraction_returns_value() {
        let plan = json!({
            "implementation": [
                { "title": "first" },
                { "title": "second" }
            ]
        });

        let step = extract_step(&plan, 2).expect("extract step");
        assert_eq!(step.get("title").and_then(|v| v.as_str()), Some("second"));
    }

    #[test]
    fn missing_steps_array_errors() {
        let err = extract_step(&json!({}), 1).expect_err("missing implementation");
        assert_eq!(err, "planner output missing implementation[]");
    }

    #[test]
    fn step_zero_errors() {
        let plan = json!({ "implementation": [{ "title": "only" }] });
        let err = extract_step(&plan, 0).expect_err("step zero");
        assert_eq!(err, "step must be >= 1");
    }

    #[test]
    fn step_out_of_range_errors() {
        let plan = json!({ "implementation": [{ "title": "only" }] });
        let err = extract_step(&plan, 2).expect_err("out of range");
        assert_eq!(err, "step 2 out of range");
    }

    #[test]
    fn title_falls_back_to_step() {
        assert_eq!(step_title(&json!({}), 7), "step 7");
    }
}
