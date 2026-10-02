use std::{
    fs::{File, OpenOptions},
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

#[cfg(test)]
pub(super) const AUTH_VARS: &[&str] = &[
    "ANTHROPIC_API_KEY",
    "OPENAI_API_KEY",
    "OPENROUTER_API_KEY",
    "GEMINI_API_KEY",
    "DEEPSEEK_API_KEY",
    "AZURE_OPENAI_API_KEY",
    "AZURE_OPENAI_BASE_URL",
    "OPENCODE_API_KEY",
    "GOOG_BASE_URL",
    "GOOG_API_KEY",
];
// These provider IDs match Pi's model selectors; unknown IDs cannot use env auth.
const AUTH_GROUPS: &[(&[&str], &[&str])] = &[
    (&["anthropic"], &["ANTHROPIC_API_KEY"]),
    (&["openai", "openai-codex"], &["OPENAI_API_KEY"]),
    (&["google", "gemini"], &["GEMINI_API_KEY"]),
    (&["deepseek"], &["DEEPSEEK_API_KEY"]),
    (
        &["azure-openai"],
        &["AZURE_OPENAI_API_KEY", "AZURE_OPENAI_BASE_URL"],
    ),
    (&["opencode", "opencode-go"], &["OPENCODE_API_KEY"]),
    (&["goog"], &["GOOG_BASE_URL", "GOOG_API_KEY"]),
    (&["openrouter"], &["OPENROUTER_API_KEY"]),
];
pub(super) const HOST_GIT_CONFIG_KEYS: &[(&str, &str)] = &[
    ("user.name", "VIBE_GIT_USER_NAME"),
    ("user.email", "VIBE_GIT_USER_EMAIL"),
];

fn env_var_is_set(key: &str) -> bool {
    std::env::var(key)
        .ok()
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
}

fn required_auth_group(model: &str) -> Option<&'static [&'static str]> {
    let provider = model.split_once('/')?.0;
    AUTH_GROUPS
        .iter()
        .find(|(providers, _)| providers.contains(&provider))
        .map(|(_, keys)| *keys)
}

fn has_provider_env(model: &str) -> bool {
    required_auth_group(model).is_some_and(|keys| keys.iter().all(|key| env_var_is_set(key)))
}

pub(super) fn auth_env_args(model: &str) -> Vec<String> {
    let Some(keys) =
        required_auth_group(model).filter(|keys| keys.iter().all(|key| env_var_is_set(key)))
    else {
        return Vec::new();
    };
    keys.iter()
        .flat_map(|key| ["-e".to_string(), (*key).to_string()])
        .collect()
}

pub(crate) fn prepare_provider_auth(
    home: Option<&str>,
    model: &str,
) -> Result<Option<PathBuf>, String> {
    if model.starts_with("openai-compatible/") {
        return Err("vibe no longer supports openai-compatible; use goog/<model>".to_string());
    }

    if let Some(model_name) = model.strip_prefix("goog/") {
        if model_name.trim().is_empty() {
            return Err("vibe requires a model after goog/".to_string());
        }
        // Goog requires endpoint credentials and never falls back to host Pi state.
        if has_provider_env(model) {
            return Ok(None);
        }
        return Err("vibe requires both Goog endpoint variables".to_string());
    }

    let pi_agent_dir = home.and_then(|home| {
        let pi_agent_dir = PathBuf::from(home).join(".pi/agent");
        let auth_file = pi_agent_dir.join("auth.json");
        let metadata = std::fs::metadata(&auth_file).ok()?;
        if !metadata.is_file() {
            return None;
        }
        File::open(auth_file).ok()?;

        // Pi rotates OAuth tokens and writes refresh locks beside auth.json.
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .ok()?
            .as_nanos();
        let probe = pi_agent_dir.join(format!(".vibe-write-check-{}-{nonce}", std::process::id()));
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&probe)
            .ok()?;
        std::fs::remove_file(probe).ok()?;
        Some(pi_agent_dir)
    });

    if has_provider_env(model) || pi_agent_dir.is_some() {
        Ok(pi_agent_dir)
    } else {
        Err("vibe requires provider auth via env vars or ~/.pi/agent/auth.json".to_string())
    }
}
