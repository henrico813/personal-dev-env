use std::fs;
use std::path::{Path, PathBuf};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

struct Asset {
    rel: &'static str,
    contents: &'static [u8],
    executable: bool,
}

const VERSION: &str = env!("CARGO_PKG_VERSION");
const ASSETS: &[Asset] = &[
    Asset {
        rel: "docker/Dockerfile",
        contents: include_bytes!("../../docker/Dockerfile"),
        executable: false,
    },
    Asset {
        rel: "docker/run-agent.sh",
        contents: include_bytes!("../../docker/run-agent.sh"),
        executable: true,
    },
    Asset {
        rel: "extensions/git-snapshot.mjs",
        contents: include_bytes!("../../extensions/git-snapshot.mjs"),
        executable: false,
    },
    Asset {
        rel: "extensions/jsonl-observer.mjs",
        contents: include_bytes!("../../extensions/jsonl-observer.mjs"),
        executable: false,
    },
    Asset {
        rel: "extensions/stderr-progress.mjs",
        contents: include_bytes!("../../extensions/stderr-progress.mjs"),
        executable: false,
    },
    Asset {
        rel: "hooks/post-commit",
        contents: include_bytes!("../../hooks/post-commit"),
        executable: true,
    },
];

pub fn ensure_runtime_assets() -> Result<PathBuf, String> {
    let home = std::env::var("HOME").map_err(|_| "HOME not set".to_string())?;
    let root = PathBuf::from(home).join(".local/share/vibe").join(VERSION);

    for asset in ASSETS {
        let path = root.join(asset.rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("create runtime asset dir {}: {e}", parent.display()))?;
        }

        let needs_write = match fs::read(&path) {
            Ok(existing) => existing != asset.contents,
            Err(_) => true,
        };
        if needs_write {
            fs::write(&path, asset.contents)
                .map_err(|e| format!("write runtime asset {}: {e}", path.display()))?;
        }

        set_executable(&path, asset.executable)?;
    }

    Ok(root)
}

