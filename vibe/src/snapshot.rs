use std::fs;
use std::path::Path;

pub fn read_snapshot_shas(path: &Path) -> Result<Vec<String>, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("read snapshot log: {e}"))?;
    Ok(parse_snapshot_shas(&text))
}

fn parse_snapshot_shas(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter_map(|line| {
            line.get("sha")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{parse_snapshot_shas, read_snapshot_shas};
    use tempfile::tempdir;

    #[test]
    fn snapshot_parser_reads_shas() {
        let shas = parse_snapshot_shas(
            r#"{"sha":"abc"}
{"sha":"def"}
"#,
        );

        assert_eq!(shas, vec!["abc", "def"]);
    }

    #[test]
    fn snapshot_parser_skips_bad_lines() {
        let shas = parse_snapshot_shas(
            r#"not json
{"event":"skip"}
{"sha":"abc"}
"#,
        );

        assert_eq!(shas, vec!["abc"]);
    }

    #[test]
    fn snapshot_reader_errors_when_seeded_file_is_missing() {
        let temp = tempdir().expect("tempdir");
        let err = read_snapshot_shas(&temp.path().join("snapshots.jsonl"))
            .expect_err("missing file should error");

        assert!(err.contains("read snapshot log"));
    }
}
