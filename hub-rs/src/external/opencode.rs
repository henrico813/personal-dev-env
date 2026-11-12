//! Reads sessions from this machine's OpenCode database, read-only.
//! Read `count_trusted_context` and `judge_activity` for the two rules
//! this file holds; the rest are queries.

use std::fmt;
use std::path::PathBuf;

use jiff::{Timestamp, Zoned};
use rusqlite::{Connection, OpenFlags, OptionalExtension};
use serde_json::Value;

/// A session with no tool running that changed nothing for this long
/// has stopped.
const QUIET_MS: i64 = 2 * 60 * 1000;

/// The OpenCode database, opened read-only for each read.
pub struct OpenCodeDb {
    path: PathBuf,
}

/// What `hub alive` shows about a session.
pub(crate) struct Activity {
    pub message_count: usize,
    pub newest: Timestamp,
    /// The newest tool the session called, or `None` before any.
    pub last_tool: Option<String>,
    pub word: ActivityWord,
}

/// Whether a session works, finished its turn or stopped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ActivityWord {
    Working,
    Finished,
    Stopped,
}

impl ActivityWord {
    /// Returns the word `hub alive` ends its line with.
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            ActivityWord::Working => "working",
            ActivityWord::Finished => "finished",
            ActivityWord::Stopped => "stopped",
        }
    }
}

/// One model step's token counts.
struct TokenStep {
    input: u64,
    cache_read: u64,
    cache_write: u64,
}

/// One message row, as `judge_activity` and `find_last_tool` read it.
struct MessageRow {
    kind: String,
    updated_ms: i64,
    data: Value,
}

/// A database read that failed.
#[derive(Debug)]
pub enum OpenCodeError {
    /// Locked, or its tables changed.
    Database(rusqlite::Error),
    /// A row whose JSON does not parse.
    BadRow(String),
    /// `hub alive` asked for a session the database lacks.
    NoSession(String),
}

impl fmt::Display for OpenCodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OpenCodeError::Database(error) => {
                write!(formatter, "cannot read the OpenCode database: {error}")
            }
            OpenCodeError::BadRow(message) => formatter.write_str(message),
            OpenCodeError::NoSession(session) => {
                write!(formatter, "no OpenCode session {session} on this machine")
            }
        }
    }
}

/// Returns the `rusqlite` error behind a `Database` failure, and lets
/// `HubError::source` return this one.
impl std::error::Error for OpenCodeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            OpenCodeError::Database(error) => Some(error),
            OpenCodeError::BadRow(_) | OpenCodeError::NoSession(_) => None,
        }
    }
}

impl OpenCodeDb {
    /// Reads the database at `path`, which `main` gives as
    /// `~/.local/share/opencode/opencode.db`.
    pub fn new(path: PathBuf) -> OpenCodeDb {
        OpenCodeDb { path }
    }

    /// Returns the context of the newest trusted step, or `None` when
    /// there is none. A missing database file reads as no rows.
    pub(crate) fn read_context_tokens(&self, session: &str) -> Result<Option<u64>, OpenCodeError> {
        let Some(connection) = self.open()? else {
            return Ok(None);
        };
        let mut statement = connection
            .prepare(
                "select coalesce(json_extract(data, '$.tokens.input'), 0), \
                 coalesce(json_extract(data, '$.tokens.cache.read'), 0), \
                 coalesce(json_extract(data, '$.tokens.cache.write'), 0) \
                 from session_message \
                 where session_id = ?1 and type = 'assistant' order by seq",
            )
            .map_err(OpenCodeError::Database)?;
        let raw_steps = statement
            .query_map([session], |row| {
                // SQLite stores integers signed; a negative count reads as zero.
                let count = |index| row.get::<_, i64>(index).map(|value| value.max(0) as u64);
                Ok(TokenStep {
                    input: count(0)?,
                    cache_read: count(1)?,
                    cache_write: count(2)?,
                })
            })
            .map_err(OpenCodeError::Database)?;
        let mut steps = Vec::new();
        for raw_step in raw_steps {
            steps.push(raw_step.map_err(OpenCodeError::Database)?);
        }
        Ok(count_trusted_context(steps))
    }