fn set_executable(path: &Path, executable: bool) -> Result<(), String> {
    if !executable {
        return Ok(());
    }

    #[cfg(unix)]
    {
        let mut perms = fs::metadata(path)
            .map_err(|e| format!("read runtime asset metadata {}: {e}", path.display()))?
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(path, perms)
            .map_err(|e| format!("chmod runtime asset {}: {e}", path.display()))?;
    }

    Ok(())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::io;
    use std::os::unix::fs::PermissionsExt;
    use std::process::{Command, Output};
    use std::thread;
    use std::time::{Duration, Instant};
    use tempfile::tempdir;

    fn write_executable(path: &Path, contents: &str) {
        fs::write(path, contents).expect("write script");
        let mut perms = fs::metadata(path).expect("script metadata").permissions();
        perms.set_mode(0o755);
        fs::set_permissions(path, perms).expect("chmod script");
    }

    fn run_executable<F>(mut spawn: F) -> Output
    where
        F: FnMut() -> io::Result<Output>,
    {
        let deadline = Instant::now() + Duration::from_secs(1);
        loop {
            match spawn() {
                Ok(output) => return output,
                Err(error)
                    if error.kind() == io::ErrorKind::ExecutableFileBusy
                        && Instant::now() < deadline =>
                {
                    // Forked test children can briefly retain the script write descriptor.
                    thread::sleep(Duration::from_millis(10));
                }
                Err(error) => panic!("run runtime shell: {error}"),
            }
        }
    }

    #[test]
    fn shipped_shell_preserves_model_and_prompt() {
        let temp = tempdir().expect("tempdir");
        let bin = temp.path().join("bin");
        let home = temp.path().join("home");
        let repo_root = temp.path().join("repo");
        let capture = temp.path().join("captured-prompt.bin");
        let args_capture = temp.path().join("captured-args.bin");
        let combined_prompt = temp.path().join("combined-prompt.txt");
        let script = temp.path().join("run-agent.sh");

        fs::create_dir_all(&bin).expect("mkdir bin");
        fs::create_dir_all(&home).expect("mkdir home");
        let user_skill = home.join(".agents/skills/normal-skill");
        fs::create_dir_all(&user_skill).expect("mkdir user skill");
        fs::write(
            user_skill.join("SKILL.md"),
            "---\nname: normal-skill\ndescription: A reviewed user skill.\n---\nDo the thing.\n",
        )
        .expect("write user skill");
        let repository_skills = repo_root.join(".agents/skills");
        let repository_skill = repository_skills.join("repository-skill");
        fs::create_dir_all(&repository_skill).expect("mkdir repository skill");
        fs::write(
            repository_skill.join("SKILL.md"),
            "---\nname: repository-skill\ndescription: A repository skill.\n---\nDo the thing.\n",
        )
        .expect("write repository skill");
        fs::write(&combined_prompt, b"Line one\nLine two\n").expect("write prompt");
        fs::write(&script, include_bytes!("../../docker/run-agent.sh")).expect("write script");
        let mut script_perms = fs::metadata(&script)
            .expect("script metadata")
            .permissions();
        script_perms.set_mode(0o755);
        fs::set_permissions(&script, script_perms).expect("chmod script");

        write_executable(
            &bin.join("git"),
            "#!/usr/bin/env bash\nset -euo pipefail\nexit 0\n",
        );
        write_executable(
            &bin.join("node"),
            "#!/usr/bin/env bash\nset -euo pipefail\ncat >/dev/null\n",
        );
        write_executable(
            &bin.join("pi"),
            concat!(
                "#!/usr/bin/env bash\n",
                "set -euo pipefail\n",
                "printf '%s\\0' \"$@\" > \"$PI_ARGS_CAPTURE_FILE\"\n",
                "last=\"${!#}\"\n",
                "printf '%s' \"$last\" > \"$PI_CAPTURE_FILE\"\n",
            ),
        );

        let status = run_executable(|| {
            Command::new(&script)
                .current_dir(&repo_root)
                .env("HOME", &home)
                .env(
                    "PATH",
                    format!(
                        "{}:{}",
                        bin.display(),
                        std::env::var("PATH").unwrap_or_default()
                    ),
                )
                .env("PI_CAPTURE_FILE", &capture)
                .env("PI_ARGS_CAPTURE_FILE", &args_capture)
                .env("VIBE_REPO_ROOT", &repo_root)
                .env("VIBE_REPO_SKILLS_DIR", &repository_skills)
                .env("VIBE_COMBINED_PROMPT_FILE", &combined_prompt)
                .env("VIBE_MODEL", "fake-provider/fake-model")
                .output()
        });

        assert!(
            status.status.success(),
            "stdout: {}\nstderr: {}",
            String::from_utf8_lossy(&status.stdout),
            String::from_utf8_lossy(&status.stderr),
        );

        // The fake Pi records its arguments before consuming the prompt. This
        // verifies shell quoting without Docker, credentials, or network access.
        let captured_args = fs::read(&args_capture).expect("read captured Pi arguments");
        let pi_args: Vec<&[u8]> = captured_args
            .split(|byte| *byte == 0)
            .filter(|arg| !arg.is_empty())
            .collect();
        let model_position = pi_args
            .iter()
            .position(|arg| *arg == b"--model")
            .expect("Pi receives --model");
        assert_eq!(
            std::str::from_utf8(pi_args[model_position + 1]).expect("UTF-8 model selector"),
            "fake-provider/fake-model"
        );
        let selected_skills: Vec<&[u8]> = pi_args
            .windows(2)
            .filter(|pair| pair[0] == b"--skill")
            .map(|pair| pair[1])
            .collect();
        assert!(pi_args.iter().any(|arg| *arg == b"--no-skills"));
        assert!(!pi_args.iter().any(
            |arg| *arg == b"/opt/vibe/.pi/agent/npm/node_modules/pi-models-discovery/index.ts"
        ));
        assert!(!home.join(".pi/agent/models.json").exists());
        assert_eq!(
            selected_skills,
            [
                user_skill.to_string_lossy().as_bytes(),
                repository_skill.to_string_lossy().as_bytes(),
            ]
        );
        assert_eq!(
            fs::read(&capture).expect("read captured prompt"),
            b"Line one\nLine two\n"
        );
    }

    #[test]
    fn shell_excludes_workflow_skills_from_pi_selection() {
        let temp = tempdir().expect("tempdir");
        let bin = temp.path().join("bin");
        let home = temp.path().join("home");
        let repo_root = temp.path().join("repo");
        let args_capture = temp.path().join("captured-args.bin");
        let combined_prompt = temp.path().join("combined-prompt.txt");
        let script = temp.path().join("run-agent.sh");

        let user_skills = home.join(".agents/skills");
        let workflow_skill = user_skills.join("workflow-skill");
        fs::create_dir_all(&workflow_skill).expect("mkdir workflow skill");
        fs::write(
            workflow_skill.join("SKILL.md"),
            "---\nname: workflow-skill\ndescription: Runs the planning workflow.\nmetadata:\n  pde-workflow: \"true\"\n---\nOrchestrate.\n",
        )
        .expect("write workflow skill");
        let user_skill = user_skills.join("normal-skill");
        fs::create_dir_all(&user_skill).expect("mkdir user skill");
        fs::write(
            user_skill.join("SKILL.md"),
            "---\nname: normal-skill\ndescription: A reviewed user skill.\n---\nDo the thing.\n",
        )
        .expect("write user skill");
        // A body mention outside the frontmatter is not a workflow marker.
        let body_marker_skill = user_skills.join("body-marker-skill");
        fs::create_dir_all(&body_marker_skill).expect("mkdir body marker skill");
        fs::write(
            body_marker_skill.join("SKILL.md"),
            "---\nname: body-marker-skill\ndescription: Mentions the marker in its body.\n---\npde-workflow: \"true\" appears in the body only.\n",
        )
        .expect("write body marker skill");
        let repository_skills = repo_root.join(".agents/skills");
        let repository_workflow_skill = repository_skills.join("repository-workflow-skill");
        fs::create_dir_all(&repository_workflow_skill).expect("mkdir repository workflow skill");
        fs::write(
            repository_workflow_skill.join("SKILL.md"),
            "---\nname: repository-workflow-skill\ndescription: Runs a shared workflow.\nmetadata:\n  pde-workflow: \"true\"\n---\nOrchestrate.\n",
        )
        .expect("write repository workflow skill");
        let repository_skill = repository_skills.join("repository-skill");
        fs::create_dir_all(&repository_skill).expect("mkdir repository skill");
        fs::write(
            repository_skill.join("SKILL.md"),
            "---\nname: repository-skill\ndescription: A repository skill.\n---\nDo the thing.\n",
        )
        .expect("write repository skill");
        fs::write(&combined_prompt, b"Prompt\n").expect("write prompt");
        fs::write(&script, include_bytes!("../../docker/run-agent.sh")).expect("write script");
        let mut script_perms = fs::metadata(&script).expect("script metadata").permissions();
        script_perms.set_mode(0o755);
        fs::set_permissions(&script, &script_perms).expect("chmod script");
        write_executable(
            &bin.join("git"),
            "#!/usr/bin/env bash\nset -euo pipefail\nexit 0\n",
        );
        write_executable(
            &bin.join("node"),
            "#!/usr/bin/env bash\nset -euo pipefail\ncat >/dev/null\n",
        );
        write_executable(
            &bin.join("pi"),
            concat!(
                "#!/usr/bin/env bash\n",
                "set -euo pipefail\n",
                "printf '%s\\0' \"$@\" > \"$PI_ARGS_CAPTURE_FILE\"\n",
            ),
        );

        let status = run_executable(|| {
            Command::new(&script)
                .current_dir(&repo_root)
                .env("HOME", &home)
                .env(
                    "PATH",
                    format!(
                        "{}:{}",
                        bin.display(),
                        std::env::var("PATH").unwrap_or_default()
                    ),
                )
                .env("PI_ARGS_CAPTURE_FILE", &args_capture)
                .env("VIBE_REPO_ROOT", &repo_root)
                .env("VIBE_REPO_SKILLS_DIR", &repository_skills)
                .env("VIBE_COMBINED_PROMPT_FILE", &combined_prompt)
                .env("VIBE_MODEL", "fake-provider/fake-model")
                .output()
        });

        assert!(
            status.status.success(),
            "stdout: {}\nstderr: {}",
            String::from_utf8_lossy(&status.stdout),
            String::from_utf8_lossy(&status.stderr),
        );

        let captured_args = fs::read(&args_capture).expect("read captured Pi arguments");
        let pi_args: Vec<&[u8]> = captured_args
            .split(|byte| *byte == 0)
            .filter(|arg| !arg.is_empty())
            .collect();
        assert!(pi_args.iter().any(|arg| *arg == b"--no-skills"));
        let selected_skills: Vec<&[u8]> = pi_args
            .windows(2)
            .filter(|pair| pair[0] == b"--skill")
            .map(|pair| pair[1])
            .collect();
        assert_eq!(
            selected_skills,
            [
                user_skill.to_string_lossy().as_bytes(),
                body_marker_skill.to_string_lossy().as_bytes(),
                repository_skill.to_string_lossy().as_bytes(),
            ]
        );
        for arg in &pi_args {
            let text = std::str::from_utf8(arg).expect("UTF-8 argument");
            assert!(!text.contains("workflow"), "workflow skill selected: {text}");
        }
    }

    #[test]
    fn goog_shell_writes_discovery_config() {
        let temp = tempdir().expect("tempdir");
        let bin = temp.path().join("bin");
        let home = temp.path().join("home");
        let repo_root = temp.path().join("repo");
        let args_capture = temp.path().join("captured-args.bin");
        let combined_prompt = temp.path().join("combined-prompt.txt");
        let script = temp.path().join("run-agent.sh");

        fs::create_dir_all(&bin).expect("mkdir bin");
        fs::create_dir_all(&home).expect("mkdir home");
        fs::create_dir_all(&repo_root).expect("mkdir repo");
        fs::write(&combined_prompt, b"Prompt\n").expect("write prompt");
        let real_node = Command::new("node")
            .arg("-p")
            .arg("process.execPath")
            .output()
            .expect("resolve Node executable")
            .stdout;
        let real_node = String::from_utf8(real_node)
            .expect("Node executable path is UTF-8")
            .trim()
            .to_owned();
        fs::write(&script, include_bytes!("../../docker/run-agent.sh")).expect("write script");
        let mut script_perms = fs::metadata(&script)
            .expect("script metadata")
            .permissions();
        script_perms.set_mode(0o755);
        fs::set_permissions(&script, script_perms).expect("chmod script");
        write_executable(
            &bin.join("git"),
            "#!/usr/bin/env bash\nset -euo pipefail\nexit 0\n",
        );
        write_executable(
            &bin.join("node"),
            concat!(
                "#!/usr/bin/env bash\n",
                "set -euo pipefail\n",
                "if [[ \"${1:-}\" == \"-e\" ]]; then\n",
                "  exec \"$REAL_NODE\" \"$@\"\n",
                "fi\n",
                "cat >/dev/null\n",
            ),
        );
        write_executable(
            &bin.join("pi"),
            concat!(
                "#!/usr/bin/env bash\n",
                "set -euo pipefail\n",
                "printf '%s\\0' \"$@\" > \"$PI_ARGS_CAPTURE_FILE\"\n",
            ),
        );

        let status = run_executable(|| {
            Command::new(&script)
                .current_dir(&repo_root)
                .env("HOME", &home)
                .env(
                    "PATH",
                    format!(
                        "{}:{}",
                        bin.display(),
                        std::env::var("PATH").unwrap_or_default()
                    ),
                )
                .env("PI_ARGS_CAPTURE_FILE", &args_capture)
                .env("REAL_NODE", &real_node)
                .env("VIBE_REPO_ROOT", &repo_root)
                .env("VIBE_COMBINED_PROMPT_FILE", &combined_prompt)
                .env("VIBE_MODEL", "goog/qwen3.8")
                .env("GOOG_BASE_URL", "https://models.example/v1")
                .env("GOOG_API_KEY", "unused")
                .output()
        });

        assert!(
            status.status.success(),
            "stdout: {}\nstderr: {}",
            String::from_utf8_lossy(&status.stdout),
            String::from_utf8_lossy(&status.stderr),
        );

        let captured_args = fs::read(&args_capture).expect("read captured Pi arguments");
        let pi_args: Vec<&[u8]> = captured_args
            .split(|byte| *byte == 0)
            .filter(|arg| !arg.is_empty())
            .collect();
        assert!(pi_args.iter().any(
            |arg| *arg == b"/opt/vibe/.pi/agent/npm/node_modules/pi-models-discovery/index.ts"
        ));
        let config: serde_json::Value = serde_json::from_slice(
            &fs::read(home.join(".pi/agent/models.json")).expect("read models config"),
        )
        .expect("parse models config");
        let provider = &config["providers"]["goog"];
        assert_eq!(provider["baseUrl"], "https://models.example/v1");
        assert_eq!(provider["api"], "openai-completions");
        assert_eq!(provider["apiKey"], "$GOOG_API_KEY");
        assert_eq!(provider["discoverModels"], true);
        assert!(provider.get("models").is_none());
    }
}
