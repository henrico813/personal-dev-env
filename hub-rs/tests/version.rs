//! Checks the version text the build script makes from `git describe`.

#[path = "../build/version.rs"]
mod version;

use version::version_from_describe;

/// A build on a release tag prints that release's number. A build past
/// it also names how many commits past and which commit, so a bug
/// report from a source build points at the exact code.
#[test]
fn version_from_describe_reads_hub_tags() {
    let tagged = version_from_describe("hub-v0.3.0");
    let past_tag = version_from_describe("hub-v0.3.0-4-g1a2b3c4");

    assert_eq!(tagged.as_deref(), Some("0.3.0"));
    assert_eq!(past_tag.as_deref(), Some("0.3.0+4.g1a2b3c4"));
}
