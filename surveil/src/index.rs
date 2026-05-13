use crate::source;
use rusqlite::{params, Connection, OptionalExtension};
use std::error::Error;
use std::fs;
use std::path::Path;
use std::time::UNIX_EPOCH;

pub const INDEX_PATH: &str = ".surveil/index.sqlite";

pub struct CachedFile {
    pub text: String,
    pub mtime_ns: i64,
    pub size_bytes: i64,
}

pub fn run(repo_root: &Path) -> Result<(), Box<dyn Error>> {
    let db_path = repo_root.join(INDEX_PATH);
    if let Some(parent) = db_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut conn = Connection::open(db_path)?;
    init_schema(&conn)?;
    rebuild_index(repo_root, &mut conn)?;
    Ok(())
}

pub fn open(repo_root: &Path) -> Result<Option<Connection>, Box<dyn Error>> {
    let db_path = repo_root.join(INDEX_PATH);
    if !db_path.is_file() {
        return Ok(None);
    }
    Ok(Some(Connection::open(db_path)?))
}

pub fn load_text(
    conn: &Connection,
    repo_root: &Path,
    path: &Path,
) -> Result<Option<CachedFile>, Box<dyn Error>> {
    let relative_path = source::display_path(repo_root, path);
    let mut query = conn.prepare("SELECT text, mtime_ns, size_bytes FROM files WHERE path = ?1")?;
    let cached = query
        .query_row([relative_path], |row| {
            Ok(CachedFile {
                text: row.get(0)?,
                mtime_ns: row.get(1)?,
                size_bytes: row.get(2)?,
            })
        })
        .optional()?;
    Ok(cached)
}

pub fn is_fresh(path: &Path, cached: &CachedFile) -> Result<bool, Box<dyn Error>> {
    let metadata = fs::metadata(path)?;
    let modified = metadata.modified()?.duration_since(UNIX_EPOCH)?.as_nanos() as i64;
    Ok(cached.mtime_ns == modified && cached.size_bytes == metadata.len() as i64)
}

fn init_schema(conn: &Connection) -> Result<(), Box<dyn Error>> {
    conn.execute_batch(
        "PRAGMA user_version = 1;
         CREATE TABLE IF NOT EXISTS files (
             path TEXT PRIMARY KEY,
             mtime_ns INTEGER NOT NULL,
             size_bytes INTEGER NOT NULL,
             text TEXT NOT NULL
         );",
    )?;
    Ok(())
}

fn rebuild_index(repo_root: &Path, conn: &mut Connection) -> Result<(), Box<dyn Error>> {
    let tx = conn.transaction()?;
    tx.execute("DELETE FROM files", [])?;

    let mut skipped_paths = Vec::new();
    let search_areas = [".".to_string()];
    let candidates = source::collect_candidate_files(repo_root, &search_areas, &[], &mut skipped_paths)?;
    for candidate in candidates {
        let path = candidate.path();
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(_) => continue,
        };
        let metadata = fs::metadata(path)?;
        let modified = metadata.modified()?.duration_since(UNIX_EPOCH)?.as_nanos() as i64;
        tx.execute(
            "INSERT INTO files(path, mtime_ns, size_bytes, text) VALUES (?1, ?2, ?3, ?4)",
            params![
                candidate.display_path(),
                modified,
                metadata.len() as i64,
                text,
            ],
        )?;
    }

    tx.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{is_fresh, load_text, open, run, INDEX_PATH};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_repo(name: &str) -> PathBuf {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("surveil-index-{name}-{stamp}"));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn builds_repo_local_text_cache_and_loads_text() {
        let repo = temp_repo("builds");
        fs::create_dir_all(repo.join("notes")).unwrap();
        fs::write(repo.join("notes/design.md"), "attach index here\n").unwrap();

        run(&repo).unwrap();

        assert!(repo.join(INDEX_PATH).is_file());
        let conn = open(&repo).unwrap().unwrap();
        let cached = load_text(&conn, &repo, &repo.join("notes/design.md")).unwrap().unwrap();
        assert_eq!(cached.text, "attach index here\n");
        assert!(is_fresh(&repo.join("notes/design.md"), &cached).unwrap());

        let _ = fs::remove_dir_all(repo);
    }
}
