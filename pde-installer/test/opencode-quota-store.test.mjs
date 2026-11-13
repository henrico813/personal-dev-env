import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, test } from "node:test";
import {
  dueSources,
  mergeCheck,
  readPlanFiles,
  releaseLock,
  takeLock,
} from "../../chezmoi/dot_config/opencode/pde-usage/quota/store.js";

/** The test's current time, a check 2 hours earlier, and one minute in milliseconds. */
const NOW = Date.UTC(2026, 10, 13, 9, 0);
const TWO_HOURS_AGO = NOW - 2 * 3_600_000;
const MINUTE = 60_000;

/** Settings with Claude Code's usage request on, as the default is. */
const SETTINGS_ON = { claudeUsageCheck: "on" };

/** A Claude usage file whose last good check, at 48% weekly, was 2 hours ago. */
const usageFile = (fields = {}) => ({
  version: 1,
  provider: "claude",
  source: "claude-usage",
  checkedAt: TWO_HOURS_AGO,
  windows: [{ name: "weekly", usedPercent: 48, resetsAt: Date.UTC(2026, 10, 18, 2, 0) }],
  lastError: null,
  ...fields,
});

/** Returns a new empty folder that the test deletes when it ends. */
function makeQuotaDir(t) {
  const dir = mkdtempSync(join(tmpdir(), "pde-quota-"));
  t.after(() => rmSync(dir, { recursive: true, force: true }));
  return dir;
}

describe("mergeCheck", () => {
  /**
   * A short outage must leave the last number on screen with its age. The
   * first quota reader cleared it, so the user saw no number at all.
   */
  test("keeps last windows on failure", () => {
    const merged = mergeCheck(usageFile(), "claude-usage", { ok: false, code: "timeout" }, NOW);

    assert.deepEqual(merged, usageFile({ lastError: { at: NOW, code: "timeout" } }));
  });

  /** A stale `lastError` would keep `check failed` under a fresh number. */
  test("clears lastError after success", () => {
    const failed = usageFile({ lastError: { at: TWO_HOURS_AGO, code: "timeout" } });
    const windows = [{ name: "weekly", usedPercent: 50, resetsAt: null }];

    const merged = mergeCheck(failed, "claude-usage", { ok: true, windows }, NOW);

    assert.deepEqual(merged, usageFile({ checkedAt: NOW, windows }));
  });
});

describe("dueSources", () => {
  /** Each case is a name, the time since a `no-plan` check and whether it is due. */
  const dayCases = [
    ["23h59m later, not due", 24 * 60 * MINUTE - MINUTE, []],
    ["24h later, due", 24 * 60 * MINUTE, ["claude-usage"]],
  ];
  for (const [name, sinceMs, due] of dayCases) {
    /**
     * A machine with no plan must not be asked every 5 minutes. Without the
     * day wait, every OpenCode window there would start a check each time.
     */
    test(`asks no-plan sources daily, ${name}`, () => {
      const files = { "claude-usage": usageFile({ checkedAt: null, windows: [], lastError: { at: NOW - sinceMs, code: "no-plan" } }) };

      assert.deepEqual(dueSources(files, SETTINGS_ON, NOW), due);
    });
  }

  /**
   * Turning `quota_claude_usage_check` off is how the user stops the call
   * that skips their Claude Code settings, so it must stop every check.
   */
  test("skips sources turned off", () => {
    assert.deepEqual(dueSources({}, { claudeUsageCheck: "off" }, NOW, { force: true }), []);
  });
});

describe("takeLock", () => {
  /** Each case is a name and the text of a lock a running refresh holds. */
  const heldCases = [
    ["lock written", JSON.stringify({ pid: process.pid, at: Date.now() })],
    ["lock not yet written", ""],
  ];
  for (const [name, text] of heldCases) {
    /**
     * Two OpenCode windows must not both start `claude -p` at once, even
     * when one reads the lock before the other has written into it.
     */
    test(`refuses while refresh runs, ${name}`, (t) => {
      const dir = makeQuotaDir(t);
      writeFileSync(join(dir, "refresh.lock"), text);

      assert.equal(takeLock(dir, Date.now()), null);
    });
  }

  /** A killed refresh leaves its lock behind, and that must not stop every later refresh. */
  test("replaces lock of dead process", (t) => {
    const dir = makeQuotaDir(t);
    const gone = spawnSync(process.execPath, ["-e", ""]).pid;
    writeFileSync(join(dir, "refresh.lock"), JSON.stringify({ pid: gone, at: NOW }));

    const lock = takeLock(dir, NOW);

    assert.deepEqual(lock, { path: join(dir, "refresh.lock"), pid: process.pid });
    releaseLock(lock);
  });
});

describe("readPlanFiles", () => {
  /** An OpenCode window started before an update must show nothing for a newer file, not misread it. */
  test("skips newer file versions", (t) => {
    const dir = makeQuotaDir(t);
    writeFileSync(join(dir, "claude-usage.json"), JSON.stringify(usageFile({ version: 2 })));
    writeFileSync(join(dir, "claude-status-line.json"), JSON.stringify(usageFile({ source: "claude-status-line" })));

    assert.deepEqual(Object.keys(readPlanFiles(dir)), ["claude-status-line"]);
  });
});
