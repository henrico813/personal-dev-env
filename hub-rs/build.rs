//! Sets `HUB_BUILD_VERSION`, the version `hub --version` prints. It is
//! `RELEASE_VERSION` when set, as the release workflow sets it. Else it
//! comes from the newest `hub-v` tag, else from Cargo.toml.

#[path = "build/version.rs"]
mod version;

use std::env;
use std::process::Command;

fn main() {
    // Cargo reruns this script only when `RELEASE_VERSION` or the script
    // changes. A kept `target/` folder still prints the tag it saw at its
    // last run.
    println!("cargo::rerun-if-env-changed=RELEASE_VERSION");
    let version = env::var("RELEASE_VERSION")
        .ok()
        .filter(|given| !given.is_empty())
        .or_else(describe_hub_tag)
        .unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_owned());
    println!("cargo::rustc-env=HUB_BUILD_VERSION={version}");
}

/// Returns the version from the newest `hub-v` tag before this commit,
/// or `None` outside a git checkout or before the first tag.
fn describe_hub_tag() -> Option<String> {
    let output = Command::new("git")
        .args(["describe", "--tags", "--match", "hub-v*"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let describe = String::from_utf8(output.stdout).ok()?;
    version::version_from_describe(describe.trim())
}
