//! Rust-owned executor prompt contract.
//!
//! This module keeps the immutable prompt text, contract metadata, and exact
//! rendering rules in one place so the tests can lock the public surface.

#[derive(Clone, Copy)]
struct SystemPrompt {
    name: &'static str,
    version: &'static str,
    body: &'static str,
}

#[derive(Clone, Copy)]
struct PromptContract {
    version: &'static str,
    task_header: &'static str,
    prompts: &'static [SystemPrompt],
}

pub(crate) struct RenderedPrompt {
    pub(crate) system_prompt: String,
    pub(crate) combined_prompt: String,
    pub(crate) version_manifest: String,
}

const EXECUTION_FOCUS_PROMPT: SystemPrompt = SystemPrompt {
    name: "execution_focus",
    version: "v1",
    body: r#"The supervisor provides the task for this run. Your job is to implement that repository change and nothing else.

Execution rules:
- Follow the supervisor's instructions exactly within these runtime rules.
- Work only on the requested repository change. Do not broaden scope.
- Inspect only the files needed to locate edit points or run minimal verification.
- Do not perform broad repository exploration, architecture review, option generation, or system-internals investigation.
- Do not inspect runtime, tooling, sandbox, auth, editor, CI, or prompt internals unless the supervisor explicitly requests it or the requested repository change cannot be completed without it.
- As soon as you find a plausible edit location, start editing. Do not continue searching for extra context unless blocked or verification fails.
- Make the smallest correct change. Do not refactor unrelated code. Do not add optional improvements.
- If there is not a single obvious implementation path, report the ambiguity briefly instead of exploring alternatives.
- After editing, run only the smallest relevant verification requested by the supervisor or directly implied by the changed code.
- When the requested change is implemented and verified, stop."#,
};

const SNAPSHOT_COMMIT_PROMPT: SystemPrompt = SystemPrompt {
    name: "snapshot_commit",
    version: "v1",
    body: r#"Maintain /artifacts/commit-message.txt as the snapshot subject for repository changes made during this run.
- Keep exactly one line.
- Use an unscoped conventional commit subject unless explicitly instructed otherwise.
- If the task is clear, you may write an initial subject before editing.
- Before finishing, update the subject to match the actual repository changes.
- If no repository changes were made, do not leave a misleading subject.
- Do not create commit-message.txt in the repository.
- Do not run git commit."#,
};

const EXECUTOR_PROMPT_CONTRACT: PromptContract = PromptContract {
    version: "v1",
    task_header: "Task:\n",
    prompts: &[EXECUTION_FOCUS_PROMPT, SNAPSHOT_COMMIT_PROMPT],
};

pub(crate) fn render_executor_prompt(supervisor_prompt: &str) -> RenderedPrompt {
    let system_prompt = EXECUTOR_PROMPT_CONTRACT.prompts[0].body.to_string();
    let combined_prompt = format!("{}{}", EXECUTOR_PROMPT_CONTRACT.task_header, supervisor_prompt);
    let version_manifest = contract_version_manifest(&EXECUTOR_PROMPT_CONTRACT);

    RenderedPrompt {
        system_prompt,
        combined_prompt,
        version_manifest,
    }
}

fn contract_version_manifest(contract: &PromptContract) -> String {
    let mut lines = Vec::with_capacity(contract.prompts.len() + 1);
    lines.push(format!("executor_prompt_contract={}", contract.version));
    lines.extend(
        contract
            .prompts
            .iter()
            .map(|prompt| format!("{}={}", prompt.name, prompt.version)),
    );
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locks_metadata_and_version_manifest() {
        assert_eq!(EXECUTOR_PROMPT_CONTRACT.version, "v1");
        assert_eq!(EXECUTOR_PROMPT_CONTRACT.task_header, "Task:\n");
        assert_eq!(EXECUTOR_PROMPT_CONTRACT.prompts.len(), 2);
        assert_eq!(EXECUTOR_PROMPT_CONTRACT.prompts[0].name, "execution_focus");
        assert_eq!(EXECUTOR_PROMPT_CONTRACT.prompts[0].version, "v1");
        assert_eq!(EXECUTOR_PROMPT_CONTRACT.prompts[1].name, "snapshot_commit");
        assert_eq!(EXECUTOR_PROMPT_CONTRACT.prompts[1].version, "v1");
        assert_eq!(render_executor_prompt("").version_manifest, "executor_prompt_contract=v1\nexecution_focus=v1\nsnapshot_commit=v1");
    }

    #[test]
    fn locks_system_and_combined_prompt_rendering() {
        let rendered = render_executor_prompt("Update README.");

        assert_eq!(rendered.system_prompt, EXECUTION_FOCUS_PROMPT.body);
        assert_eq!(rendered.combined_prompt, "Task:\nUpdate README.");
    }
}
