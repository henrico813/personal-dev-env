// Reads Claude plan usage from Claude Code's usage request, which `pde-quota
// refresh` asks, or from the JSON Claude Code gives its status line. Both give
// the same window names, so the newer answer can replace the other.
import { spawn } from "node:child_process";
import { mkdirSync } from "node:fs";
import { join } from "node:path";
import { quotaDir } from "./store.js";

// Claude Code reads only this folder's project settings, so the user's
// "no non-essential traffic" switch, which empties the answer, does not apply.
// Telemetry, error reports and auto-update stay off.
const CLAUDE_ARGS = [
  "-p",
  "--input-format",
  "stream-json",
  "--output-format",
  "stream-json",
  "--verbose",
  "--setting-sources",
  "project",
  "--settings",
  JSON.stringify({ env: { DISABLE_TELEMETRY: "1", DISABLE_ERROR_REPORTING: "1", DISABLE_AUTOUPDATER: "1" } }),
  "--no-session-persistence",
  "--strict-mcp-config",
];
const REQUEST_ID = "pde-quota-usage";
// Asks for the plan's usage rows only, without a model call or a scan of past transcripts.
const USAGE_REQUEST = { type: "control_request", request_id: REQUEST_ID, request: { subtype: "get_usage", skip_behaviors: true } };
// A stopped Claude Code that does not exit by then is killed.
const KILL_WAIT_MS = 2_000;
// The server's row kinds. The SDK asks that rows be told apart by kind, never by label.
const LIMIT_KINDS = { session: "5-hour", weekly_all: "weekly" };
const NAMED_WINDOWS = { five_hour: "5-hour", seven_day: "weekly", seven_day_opus: "weekly Opus", seven_day_sonnet: "weekly Sonnet" };
const STATUS_WINDOWS = { five_hour: "5-hour", seven_day: "weekly" };

/**
 * Asks a short-lived `claude -p` for `get_usage` and returns the check. It
 * never starts a model call. Claude Code stays up while its input is open,
 * so its process group is stopped after the one answer or the timeout, and
 * the promise settles only once it has exited.
 */
export async function askClaudeUsage({ run = spawn, cwd = join(quotaDir(), "claude-cwd"), timeoutMs = 15_000 } = {}) {
  // An empty folder kept for this applies no project's settings or hooks.
  mkdirSync(cwd, { recursive: true });
  return new Promise((resolve) => {
    // Claude Code gets its own process group, so stopping it also stops anything it started.
    const child = run("claude", CLAUDE_ARGS, { cwd, stdio: ["pipe", "pipe", "ignore"], detached: true });
    let result;
    let killTimer;
    const finish = (value) => {
      if (result) return;
      result = value;
      clearTimeout(timer);
      stopGroup(child, "SIGTERM");
      killTimer = setTimeout(() => stopGroup(child, "SIGKILL"), KILL_WAIT_MS);
    };
    const timer = setTimeout(() => finish({ ok: false, code: "timeout" }), timeoutMs);
    child.on("error", (error) => {
      finish({ ok: false, code: error.code === "ENOENT" ? "no-tool" : "bad-answer" });
      clearTimeout(killTimer);
      resolve(result);
    });
    // An exit before the answer means Claude Code could not take the request.
    child.on("close", () => {
      finish({ ok: false, code: "bad-answer" });
      clearTimeout(killTimer);
      resolve(result);
    });
    // A Claude Code that exits early would otherwise end this process with EPIPE.
    child.stdin.on("error", () => {});
    let pending = "";
    child.stdout.on("data", (chunk) => {
      const lines = (pending + chunk).split("\n");
      pending = lines.pop();
      for (const line of lines) {
        const response = readControlResponse(line);
        if (response) finish(response.subtype === "success" ? readClaudeUsage(response.response) : { ok: false, code: "bad-answer" });
      }
    });
    child.stdin.write(`${JSON.stringify(USAGE_REQUEST)}\n`);
  });
}

/**
 * Returns the check from a `get_usage` reply. The server's `limits[]` rows
 * come first, where `weekly_all` is the weekly total across models, and the
 * named windows are the fallback. A reply without plan limits, such as one
 * for an API key, gives `no-plan`.
 */
export function readClaudeUsage(reply) {
  if (typeof reply !== "object" || reply === null) return { ok: false, code: "bad-answer" };
  if (reply.rate_limits_available === false || reply.subscription_type === null) return { ok: false, code: "no-plan" };
  const limits = reply.rate_limits;
  if (limits === null) return { ok: false, code: "empty-answer" };
  if (typeof limits !== "object") return { ok: false, code: "bad-answer" };
  const windows = Array.isArray(limits.limits) ? readLimitRows(limits.limits) : readNamedWindows(limits);
  return windows.length === 0 ? { ok: false, code: "empty-answer" } : { ok: true, windows };
}

/**
 * Returns the `5-hour` and `weekly` windows from status-line JSON, or [] when
 * it has none. It keeps only the used share and reset time, so no Claude
 * dollar amount reaches the store.
 */
export function readClaudeStatusWindows(status) {
  return Object.entries(STATUS_WINDOWS).flatMap(([key, name]) => {
    const window = status?.rate_limits?.[key];
    if (typeof window?.used_percentage !== "number") return [];
    // The status line gives reset times in epoch seconds.
    const resetsAt = typeof window.resets_at === "number" ? window.resets_at * 1000 : null;
    return [{ name, usedPercent: window.used_percentage, resetsAt }];
  });
}

/** Sends a signal to the child's process group, which is gone or never started when this fails. */
function stopGroup(child, signal) {
  if (child.pid === undefined) return;
  try {
    process.kill(-child.pid, signal);
  } catch {
    // ESRCH means the group has already exited.
  }
}

/** Returns the control response to this module's request in one output line, or undefined. */
function readControlResponse(line) {
  try {
    const message = JSON.parse(line);
    return message?.type === "control_response" && message.response?.request_id === REQUEST_ID ? message.response : undefined;
  } catch {
    return undefined;
  }
}

/** Returns windows from the server's rows, with a model's own weekly row as `weekly <model>`. */
function readLimitRows(rows) {
  return rows.flatMap((row) => {
    if (typeof row?.percent !== "number") return [];
    const model = row.scope?.model?.display_name;
    const name = row.kind === "weekly_scoped" && model ? `weekly ${model}` : LIMIT_KINDS[row.kind];
    return name ? [{ name, usedPercent: row.percent, resetsAt: parseTime(row.resets_at) }] : [];
  });
}

/** Returns windows from the reply's named windows, used when it has no `limits[]` rows. */
function readNamedWindows(limits) {
  return Object.entries(NAMED_WINDOWS).flatMap(([key, name]) => {
    const window = limits[key];
    return typeof window?.utilization === "number"
      ? [{ name, usedPercent: window.utilization, resetsAt: parseTime(window.resets_at) }]
      : [];
  });
}

/** Returns epoch milliseconds from an ISO time, or null when there is none. */
function parseTime(text) {
  const time = typeof text === "string" ? Date.parse(text) : Number.NaN;
  return Number.isNaN(time) ? null : time;
}
