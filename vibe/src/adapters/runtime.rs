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
    use std::os::unix::fs::PermissionsExt;
    use std::process::Command;
    use tempfile::tempdir;

    fn write_executable(path: &Path, contents: &str) {
        fs::write(path, contents).expect("write script");
        let mut perms = fs::metadata(path).expect("script metadata").permissions();
        perms.set_mode(0o755);
        fs::set_permissions(path, perms).expect("chmod script");
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
        fs::create_dir_all(home.join(".agents/skills")).expect("mkdir skills");
        fs::create_dir_all(&repo_root).expect("mkdir repo");
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

        let status = Command::new(&script)
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
            .env("VIBE_COMBINED_PROMPT_FILE", &combined_prompt)
            .env("VIBE_MODEL", "fake-provider/fake-model")
            .output()
            .expect("run runtime shell");

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
        let skill_position = pi_args
            .iter()
            .position(|arg| *arg == b"--skill")
            .expect("Pi receives --skill");
        assert!(pi_args.iter().any(|arg| *arg == b"--no-skills"));
        assert_eq!(
            std::str::from_utf8(pi_args[skill_position + 1]).expect("UTF-8 skill path"),
            home.join(".agents/skills").to_string_lossy()
        );
        assert_eq!(
            fs::read(&capture).expect("read captured prompt"),
            b"Line one\nLine two\n"
        );
    }
}
