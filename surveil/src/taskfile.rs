use std::ffi::OsStr;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const DEFAULT_TASK_FILENAME: &str = "task.md";
const MANAGED_ROOT_MARKER: &str = ".surveil-managed";
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

pub fn create_managed_task(state_root: &Path, task: &str) -> io::Result<PathBuf> {
    validate_task_name(task)?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| io::Error::new(io::ErrorKind::Other, error))?
        .as_nanos();
    let root = create_managed_root_at(state_root, stamp, std::process::id())?;
    let initialized = (|| {
        fs::write(root.join(MANAGED_ROOT_MARKER), b"surveil managed root\n")?;
        create_managed_task_dir(&root, task)
    })();
    if let Err(error) = initialized {
        let _ = fs::remove_dir_all(&root);
        return Err(error);
    }
    Ok(root)
}

pub fn append_managed_task(root: &Path, task: &str) -> io::Result<()> {
    validate_task_name(task)?;
    validate_existing_managed_root(root)?;
    create_managed_task_dir(root, task)
}

pub fn state_root() -> io::Result<PathBuf> {
    let xdg_state_home = std::env::var_os("XDG_STATE_HOME");
    let home = std::env::var_os("HOME");
    state_root_from(xdg_state_home.as_deref(), home.as_deref())
}

fn state_root_from(xdg_state_home: Option<&OsStr>, home: Option<&OsStr>) -> io::Result<PathBuf> {
    if let Some(path) = xdg_state_home.map(Path::new).filter(|path| path.is_absolute()) {
        return Ok(path.join("surveil").join("runs"));
    }

    let home = Path::new(
        home.ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "HOME is not set"))?,
    );
    if !home.is_absolute() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "HOME must be an absolute path",
        ));
    }

    Ok(home.join(".local").join("state").join("surveil").join("runs"))
}

fn create_managed_root_at(state_root: &Path, stamp: u128, process_id: u32) -> io::Result<PathBuf> {
    fs::create_dir_all(state_root)?;
    for attempt in 0..10 {
        let root = state_root.join(format!("{stamp}-{process_id}-{attempt}"));
        match fs::create_dir(&root) {
            Ok(()) => return Ok(root),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "could not allocate a unique Surveil run directory",
    ))
}

fn create_managed_task_dir(root: &Path, task: &str) -> io::Result<()> {
    let task_dir = root.join(task);
    fs::create_dir(&task_dir)?;
    if let Err(error) = create_task_file(&task_dir) {
        let _ = fs::remove_dir_all(&task_dir);
        return Err(error);
    }
    Ok(())
}

fn validate_existing_managed_root(root: &Path) -> io::Result<()> {
    if !root.is_absolute() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "managed task root must be absolute",
        ));
    }

    let metadata = fs::symlink_metadata(root)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "managed task root must be a non-symlink directory",
        ));
    }

    let marker = fs::symlink_metadata(root.join(MANAGED_ROOT_MARKER)).map_err(|error| {
        if error.kind() == io::ErrorKind::NotFound {
            io::Error::new(io::ErrorKind::InvalidInput, "not a Surveil-managed root")
        } else {
            error
        }
    })?;
    if !marker.file_type().is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid Surveil-managed root marker",
        ));
    }
    Ok(())
}

pub(crate) fn validate_task_name(task: &str) -> io::Result<()> {
    let mut components = Path::new(task).components();
    let valid = !task.trim().is_empty()
        && !task.chars().any(char::is_control)
        && !task.contains('/')
        && !task.contains('\\')
        && matches!(components.next(), Some(Component::Normal(_)))
        && components.next().is_none();
    if !valid {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "task name must be one nonempty path component without control characters",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        append_managed_task, create_managed_root_at, create_managed_task, create_task_file,
        state_root_from, validate_task_name, DEFAULT_TASK_TEMPLATE,
    };
    use std::ffi::OsStr;
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

    #[test]
    fn retries_managed_root_collisions() {
        let state_root = temp_root("collision");
        fs::create_dir_all(state_root.join("42-7-0")).expect("seed collision");
        let root = create_managed_root_at(&state_root, 42, 7).expect("create root");
        assert_eq!(root, state_root.join("42-7-1"));
        let _ = fs::remove_dir_all(&state_root);
    }

    #[test]
    fn creates_and_appends_managed_tasks() {
        let state_root = temp_root("managed");
        let root = create_managed_task(&state_root, "architecture").expect("create task");
        append_managed_task(&root, "tests verification").expect("append task");
        assert!(root.join(".surveil-managed").is_file());
        assert!(root.join("architecture/task.md").exists());
        assert!(root.join("tests verification/task.md").exists());
        assert_eq!(
            append_managed_task(&root, "architecture")
                .expect_err("existing task")
                .kind(),
            io::ErrorKind::AlreadyExists,
        );
        let _ = fs::remove_dir_all(&state_root);
    }

    #[test]
    fn cleans_failed_initial_allocation() {
        let state_root = temp_root("cleanup");
        let error = create_managed_task(&state_root, ".surveil-managed")
            .expect_err("reject marker collision");
        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(fs::read_dir(&state_root).expect("read roots").count(), 0);
        let _ = fs::remove_dir_all(&state_root);
    }

    #[test]
    fn rejects_invalid_append_roots() {
        let state_root = temp_root("append");
        let foreign = state_root.join("foreign");
        let file = state_root.join("file");
        fs::create_dir_all(&foreign).expect("create foreign root");
        fs::write(&file, "file").expect("create file");
        for root in [PathBuf::from("relative"), foreign, file] {
            assert!(append_managed_task(&root, "architecture").is_err());
        }
        let _ = fs::remove_dir_all(&state_root);
    }

    #[cfg(unix)]
    #[test]
    fn rejects_managed_root_symlink() {
        use std::os::unix::fs::symlink;
        let state_root = temp_root("symlink");
        let root = create_managed_task(&state_root, "architecture").expect("create task");
        let link = state_root.join("linked-root");
        symlink(&root, &link).expect("create symlink");
        assert!(append_managed_task(&link, "tests").is_err());
        let _ = fs::remove_dir_all(&state_root);
    }

    #[test]
    fn validates_task_names() {
        for task in ["architecture", "tests verification", "résumé"] {
            validate_task_name(task).expect("valid task name");
        }
        for task in [
            "", "   ", ".", "..", "nested/task", "nested\\task", "/absolute", "line\nbreak",
        ] {
            assert!(validate_task_name(task).is_err(), "task should fail: {task:?}");
        }
    }

    #[test]
    fn resolves_absolute_state_roots() {
        let cases = [
            (
                Some(OsStr::new("/state")),
                Some(OsStr::new("/home/user")),
                Some(PathBuf::from("/state/surveil/runs")),
            ),
            (
                Some(OsStr::new("relative")),
                Some(OsStr::new("/home/user")),
                Some(PathBuf::from("/home/user/.local/state/surveil/runs")),
            ),
            (None, Some(OsStr::new("relative")), None),
            (None, None, None),
        ];
        for (xdg, home, expected) in cases {
            match expected {
                Some(expected) => assert_eq!(state_root_from(xdg, home).expect("state root"), expected),
                None => assert!(state_root_from(xdg, home).is_err()),
            }
        }
    }

    #[cfg(unix)]
    #[test]
    fn preserves_non_utf8_state_root() {
        use std::os::unix::ffi::OsStringExt;
        let xdg = std::ffi::OsString::from_vec(b"/state/\xff".to_vec());
        let expected = PathBuf::from(xdg.clone()).join("surveil").join("runs");
        assert_eq!(state_root_from(Some(&xdg), None).expect("state root"), expected);
    }
}
