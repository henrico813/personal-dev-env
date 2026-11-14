import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, test } from "node:test";
import { fileURLToPath } from "node:url";
import { saveClaudeStatus } from "../../chezmoi/dot_config/opencode/pde-usage/quota/cli.js";
import { readPlanFiles } from "../../chezmoi/dot_config/opencode/pde-usage/quota/store.js";
import { makeFakeTools } from "./quota-fakes.mjs";

const CLI = fileURLToPath(new URL("../../chezmoi/dot_config/opencode/pde-usage/quota/cli.js", import.meta.url));

/** A `get_usage` reply at 48% weekly with no reset time, so the printed lines do not depend on the date. */
const usageReply = {
  subscription_type: "max",
  rate_limits_available: true,
  rate_limits: { limits: [{ kind: "weekly_all", group: "weekly", percent: 48, resets_at: null }] },
};

/**
 * Returns a function that runs `pde-quota` with its arguments and stdin, a
 * new state folder, no OpenCode Claude plugin file, and the fake tools from
 * `fakeOptions` first on PATH. The test deletes the folder when it ends.
 */
function makeQuotaRunner(t, fakeOptions) {
  const fake = makeFakeTools(t, usageReply, fakeOptions);
  const stateHome = mkdtempSync(join(tmpdir(), "pde-quota-state-"));
  t.after(() => rmSync(stateHome, { recursive: true, force: true }));
  const env = { ...fake.env, XDG_STATE_HOME: stateHome, OPENCODE_CLAUDE_RATE_LIMIT_STORE: join(stateHome, "none.json") };
  const run = (args, input = "") => spawnSync(process.execPath, [CLI, ...args], { env, input, encoding: "utf8", timeout: 20_000 });
  return { run, quotaDir: join(stateHome, "pde/quota") };
}

describe("pde-quota", () => {
  /** A break anywhere between Claude Code's answer, the saved file and the printed line hides the weekly number. */
  test("refresh then show prints Claude", (t) => {
    const { run } = makeQuotaRunner(t);

    const refresh = run(["refresh"]);
    const show = run(["show"]);

    assert.equal(refresh.status, 0, refresh.stderr);
    assert.equal(show.stdout, "Claude weekly 48% used\n");
  });

  /**
   * A machine without Claude Code must get `no-tool` and show no Claude line.
   * A `check failed` line there would be about a plan nobody uses.
   */
  test("refresh without claude shows nothing", (t) => {
    const { run } = makeQuotaRunner(t, { claude: "missing" });

    run(["refresh"]);
    const show = run(["show"]);

    assert.equal(show.stdout, "");
  });

  /** Output or a failure here lands in, or breaks, the user's Claude Code status line. */
  test("save-claude-status never prints or fails", (t) => {
    const { run } = makeQuotaRunner(t);

    const saved = run(["save-claude-status"], "not json");

    assert.deepEqual([saved.status, saved.stdout, saved.stderr], [0, "", ""]);
  });

  /** `hub usage` reads plan usage only through this reply, so its fields must hold. */
  test("show --json lists provider views", (t) => {
    const { run, quotaDir } = makeQuotaRunner(t);
    const checkedAt = Date.now();
    const file = {
      version: 1,
      provider: "claude",
      source: "claude-usage",
      checkedAt,
      windows: [{ name: "weekly", usedPercent: 48, resetsAt: null }],
      lastError: null,
    };
    mkdirSync(quotaDir, { recursive: true });
    writeFileSync(join(quotaDir, "claude-usage.json"), JSON.stringify(file));

    const show = run(["show", "--json", "--width", "72"]);

    assert.deepEqual(JSON.parse(show.stdout), [
      {
        provider: "claude",
        source: "claude-usage",
        checkedAt,
        failed: null,
        windows: [{ name: "weekly", usedPercent: 48, resetsAt: null, stale: false }],
        lines: ["Claude weekly 48% used"],
      },
    ]);
  });
});

describe("saveClaudeStatus", () => {
  /**
   * Claude Code runs the status line on every redraw. Without the one-minute
   * wait, each redraw would rewrite the file and wake every OpenCode window.
   */
  test("skips same windows within a minute", (t) => {
    const dir = mkdtempSync(join(tmpdir(), "pde-quota-"));
    t.after(() => rmSync(dir, { recursive: true, force: true }));
    const status = JSON.stringify({ rate_limits: { seven_day: { used_percentage: 48, resets_at: null } } });
    const first = Date.UTC(2026, 10, 13, 9, 0);

    saveClaudeStatus(status, { now: first, dir });
    saveClaudeStatus(status, { now: first + 59_000, dir });
    const early = readPlanFiles(dir)["claude-status-line"].checkedAt;
    saveClaudeStatus(status, { now: first + 60_000, dir });
    const late = readPlanFiles(dir)["claude-status-line"].checkedAt;

    assert.deepEqual([early, late], [first, first + 60_000]);
  });
});
