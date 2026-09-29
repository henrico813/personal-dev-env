use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use crate::adapters::git::RepoLayout;

const KEY_FILE: &str = "key";

/// Identity and paths shared by one run invocation.
pub(crate) struct RunTarget {
    key: String,
    slug: String,
    repo_id: String,
    repo_root: PathBuf,
    git_common_dir: PathBuf,
}

impl RunTarget {
    /// Keep the raw Git common directory for Git and Docker; hash its resolved
    /// path so symlinked checkouts share the same state identity.
    pub(crate) fn from_repo(key: &str, repo: RepoLayout) -> Result<Self, String> {
        let hash_input = repo
            .git_common_dir
            .canonicalize()
            .map_err(|error| format!("canonicalize git common dir: {error}"))?;
        Ok(Self::from_parts_with_hash(
            key,
            repo.repo_root,
            repo.git_common_dir,
            &hash_input,
        ))
    }

    #[cfg(test)]
    pub(crate) fn from_parts(key: &str, repo_root: PathBuf, git_common_dir: PathBuf) -> Self {
        Self::from_parts_with_hash(key, repo_root, git_common_dir.clone(), &git_common_dir)
    }

    fn from_parts_with_hash(
        key: &str,
        repo_root: PathBuf,
        git_common_dir: PathBuf,
        hash_input: &Path,
    ) -> Self {
        let basename = repo_root
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("repo");
        Self {
            key: key.to_string(),
            slug: slugify(key),
            repo_id: format!(
                "{basename}-{:016x}",
                fnv1a64(hash_input.as_os_str().as_bytes())
            ),
            repo_root,
            git_common_dir,
        }
    }

    pub(crate) fn key(&self) -> &str {
        &self.key
    }
    pub(crate) fn slug(&self) -> &str {
        &self.slug
    }
    pub(crate) fn repo_root(&self) -> &Path {
        &self.repo_root
    }
    pub(crate) fn git_common_dir(&self) -> &Path {
        &self.git_common_dir
    }
    pub(crate) fn branch(&self) -> String {
        format!("vibe/{}", self.slug)
    }
    pub(crate) fn worktree_path(&self) -> PathBuf {
        self.repo_root.join("worktrees").join(&self.slug)
    }
    pub(crate) fn state_dir(&self, home: &Path) -> PathBuf {
        home.join(".local/state/vibe")
            .join(&self.repo_id)
            .join(&self.slug)
    }

    pub(crate) fn key_path(&self, home: &Path) -> PathBuf {
        self.state_dir(home).join(KEY_FILE)
    }

    /// The first stored key owns this slug; a missing key leaves it unclaimed.
    pub(crate) fn check_stored_key(&self, home: &Path) -> Result<(), String> {
        match fs::read_to_string(self.key_path(home)) {
            Ok(stored) if stored != self.key => Err(format!(
                "slug {} already belongs to key {}",
                self.slug, stored
            )),
            Ok(_) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(format!("read Vibe key: {error}")),
        }
    }
}

pub(crate) fn slugify(key: &str) -> String {
    let mut out = String::new();
    let mut dash = false;
    for ch in key.to_lowercase().chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
            dash = false;
        } else if !dash {
            out.push('-');
            dash = true;
        }
    }
    let trimmed: String = out.trim_matches('-').chars().take(48).collect();
    if trimmed.is_empty() {
        "vibe".to_string()
    } else {
        trimmed
    }
}

// `DefaultHasher` is not stable across Rust releases; changing this hash would
// move existing state directories.
fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::{fnv1a64, slugify, RunTarget};
    use std::path::{Path, PathBuf};

    #[test]
    fn fnv_vectors_remain_stable() {
        assert_eq!(fnv1a64(b""), 0xcbf29ce484222325);
        assert_eq!(fnv1a64(b"a"), 0xaf63dc4c8601ec8c);
    }

    #[test]
    fn repository_ids_separate_common_dirs() {
        let first = RunTarget::from_parts(
            "demo",
            PathBuf::from("/tmp/repo"),
            PathBuf::from("/tmp/one/.git"),
        );
        let second = RunTarget::from_parts(
            "demo",
            PathBuf::from("/tmp/repo"),
            PathBuf::from("/tmp/two/.git"),
        );
        assert_ne!(
            first.state_dir(Path::new("/home/user")),
            second.state_dir(Path::new("/home/user"))
        );
    }

    #[test]
    fn slugify_normalizes_keys() {
        assert_eq!(slugify("PDEV-049 demo/key"), "pdev-049-demo-key");
    }

    #[test]
    fn empty_slug_falls_back() {
        assert_eq!(slugify("---"), "vibe");
    }
}
