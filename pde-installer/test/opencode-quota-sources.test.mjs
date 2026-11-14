import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, test } from "node:test";
import { readSettings } from "../../chezmoi/dot_config/opencode/pde-usage/quota/settings.js";
import {
  askClaudeUsage,
  readClaudeStatusWindows,
  readClaudeUsage,
} from "../../chezmoi/dot_config/opencode/pde-usage/quota/claude.js";
import { makeFakeTools } from "./quota-fakes.mjs";

/** Claude Code's `get_usage` reply recorded on this machine, trimmed to what the reader uses. */
const usageReply = JSON.parse(readFileSync(new URL("samples/quota/claude-get-usage.json", import.meta.url), "utf8"));
/** The sample's weekly reset, `2026-11-18 02:00` UTC. */
const WEEK_END = Date.UTC(2026, 10, 18, 2, 0);

describe("readClaudeUsage", () => {
  /**
   * The `weekly_all` row is the total across models the user asked for, and
   * a model's own row must keep its name, or the two would show as one.
   */
  test("takes weekly total", () => {
    assert.deepEqual(readClaudeUsage(usageReply), {
      ok: true,
      windows: [
        { name: "weekly", usedPercent: 48, resetsAt: WEEK_END },
        { name: "weekly Fable", usedPercent: 0, resetsAt: WEEK_END },
      ],
    });
  });

  /** The request is experimental, so a reply without `limits[]` must still give the weekly number. */
  test("falls back to windows", () => {
    const { limits, ...named } = usageReply.rate_limits;

    assert.deepEqual(readClaudeUsage({ ...usageReply, rate_limits: named }), {
      ok: true,
      windows: [{ name: "weekly", usedPercent: 48, resetsAt: WEEK_END }],
    });
  });

  /** Each case is a name, the reply and the code expected. */
  const unusableCases = [
    ["no subscription", { ...usageReply, subscription_type: null, rate_limits_available: false, rate_limits: null }, "no-plan"],
    ["rate limits null", { ...usageReply, rate_limits: null }, "empty-answer"],
    ["unknown shape", { usage: "48%" }, "bad-answer"],
  ];
  for (const [name, reply, code] of unusableCases) {
    /**
     * `no-plan` hides Claude, while the other codes show `check failed`. A
     * wrong code hides a plan the user has or reports one they do not.
     */
    test(`codes unusable replies, ${name}`, () => {
      assert.deepEqual(readClaudeUsage(reply), { ok: false, code });
    });
  }
});

describe("readClaudeStatusWindows", () => {
  /**
   * The status line's reset times are in seconds while the store's are in
   * milliseconds, and the store must never hold a Claude dollar amount.
   */
  test("converts reset seconds, drops dollars", () => {
    const status = {
      cost: { total_cost_usd: 1.25 },
      rate_limits: {
        five_hour: { used_percentage: 12, resets_at: 1794578400 },
        seven_day: { used_percentage: 48, resets_at: 1794967200 },
        spend_limit: { used_usd: 3.5 },
      },
    };

    assert.deepEqual(readClaudeStatusWindows(status), [
      { name: "5-hour", usedPercent: 12, resetsAt: 1794578400000 },
      { name: "weekly", usedPercent: 48, resetsAt: 1794967200000 },
    ]);
  });
});

describe("askClaudeUsage", () => {
  /**
   * The answer must come from a run that skips the user's own settings, or
   * it has no numbers. Claude Code must have exited when the check returns,
   * or one leaked `claude` per check piles up over a day.
   */
  test("answers without user settings, then exits", async (t) => {
    const fake = makeFakeTools(t, usageReply);

    const result = await askClaudeUsage({ run: runWith(fake.env), cwd: makeEmptyDir(t), timeoutMs: 10_000 });

    assert.equal(result.ok, true);
    assert.deepEqual(result.windows[0], { name: "weekly", usedPercent: 48, resetsAt: WEEK_END });
    assert.throws(() => process.kill(Number(readFileSync(fake.pidFile, "utf8")), 0), { code: "ESRCH" });
  });

  /**
   * A Claude Code that never answers must give `timeout`, which shows
   * `check failed`, and must still be stopped, or each check leaves one behind.
   */
  // The timeout fails a run that never stops `claude`, before the fake exits by itself.
  test("times out and stops silent claude", { timeout: 10_000 }, async (t) => {
    const fake = makeFakeTools(t, usageReply, { claude: "hang" });

    const result = await askClaudeUsage({ run: runWith(fake.env), cwd: makeEmptyDir(t), timeoutMs: 500 });

    assert.deepEqual(result, { ok: false, code: "timeout" });
    assert.throws(() => process.kill(Number(readFileSync(fake.pidFile, "utf8")), 0), { code: "ESRCH" });
  });
});

/** Returns a spawn that runs each tool with `env`, so the fakes come first on PATH. */
function runWith(env) {
  return (command, args, options) => spawn(command, args, { ...options, env });
}

/** Returns a new empty folder for Claude Code to run in, which the test deletes when it ends. */
function makeEmptyDir(t) {
  const dir = mkdtempSync(join(tmpdir(), "pde-quota-cwd-"));
  t.after(() => rmSync(dir, { recursive: true, force: true }));
  return dir;
}

describe("readSettings", () => {
  /** Each case is a name, what `pde-installer setting get` does, and the setting expected. */
  const settingCases = [
    ["off is kept", () => "off\n", "off"],
    ["unknown value is on", () => "maybe\n", "on"],
    ["failed command is on", () => { throw new Error("unknown command"); }, "on"],
  ];
  for (const [name, run, expected] of settingCases) {
    /**
     * Turning the check off is how the user stops the call that skips their
     * Claude Code settings, and a pde-installer without the setting must
     * still check.
     */
    test(`reads quota_claude_usage_check, ${name}`, () => {
      assert.deepEqual(readSettings({ run }), { claudeUsageCheck: expected });
    });
  }
});
