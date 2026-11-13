import assert from "node:assert/strict";
import { describe, test } from "node:test";
import { pickProviderViews } from "../../chezmoi/dot_config/opencode/pde-usage/quota/pick.js";
import { describePlanLines, levelOf } from "../../chezmoi/dot_config/opencode/pde-usage/quota/lines.js";

/** The test's current time, `2026-11-13 09:00` UTC, and the weekly reset after it. */
const NOW = Date.UTC(2026, 10, 13, 9, 0);
const WEEK_END = Date.UTC(2026, 10, 18, 2, 0);
const MINUTE = 60_000;
/** Settings with Claude Code's usage request on, as the default is. */
const SETTINGS = { claudeUsageCheck: "on" };

/** A Claude plan file from `source`, at 48% weekly, with fields a test can override. */
const claudeFile = (source, fields = {}) => ({
  version: 1,
  provider: "claude",
  source,
  checkedAt: NOW,
  windows: [{ name: "weekly", usedPercent: 48, resetsAt: WEEK_END }],
  lastError: null,
  ...fields,
});

/** Claude's view at 48% weekly, checked `minutesAgo` before NOW, with fields a test can override. */
const claudeView = (minutesAgo, fields = {}) => ({
  provider: "claude",
  source: "claude-usage",
  checkedAt: NOW - minutesAgo * MINUTE,
  failed: null,
  windows: [{ name: "weekly", usedPercent: 48, resetsAt: WEEK_END }],
  ...fields,
});

/** Returns the text of each line `describePlanLines` draws for `views` at NOW in UTC. */
const texts = (views, width) => describePlanLines(views, { now: NOW, width, timeZone: "UTC" }).map((line) => line.text);

describe("pickProviderViews", () => {
  /**
   * A machine with Claude Code but no Claude login must show no Claude line.
   * `Claude plan unknown` there is about a plan nobody uses.
   */
  test("hides providers without login", () => {
    const files = { "claude-usage": claudeFile("claude-usage", { checkedAt: null, windows: [], lastError: { at: NOW, code: "no-login" } }) };

    assert.deepEqual(pickProviderViews(files, undefined, SETTINGS, NOW), []);
  });

  /** A status-line number from 2 minutes ago must beat a usage answer from 2 hours ago. */
  test("prefers newer Claude file", () => {
    const files = {
      "claude-usage": claudeFile("claude-usage", { checkedAt: NOW - 120 * MINUTE }),
      "claude-status-line": claudeFile("claude-status-line", { checkedAt: NOW - 2 * MINUTE }),
    };

    const [view] = pickProviderViews(files, undefined, SETTINGS, NOW);

    assert.equal(view.source, "claude-status-line");
  });
});

describe("describePlanLines", () => {
  /** Each case is a name, how old the number is in minutes, and the lines expected. */
  const ageCases = [
    ["9 minutes, no age", 9, ["Claude weekly 48% used", "  resets Nov 18, 02:00"]],
    ["10 minutes, age shown", 10, ["Claude weekly 48% used · 10m ago", "  resets Nov 18, 02:00"]],
  ];
  for (const [name, minutesAgo, lines] of ageCases) {
    /** A number 10 minutes old or more must say how old it is, so the user never takes it as current. */
    test(`shows age when stale, ${name}`, () => {
      assert.deepEqual(texts([claudeView(minutesAgo)], 37), lines);
    });
  }

  /** A 48% from before the weekly reset must never show as this week's number. */
  test("marks reset windows unknown", () => {
    const view = claudeView(0, { windows: [{ name: "weekly", usedPercent: 48, resetsAt: NOW - MINUTE }] });

    assert.deepEqual(texts([view], 37), ["Claude weekly unknown", "  reset since last check"]);
  });

  /** Each case is a name, the view and the lines expected. */
  const widthCases = [
    [
      "failed check drops note",
      claudeView(120, { failed: "timeout", note: "may change" }),
      ["Claude weekly 48% used · 2h ago", "  check failed · resets Nov 18"],
    ],
    [
      "limit ending later keeps date",
      claudeView(0, { limitedUntil: WEEK_END }),
      ["Claude limit reached until Nov 18", "Claude weekly 48% used", "  resets Nov 18, 02:00"],
    ],
  ];
  for (const [name, view, lines] of widthCases) {
    /**
     * A wrapped line splits numbers on a 40-column phone, and a cut one
     * loses its end. The longest real lines must drop a whole part instead.
     */
    test(`keeps lines within width, ${name}`, () => {
      assert.deepEqual(texts([view], 37), lines);
    });
  }
});

describe("levelOf", () => {
  /** Each case is a used percent and the level expected. */
  const levelCases = [
    [79, "normal"],
    [80, "warning"],
    [94, "warning"],
    [95, "error"],
  ];
  for (const [usedPercent, level] of levelCases) {
    /** The color is the user's first warning that a plan is nearly full, and it must be the same for every provider. */
    test(`marks warning and error levels, ${usedPercent}%`, () => {
      assert.equal(levelOf(usedPercent), level);
    });
  }
});
