use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

pub const DEFAULT_TASK_FILENAME: &str = "task.md";
pub const DEFAULT_TASK_TEMPLATE: &str = concat!(
    "# Task\n\n",
    "## Summary\n\n",
    "## Explicit Files\n\n",
    "## Search Areas\n\n",
    "## Query\n\n",
    "## Terms\n",
);

pub fn run(output_dir: &Path) -> io::Result<()> {
    create_task_file(output_dir).map(|_| ())
}

pub fn create_task_file(output_dir: &Path) -> io::Result<PathBuf> {
    let task_path = output_dir.join(DEFAULT_TASK_FILENAME);

    if let Some(parent) = task_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&task_path)?;
    file.write_all(DEFAULT_TASK_TEMPLATE.as_bytes())?;

    Ok(task_path)
}

#[cfg(test)]
mod tests {
    use super::{create_task_file, DEFAULT_TASK_TEMPLATE};
    use std::fs;
    use std::io;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        std::env::temp_dir().join(format!("surveil-taskfile-{name}-{stamp}"))
    }

    #[test]
    fn creates_task_file_with_default_template() {
        let root = temp_root("template");
        fs::create_dir_all(&root).expect("create root");

        let task_path = create_task_file(&root).expect("create task file");

        assert_eq!(task_path, root.join("task.md"));
        assert_eq!(
            fs::read_to_string(&task_path).expect("read task file"),
            DEFAULT_TASK_TEMPLATE
        );

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn creates_missing_parent_directories() {
        let root = temp_root("parents");
        let output_dir = root.join("nested/output");

        let task_path = create_task_file(&output_dir).expect("create task file");

        assert!(task_path.exists());
        assert_eq!(task_path, output_dir.join("task.md"));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn fails_when_task_file_already_exists() {
        let root = temp_root("exists");
        fs::create_dir_all(&root).expect("create root");
        fs::write(root.join("task.md"), "existing").expect("seed task file");

        let err = create_task_file(&root).expect_err("expected create failure");

        assert_eq!(err.kind(), io::ErrorKind::AlreadyExists);

        let _ = fs::remove_dir_all(&root);
    }
}
