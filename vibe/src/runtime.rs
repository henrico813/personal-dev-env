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
        contents: include_bytes!("../docker/Dockerfile"),
        executable: false,
    },
    Asset {
        rel: "docker/run-agent.sh",
        contents: include_bytes!("../docker/run-agent.sh"),
        executable: true,
    },
    Asset {
        rel: "extensions/git-snapshot.mjs",
        contents: include_bytes!("../extensions/git-snapshot.mjs"),
        executable: false,
    },
    Asset {
        rel: "extensions/jsonl-observer.mjs",
        contents: include_bytes!("../extensions/jsonl-observer.mjs"),
        executable: false,
    },
    Asset {
        rel: "hooks/post-commit",
        contents: include_bytes!("../hooks/post-commit"),
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
