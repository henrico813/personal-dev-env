use std::fs;
use std::path::Path;

pub fn read_snapshot_shas(path: &Path) -> Vec<String> {
    let Ok(text) = fs::read_to_string(path) else {
        return Vec::new();
    };
    parse_snapshot_shas(&text)
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
    use super::parse_snapshot_shas;

    #[test]
    fn snapshot_parser_reads_shas() {
        let shas = parse_snapshot_shas("{\"sha\":\"abc\"}\n{\"sha\":\"def\"}\n");

        assert_eq!(shas, vec!["abc", "def"]);
    }

    #[test]
    fn snapshot_parser_skips_bad_lines() {
        let shas = parse_snapshot_shas("not json\n{\"event\":\"skip\"}\n{\"sha\":\"abc\"}\n");

        assert_eq!(shas, vec!["abc"]);
    }
}
