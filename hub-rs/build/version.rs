//! Turns `git describe` output for a `hub-v` tag into the version
//! `hub --version` prints. `build.rs` and `tests/version.rs` both
//! include this file, so the test checks the code the build runs.

/// Returns `0.3.0` for the tag `hub-v0.3.0`, and `0.3.0+4.g1a2b3c4`
/// for a build four commits past it at commit `1a2b3c4`. Returns `None`
/// when the text does not start with `hub-v`.
pub fn version_from_describe(describe: &str) -> Option<String> {
    let rest = describe.strip_prefix("hub-v")?;
    // Past a tag, `git describe` appends `-<commits>-g<hash>`.
    match rest.rsplitn(3, '-').collect::<Vec<_>>()[..] {
        [hash, commits, tag] if hash.starts_with('g') && commits.parse::<u32>().is_ok() => {
            Some(format!("{tag}+{commits}.{hash}"))
        }
        _ => Some(rest.to_owned()),
    }
}
