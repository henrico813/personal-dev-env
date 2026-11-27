//! Reads the numbers the board's gauges show and the report files for
//! its feed. A read that fails gives `None` or nothing, since a gauge
//! is only a hint. Read `read_gauges` and `read_feed` first.

use std::fs;
use std::path::Path;
use std::process::Command;

use jiff::Timestamp;
use jiff::tz::TimeZone;
use serde::{Deserialize, Serialize};

use crate::shared::run_folder::RunFolder;

/// One gauge's use against its limit, such as 116 of 128 watch slots.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Usage {
    pub used: u64,
    pub limit: u64,
}

/// What the gauges show, `None` where a read failed or the system has
/// no such number, as on macOS for watch slots.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Gauges {
    /// inotify instances the user's processes hold, against the
    /// kernel's `max_user_instances`.
    pub watch: Option<Usage>,
    /// Kilobytes used on the home folder's disk, against its size.
    pub disk: Option<Usage>,
}

/// One report file in the feed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct FeedLine {
    /// When the file last changed, such as `11:41`.
    pub at: String,
    /// The same time in full, which SINCE YOU LEFT compares with the
    /// last visit.
    pub stamp: Timestamp,
    /// True for a `report-NAME.md` the hub has not read yet.
    pub unread: bool,
    pub name: String,
}

/// Reads both gauges. The disk is the one that holds `home`.
pub(crate) fn read_gauges(home: &Path) -> Gauges {
    Gauges {
        watch: read_watch_slots(),
        disk: read_disk(home),
    }
}

/// Returns every report file in `runs`, newest first, both unread
/// `report-NAME.md` and read `report-NAME.HHMM.read`, with times in
/// `time_zone`.
pub(crate) fn read_feed(runs: &[&RunFolder], time_zone: &TimeZone) -> Vec<FeedLine> {
    let mut found = Vec::new();
    for run in runs {
        let Ok(entries) = fs::read_dir(run.path()) else {
            continue;
        };
        for entry in entries.flatten() {
            let file_name = entry.file_name().to_string_lossy().into_owned();
            let Some((name, unread)) = parse_report_name(&file_name) else {
                continue;
            };
            let modified = entry.metadata().and_then(|metadata| metadata.modified());
            let Some(stamp) = modified
                .ok()
                .and_then(|time| Timestamp::try_from(time).ok())
            else {
                continue;
            };
            found.push((stamp, name, unread));
        }
    }
    found.sort_by_key(|(stamp, _, _)| std::cmp::Reverse(*stamp));
    found
        .into_iter()
        .map(|(stamp, name, unread)| FeedLine {
            at: stamp
                .to_zoned(time_zone.clone())
                .strftime("%H:%M")
                .to_string(),
            stamp,
            unread,
            name,
        })
        .collect()
}

/// Counts the inotify instances open in the processes this user may
/// read, against the kernel's limit per user. New OpenCode sessions
/// crashed at 122 of 128.
fn read_watch_slots() -> Option<Usage> {
    let limit = fs::read_to_string("/proc/sys/fs/inotify/max_user_instances").ok()?;
    let mut used: usize = 0;
    for process in fs::read_dir("/proc").ok()?.flatten() {
        // `self` and `thread-self` repeat this process.
        let is_pid = process
            .file_name()
            .to_str()
            .is_some_and(|name| name.bytes().all(|byte| byte.is_ascii_digit()));
        if !is_pid {
            continue;
        }
        // Another user's fd folder cannot be read, and its instances
        // count against that user's limit, not this one.
        let Ok(fds) = fs::read_dir(process.path().join("fd")) else {
            continue;
        };
        used += fds
            .flatten()
            .filter(|fd| {
                fs::read_link(fd.path())
                    .is_ok_and(|target| target.as_os_str() == "anon_inode:inotify")
            })
            .count();
    }
    Some(Usage {
        used: u64::try_from(used).ok()?,
        limit: limit.trim().parse().ok()?,
    })
}

/// Runs `df -Pk` on `home` and reads its disk's use.
fn read_disk(home: &Path) -> Option<Usage> {
    let output = Command::new("df").arg("-Pk").arg(home).output().ok()?;
    parse_df(&String::from_utf8_lossy(&output.stdout))
}

/// Reads the used and total kilobytes from `df -Pk` output, whose
/// second line is `FILESYSTEM TOTAL USED AVAILABLE CAPACITY MOUNT`.
/// Used counts the root-only reserve, so free is what the user can
/// write.
fn parse_df(text: &str) -> Option<Usage> {
    let fields: Vec<&str> = text.lines().nth(1)?.split_whitespace().collect();
    let total: u64 = fields.get(1)?.parse().ok()?;
    let available: u64 = fields.get(3)?.parse().ok()?;
    Some(Usage {
        used: total.saturating_sub(available),
        limit: total,
    })
}

/// Returns the session name in a report file name, and true when the
/// hub has not read it, such as `docs-fix` and true for
/// `report-docs-fix.md`.
fn parse_report_name(file_name: &str) -> Option<(String, bool)> {
    let rest = file_name.strip_prefix("report-")?;
    // Names hold no dot, so the name ends at the first one.
    let (name, suffix) = rest.split_once('.')?;
    match suffix {
        "md" => Some((name.to_owned(), true)),
        _ if suffix.ends_with(".read") => Some((name.to_owned(), false)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::fs::File;
    use std::time::{Duration, SystemTime};

    /// The disk gauge reads columns of `df -Pk` by place. Reading the
    /// used column as free would show a full disk as empty.
    #[test]
    fn parse_df_reads_total_and_available() {
        let text = "\
Filesystem     1024-blocks      Used Available Capacity Mounted on
/dev/nvme0n1p2   100000000  70000000  27000000      73% /";

        let usage = parse_df(text);

        assert_eq!(
            usage,
            Some(Usage {
                used: 73_000_000,
                limit: 100_000_000,
            })
        );
    }

    /// The feed shows what workers reported, and SINCE YOU LEFT reads
    /// it for what came since the last visit. It must come newest first, and a
    /// file that is no report, such as `notes.md`, must stay out, or the
    /// user reads stale news.
    #[test]
    fn read_feed_lists_newest_reports_first() {
        let path = std::env::temp_dir().join(format!("hub-read-feed-{}", std::process::id()));
        fs::create_dir_all(&path).unwrap();
        let start = SystemTime::UNIX_EPOCH + Duration::from_secs(1_800_000_000);
        let files = [
            "report-old.1010.read",
            "notes.md",
            "report-a.1100.read",
            "report-b.md",
            "report-c.1120.read",
            "report-d.md",
            "report-e.1140.read",
            "report-f.md",
        ];
        for (minute, file) in (0u64..).zip(files) {
            let made = File::create(path.join(file)).unwrap();
            made.set_modified(start + Duration::from_secs(60 * minute))
                .unwrap();
        }
        let run = RunFolder::open(&path).unwrap();

        let feed = read_feed(&[&run], &TimeZone::UTC);

        fs::remove_dir_all(&path).unwrap();
        let shown: Vec<(&str, bool)> = feed
            .iter()
            .map(|line| (line.name.as_str(), line.unread))
            .collect();
        assert_eq!(
            shown,
            [
                ("f", true),
                ("e", false),
                ("d", true),
                ("c", false),
                ("b", true),
                ("a", false),
                ("old", false),
            ]
        );
        assert_eq!(feed[0].at, "08:07");
    }
}
