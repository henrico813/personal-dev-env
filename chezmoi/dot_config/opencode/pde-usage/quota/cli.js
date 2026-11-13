// Runs `pde-quota refresh`, `show` and `save-claude-status`. Plan usage is
// saved per source under ~/.local/state/pde/quota/, and every view reads it
// from there. Start with refreshPlans and showPlans.
import { spawn } from "node:child_process";
import { realpathSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { parseArgs } from "node:util";
import { readJsonFile } from "../usage.js";
import { askClaudeUsage, readClaudeStatusWindows } from "./claude.js";
import { describePlanLines } from "./lines.js";
import { claudeLimitPath, pickProviderViews, readClaudeLimit } from "./pick.js";
import { readSettings } from "./settings.js";
import {
  deleteDisabledFiles,
  deleteStaleTempFiles,
  dueSources,
  logCheck,
  mergeCheck,
  quotaDir,
  readPlanFiles,
  releaseLock,
  sameWindows,
  savePlanFile,
  takeLock,
} from "./store.js";

const CLI_PATH = fileURLToPath(import.meta.url);
// Claude Code runs the status line often, so an unchanged answer is saved at most this often.
const STATUS_SAVE_MS = 60_000;
// The flags each subcommand but save-claude-status takes.
const COMMAND_OPTIONS = {
  refresh: { source: { type: "string" }, force: { type: "boolean" } },
  show: { json: { type: "boolean" }, width: { type: "string" } },
};

const USAGE = `usage: pde-quota refresh [--source S] [--force]
       pde-quota show [--json] [--width N]
       pde-quota save-claude-status < status-line.json
`;

/**
 * Maps each asked source to a function of `{ now, settings }` that returns
 * its check. `claude-status-line` has no asker, since only
 * `save-claude-status` writes it.
 */
export const SOURCE_ASKERS = {
  "claude-usage": () => askClaudeUsage(),
};

/**
 * Runs one subcommand from `argv` and returns its exit code. `refresh`
 * returns 0 even when a check fails, since the failure is saved, and
 * `save-claude-status` always returns 0, so a status line never breaks.
 */
export async function main(argv, { env = process.env, now = Date.now, stdin = process.stdin, stdout = process.stdout } = {}) {
  const [command, ...rest] = argv;
  const dir = quotaDir(env);
  if (command === "save-claude-status") {
    try {
      saveClaudeStatus(await readText(stdin), { now: now(), dir });
    } catch {
      // Output here would land in the user's status line.
    }
    return 0;
  }
  let values;
  try {
    values = parseArgs({ args: rest, options: COMMAND_OPTIONS[command] ?? {} }).values;
  } catch {
    values = undefined;
  }
  const width = values?.width === undefined ? 37 : Number(values.width);
  if (!COMMAND_OPTIONS[command] || !values || !Number.isInteger(width) || width < 20) {
    process.stderr.write(USAGE);
    return 2;
  }
  if (command === "refresh") {
    await refreshPlans({ now: now(), source: values.source, force: values.force, dir });
  } else {
    stdout.write(showPlans({ now: now(), width, mode: values.json ? "json" : "lines", dir }));
  }
  return 0;
}

/**
 * Returns the plan lines from the saved files, or the provider views as JSON
 * with `json`, and starts a background refresh when a source is due. It
 * never waits on a provider.
 */
export function showPlans({ now, width = 37, mode = "lines", dir, settings }) {
  const current = settings ?? readSettings();
  const files = readPlanFiles(dir);
  const views = pickProviderViews(files, readClaudeLimit(readJsonFile(claudeLimitPath()), now), current, now);
  if (dueSources(files, current, now).length > 0) startRefresh();
  if (mode === "json") {
    const replies = views.map((view) => ({
      ...view,
      windows: view.windows.map((window) => ({ ...window, stale: window.resetsAt !== null && window.resetsAt <= now })),
      lines: describePlanLines([view], { now, width }).map((line) => line.text),
    }));
    return `${JSON.stringify(replies, null, 2)}\n`;
  }
  return describePlanLines(views, { now, width })
    .map((line) => `${line.text}\n`)
    .join("");
}

/**
 * Asks each due source once, saves each answer as it arrives and logs each
 * check. It returns at once when another refresh holds the lock, so many
 * OpenCode windows never ask a provider at the same time.
 */
export async function refreshPlans({ now, source, force = false, dir, settings, askers = SOURCE_ASKERS }) {
  const lock = takeLock(dir, now);
  if (lock === null) return;
  try {
    const current = settings ?? readSettings();
    const files = readPlanFiles(dir);
    const due = dueSources(files, current, now, { force }).filter((name) => source === undefined || name === source);
    await Promise.all(
      due.map(async (name) => {
        let result;
        try {
          result = await askers[name]({ now, settings: current });
        } catch {
          // A fault in a reader is saved as a failed check, so the source waits its interval before the next try.
          result = { ok: false, code: "bad-answer" };
        }
        savePlanFile(dir, mergeCheck(files[name], name, result, now));
        logCheck(dir, name, result, now);
      }),
    );
    deleteDisabledFiles(dir, current);
    deleteStaleTempFiles(dir, now);
  } finally {
    releaseLock(lock);
  }
}

/**
 * Saves the windows from Claude Code's status-line JSON. An unchanged answer
 * is saved at most once a minute, so the age a view shows is off by under a
 * minute. Bad JSON or JSON without windows saves nothing.
 */
export function saveClaudeStatus(stdinText, { now, dir }) {
  let status;
  try {
    status = JSON.parse(stdinText);
  } catch {
    return;
  }
  const windows = readClaudeStatusWindows(status);
  if (windows.length === 0) return;
  const old = readPlanFiles(dir)["claude-status-line"];
  if (old && sameWindows(old.windows, windows) && now - old.checkedAt < STATUS_SAVE_MS) return;
  savePlanFile(dir, mergeCheck(old, "claude-status-line", { ok: true, windows }, now));
}

/**
 * Spawns `pde-quota refresh` detached, without waiting, and ignores a
 * failure to start. It runs `node` by name, since inside OpenCode this
 * process is not Node.
 */
export function startRefresh() {
  const child = spawn("node", [CLI_PATH, "refresh"], { detached: true, stdio: "ignore" });
  // A missing node reports here, and the next due check tries again.
  child.on("error", () => {});
  child.unref();
}

/** Returns all of a stream's text. */
async function readText(stream) {
  let text = "";
  for await (const chunk of stream) text += chunk;
  return text;
}

/** Returns whether Node runs this file as the program, rather than importing it. */
function isProgram() {
  try {
    return realpathSync(process.argv[1]) === realpathSync(CLI_PATH);
  } catch {
    return false;
  }
}

if (isProgram()) process.exitCode = await main(process.argv.slice(2));
