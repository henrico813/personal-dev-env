// Reads and writes the plan files, the refresh lock and the check log in the
// quota folder. Each source has one plan file and one writer, `pde-quota
// refresh` for the asked sources and `pde-quota save-claude-status` for
// Claude Code's status line. Start with readPlanFiles, mergeCheck and
// dueSources.
import { appendFileSync, mkdirSync, readdirSync, readFileSync, renameSync, statSync, unlinkSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";

/** @typedef {"claude" | "copilot" | "codex"} Provider */
/** @typedef {"claude-usage" | "claude-status-line" | "copilot-github" | "copilot-counted" | "codex-app-server"} Source */
/** @typedef {"no-tool" | "no-login" | "no-plan" | "timeout" | "unreachable" | "empty-answer" | "bad-answer"} ErrorCode */
/** @typedef {{name: string, usedPercent: number, resetsAt: number | null}} PlanWindow */
/** @typedef {{dollars: number, since: number}} Spent */
/**
 * @typedef {{version: 1, provider: Provider, source: Source, checkedAt: number | null,
 *   windows: PlanWindow[], spent?: Spent, lastError: {at: number, code: ErrorCode} | null,
 *   note?: "may change"}} PlanFile
 */
/** @typedef {{ok: true, windows: PlanWindow[], spent?: Spent} | {ok: false, code: ErrorCode}} CheckResult */

const MINUTE = 60_000;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;

/** Each source's provider. A plan file's name is its source. */
export const SOURCE_PROVIDERS = {
  "claude-usage": "claude",
  "claude-status-line": "claude",
  "copilot-github": "copilot",
  "copilot-counted": "copilot",
  "codex-app-server": "codex",
};

// The sources `pde-quota refresh` asks, how often, and whether settings turn
// each on. Copilot and Codex join this table when their askers do.
const ASKED_SOURCES = {
  "claude-usage": { everyMs: 5 * MINUTE, isOn: (settings) => settings.claudeUsageCheck === "on" },
};

// A source with no tool, login or plan here is unlikely to gain one soon.
const WAIT_AFTER_ERROR_MS = { "no-tool": DAY, "no-login": DAY, "no-plan": DAY };

// The longest check takes 30 seconds, so a live refresh never holds the lock this long.
const STALE_LOCK_MS = 2 * MINUTE;
const LOG_LIMIT_BYTES = 100 * 1024;
const LOG_KEEP_LINES = 1000;
const STALE_TEMP_MS = HOUR;

/** Returns the quota folder, `$XDG_STATE_HOME/pde/quota` or `~/.local/state/pde/quota`. */
export function quotaDir(env = process.env) {
  return join(env.XDG_STATE_HOME || join(homedir(), ".local/state"), "pde/quota");
}

/**
 * Returns each readable plan file by source. A file that does not parse, or
 * has a version this code does not know, is left out as if it were missing,
 * so an old OpenCode window shows nothing rather than a misread number.
 */
export function readPlanFiles(dir) {
  const files = {};
  for (const source of Object.keys(SOURCE_PROVIDERS)) {
    const file = readJson(join(dir, `${source}.json`));
    if (file?.version === 1 && file.source === source && Array.isArray(file.windows)) files[source] = file;
  }
  return files;
}

/** Writes a plan file through a temporary file and a rename, so a reader never sees half of it. */
export function savePlanFile(dir, file) {
  mkdirSync(dir, { recursive: true });
  const temporary = join(dir, `${file.source}.json.${process.pid}.tmp`);
  writeFileSync(temporary, `${JSON.stringify(file)}\n`);
  renameSync(temporary, join(dir, `${file.source}.json`));
}

/**
 * Returns the plan file after one check. A success replaces the windows and
 * clears `lastError`. A failure keeps the last good windows and their
 * `checkedAt`, so the views show the old number with its age.
 */
export function mergeCheck(old, source, result, now) {
  const base = { version: 1, provider: SOURCE_PROVIDERS[source], source, checkedAt: null, windows: [], lastError: null };
  if (!result.ok) return { ...base, ...old, lastError: { at: now, code: result.code } };
  const windows = result.windows.map((window) => ({ ...window, usedPercent: Math.min(100, Math.max(0, window.usedPercent)) }));
  return { ...base, checkedAt: now, windows, ...(result.spent && { spent: result.spent }) };
}

/**
 * Returns the asked sources that settings turn on and that are due at `now`.
 * A source is due when its latest check, good or failed, is older than its
 * interval, or than a day after `no-tool`, `no-login` or `no-plan`. `force`
 * skips the due test.
 */
export function dueSources(files, settings, now, { force = false } = {}) {
  return Object.entries(ASKED_SOURCES)
    .filter(([source, asked]) => {
      if (!asked.isOn(settings)) return false;
      const file = files[source];
      if (force || !file) return true;
      const checkedAt = file.checkedAt ?? 0;
      const failedLast = file.lastError !== null && file.lastError.at >= checkedAt;
      const waitMs = (failedLast && WAIT_AFTER_ERROR_MS[file.lastError.code]) || asked.everyMs;
      return now - Math.max(checkedAt, file.lastError?.at ?? 0) >= waitMs;
    })
    .map(([source]) => source);
}

/** Returns whether two window lists hold the same names, percents and reset times, in order. */
export function sameWindows(a, b) {
  return (
    a.length === b.length &&
    a.every(
      (window, index) =>
        window.name === b[index].name && window.usedPercent === b[index].usedPercent && window.resetsAt === b[index].resetsAt,
    )
  );
}

/** Deletes the plan file of each asked source that settings turn off, so no view shows a number the user stopped checking. */
export function deleteDisabledFiles(dir, settings) {
  for (const [source, asked] of Object.entries(ASKED_SOURCES)) {
    if (!asked.isOn(settings)) removeFile(join(dir, `${source}.json`));
  }
}

/** Deletes temporary files older than an hour, which a crash between savePlanFile's write and rename leaves behind. */
export function deleteStaleTempFiles(dir, now) {
  for (const name of listFiles(dir)) {
    if (!name.endsWith(".tmp")) continue;
    const path = join(dir, name);
    const modifiedAt = statSync(path, { throwIfNoEntry: false })?.mtimeMs ?? now;
    if (now - modifiedAt > STALE_TEMP_MS) removeFile(path);
  }
}

/**
 * Creates `refresh.lock` and returns it, or returns null while a live
 * refresh holds it. A lock whose process is gone, or that is older than 2
 * minutes, is stale and replaced.
 */
export function takeLock(dir, now) {
  mkdirSync(dir, { recursive: true });
  const path = join(dir, "refresh.lock");
  if (createLock(path, now)) return { path, pid: process.pid };
  const held = readJson(path);
  // A lock read before its creator wrote it has no process ID or time yet.
  // It counts as live, and its file time stands in, until it is 2 minutes old.
  const heldAt = typeof held?.at === "number" ? held.at : (statSync(path, { throwIfNoEntry: false })?.mtimeMs ?? 0);
  const live = held?.pid === undefined || isProcessAlive(held.pid);
  if (live && now - heldAt < STALE_LOCK_MS) return null;
  removeFile(path);
  // Two refreshes that find the same stale lock can, rarely, both run, which costs one extra check.
  return createLock(path, now) ? { path, pid: process.pid } : null;
}

/** Deletes the lock when this process still holds it. */
export function releaseLock(lock) {
  if (readJson(lock.path)?.pid === lock.pid) removeFile(lock.path);
}

/**
 * Appends one line such as `2026-11-13T09:00:00Z claude-usage ok` to
 * `checks.log`. Past 100 KB the log keeps its newest 1,000 lines.
 */
export function logCheck(dir, source, result, now) {
  const path = join(dir, "checks.log");
  const time = new Date(now).toISOString().replace(/\.\d{3}Z$/, "Z");
  appendFileSync(path, `${time} ${source} ${result.ok ? "ok" : result.code}\n`);
  if (statSync(path).size <= LOG_LIMIT_BYTES) return;
  const lines = readFileSync(path, "utf8").trimEnd().split("\n").slice(-LOG_KEEP_LINES);
  const temporary = `${path}.${process.pid}.tmp`;
  writeFileSync(temporary, `${lines.join("\n")}\n`);
  renameSync(temporary, path);
}

/** Returns whether this call created the lock file, which fails when the file exists. */
function createLock(path, now) {
  try {
    writeFileSync(path, JSON.stringify({ pid: process.pid, at: now }), { flag: "wx" });
    return true;
  } catch (error) {
    if (error.code === "EEXIST") return false;
    throw error;
  }
}

/** Returns whether a process with this ID runs. EPERM means it runs as another user. */
function isProcessAlive(pid) {
  if (typeof pid !== "number") return false;
  try {
    process.kill(pid, 0);
    return true;
  } catch (error) {
    return error.code === "EPERM";
  }
}

/** Returns parsed JSON from a file, or undefined when it is missing or not JSON. */
function readJson(path) {
  try {
    return JSON.parse(readFileSync(path, "utf8"));
  } catch {
    return undefined;
  }
}

/** Returns the names in a folder, or none when it does not exist. */
function listFiles(dir) {
  try {
    return readdirSync(dir);
  } catch {
    return [];
  }
}

/** Deletes a file that may already be gone. */
function removeFile(path) {
  try {
    unlinkSync(path);
  } catch (error) {
    if (error.code !== "ENOENT") throw error;
  }
}
