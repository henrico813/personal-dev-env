//! Sets `HUB_BUILD_VERSION`, the version `hub --version` prints. It is
//! `RELEASE_VERSION` when set, as the release workflow sets it. Else it
//! comes from the newest `hub-v` tag, else from Cargo.toml.

#[path = "build/version.rs"]
mod version;

use std::env;
use std::path::Path;
use std::process::Command;

fn main() {
    // Cargo reruns this script only when a file or variable named here
    // changes, so a kept `target/` folder would print an old version.
    println!("cargo::rerun-if-env-changed=RELEASE_VERSION");
    watch_git_refs();
    let version = env::var("RELEASE_VERSION")
        .ok()
        .filter(|given| !given.is_empty())
        .or_else(describe_hub_tag)
        .unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_owned());
    println!("cargo::rustc-env=HUB_BUILD_VERSION={version}");
}

/// Asks Cargo to rerun this script after a commit, a checkout or a new
/// tag, the changes that move what `git describe` prints. Outside a git
/// checkout it asks for nothing.
fn watch_git_refs() {
    let mut names = vec![
        "HEAD".to_owned(),
        "packed-refs".to_owned(),
        "refs/tags".to_owned(),
    ];
    // A detached HEAD names no branch, and `symbolic-ref` fails.
    names.extend(run_git(&["symbolic-ref", "-q", "HEAD"]));
    let mut args = vec!["rev-parse", "--path-format=absolute"];
    for name in &names {
        args.extend(["--git-path", name.as_str()]);
    }
    // `--git-path` finds HEAD in a worktree's own folder and the refs
    // in the main `.git` folder.
    let Some(paths) = run_git(&args) else {
        return;
    };
    // Cargo reruns the script on every build while a named path is
    // missing, such as `packed-refs` before git first packs refs.
    for path in paths.lines().filter(|path| Path::new(path).exists()) {
        println!("cargo::rerun-if-changed={path}");
    }
}

/// Returns the version from the newest `hub-v` tag before this commit,
/// or `None` outside a git checkout or before the first tag.
fn describe_hub_tag() -> Option<String> {
    let describe = run_git(&["describe", "--tags", "--match", "hub-v*"])?;
    version::version_from_describe(&describe)
}

/// Returns what `git` printed with `args`, trimmed, or `None` when it
/// fails or does not run.
fn run_git(args: &[&str]) -> Option<String> {
    let output = Command::new("git").args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let printed = String::from_utf8(output.stdout).ok()?;
    Some(printed.trim().to_owned())
}