    /// Returns the first reply's reasoning effort, `default` when it
    /// names none, or `None` before any reply. A later switch of effort
    /// does not count, since the first reply shows how the session
    /// started.
    pub(crate) fn read_first_reply_effort(
        &self,
        session: &str,
    ) -> Result<Option<String>, OpenCodeError> {
        let Some(connection) = self.open()? else {
            return Ok(None);
        };
        let data: Option<String> = connection
            .query_row(
                "select data from session_message \
                 where session_id = ?1 and type = 'assistant' order by seq limit 1",
                [session],
                |row| row.get(0),
            )
            .optional()
            .map_err(OpenCodeError::Database)?;
        let Some(data) = data else {
            return Ok(None);
        };
        let reply = parse_row_data(&data)?;
        let effort = reply
            .pointer("/model/variant")
            .and_then(Value::as_str)
            .unwrap_or("default");
        Ok(Some(effort.to_owned()))
    }

    /// Returns what `hub alive` shows, or `None` for an unknown session.
    pub(crate) fn read_activity(
        &self,
        session: &str,
        now: &Zoned,
    ) -> Result<Option<Activity>, OpenCodeError> {
        let Some(connection) = self.open()? else {
            return Ok(None);
        };
        let mut statement = connection
            .prepare(
                "select type, time_updated, data from session_message \
                 where session_id = ?1 order by seq desc",
            )
            .map_err(OpenCodeError::Database)?;
        let raw_rows = statement
            .query_map([session], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .map_err(OpenCodeError::Database)?;
        let mut rows = Vec::new();
        for raw_row in raw_rows {
            let (kind, updated_ms, data) = raw_row.map_err(OpenCodeError::Database)?;
            rows.push(MessageRow {
                kind,
                updated_ms,
                data: parse_row_data(&data)?,
            });
        }
        let Some(newest) = rows.first() else {
            return Ok(None);
        };
        let newest_at = Timestamp::from_millisecond(newest.updated_ms).map_err(|error| {
            OpenCodeError::BadRow(format!("an OpenCode message has a bad time: {error}"))
        })?;
        Ok(Some(Activity {
            message_count: rows.len(),
            newest: newest_at,
            last_tool: find_last_tool(&rows),
            word: judge_activity(newest, now.timestamp().as_millisecond()),
        }))
    }

    /// Returns the folder a session works in, from the `session` table,
    /// or `None` when the database lacks the session.
    pub fn find_session(&self, session: &str) -> Result<Option<PathBuf>, OpenCodeError> {
        let Some(connection) = self.open()? else {
            return Ok(None);
        };
        let directory: Option<String> = connection
            .query_row(
                "select directory from session where id = ?1",
                [session],
                |row| row.get(0),
            )
            .optional()
            .map_err(OpenCodeError::Database)?;
        Ok(directory.map(PathBuf::from))
    }

    /// Opens the database read-only, or returns `None` when this machine
    /// has none.
    fn open(&self) -> Result<Option<Connection>, OpenCodeError> {
        if !self.path.exists() {
            return Ok(None);
        }
        Connection::open_with_flags(&self.path, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map(Some)
            .map_err(OpenCodeError::Database)
    }
}

/// Reads one row's `data` column, which OpenCode stores as JSON.
fn parse_row_data(data: &str) -> Result<Value, OpenCodeError> {
    serde_json::from_str(data)
        .map_err(|error| OpenCodeError::BadRow(format!("an OpenCode message is not JSON: {error}")))
}

/// Returns the newest step's context that was not zero and did not
/// come right after a zero step. The claude-code provider logs some
/// steps as zero and adds them to the next one.
fn count_trusted_context(steps: impl IntoIterator<Item = TokenStep>) -> Option<u64> {
    let mut context = None;
    let mut after_zero = false;
    for step in steps {
        let size = step.input + step.cache_read + step.cache_write;
        if size > 0 && !after_zero {
            context = Some(size);
        }
        after_zero = size == 0;
    }
    context
}

/// Returns working while a tool part runs, or while the newest row
/// changed in the last 2 minutes. Returns finished after a succeeded
/// `idle` row. Returns stopped otherwise. An unfinished step alone is
/// not working, because a killed session keeps one forever. A long
/// quiet time alone is not stopped, because a parent waiting on a
/// subagent changes nothing for minutes while its tool part runs.
fn judge_activity(newest: &MessageRow, now_ms: i64) -> ActivityWord {
    if newest.kind == "idle" {
        return if newest.data["outcome"] == "succeeded" {
            ActivityWord::Finished
        } else {
            ActivityWord::Stopped
        };
    }
    let tool_runs = newest.kind == "assistant"
        && newest.data["content"].as_array().is_some_and(|parts| {
            parts
                .iter()
                .any(|part| part["state"]["status"] == "running")
        });
    if tool_runs || now_ms - newest.updated_ms < QUIET_MS {
        ActivityWord::Working
    } else {
        ActivityWord::Stopped
    }
}

/// Returns the name of the last tool part in the newest assistant row
/// that has one, or `-` for a tool part without a name.
fn find_last_tool(rows: &[MessageRow]) -> Option<String> {
    rows.iter()
        .filter(|row| row.kind == "assistant")
        .find_map(|row| {
            let parts = row.data["content"].as_array()?;
            parts.iter().rev().find(|part| part["type"] == "tool")
        })
        .map(|tool| tool["name"].as_str().unwrap_or("-").to_owned())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    /// The hub's context ends every `hub next` line and decides when it
    /// hands off. A step that follows a zero step holds two steps' tokens,
    /// so counting it shows about twice the real context and makes the
    /// hub hand off far too early.
    #[test]
    fn count_trusted_context_skips_zero_and_next() {
        let step = |input, cache_read, cache_write| TokenStep {
            input,
            cache_read,
            cache_write,
        };
        let cases = [
            ("no-steps", vec![], None),
            (
                "newest-counts",
                vec![step(10, 100_000, 7_000), step(20, 110_000, 0)],
                Some(110_020),
            ),
            (
                "zero-and-next-skipped",
                vec![step(10, 100_000, 7_000), step(0, 0, 0), step(0, 217_700, 0)],
                Some(107_010),
            ),
        ];

        for (case_name, steps, expected) in cases {
            assert_eq!(count_trusted_context(steps), expected, "{case_name}");
        }
    }

    /// The hub runs `hub alive` before it tells the user a session
    /// died. A session killed mid-step keeps an unfinished step forever.
    /// A parent waiting on a subagent changes nothing for minutes. So
    /// neither the unfinished step nor the quiet time decides the
    /// answer by itself.
    #[test]
    fn judge_activity_follows_newest_message() {
        let now_ms = 1_000_000_000;
        let ten_minutes_ago = now_ms - 10 * 60 * 1000;
        let cases = [
            (
                "turn-succeeded",
                "idle",
                ten_minutes_ago,
                json!({"outcome": "succeeded"}),
                ActivityWord::Finished,
            ),
            (
                "turn-interrupted",
                "idle",
                now_ms,
                json!({"outcome": "interrupted"}),
                ActivityWord::Stopped,
            ),
            (
                "quiet-tool-running",
                "assistant",
                ten_minutes_ago,
                json!({"content": [{"type": "tool", "name": "subagent", "state": {"status": "running"}}]}),
                ActivityWord::Working,
            ),
            (
                "recent-message",
                "user",
                now_ms,
                json!({}),
                ActivityWord::Working,
            ),
            (
                "quiet-unfinished-step",
                "assistant",
                ten_minutes_ago,
                json!({"content": [{"type": "text"}]}),
                ActivityWord::Stopped,
            ),
        ];

        for (case_name, kind, updated_ms, data, expected) in cases {
            let newest = MessageRow {
                kind: kind.to_owned(),
                updated_ms,
                data,
            };

            assert_eq!(judge_activity(&newest, now_ms), expected, "{case_name}");
        }
    }
}
