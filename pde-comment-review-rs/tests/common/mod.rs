use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct TempFolder(PathBuf);

impl TempFolder {
    pub fn new() -> Self {
        let output = Command::new("mktemp").arg("-d").output().unwrap();
        assert!(output.status.success());
        let path = String::from_utf8(output.stdout).unwrap();
        Self(PathBuf::from(path.trim_end_matches('\n')))
    }

    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempFolder {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
