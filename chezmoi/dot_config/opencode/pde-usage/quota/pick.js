// Picks one view per provider from its plan files, for every display, so the
// sidebar, /usage and `pde-quota show` agree on which number to show.
import { homedir } from "node:os";
import { join } from "node:path";

/**
 * @typedef {{provider: import("./store.js").Provider, source: import("./store.js").Source | null,
 *   checkedAt: number | null, failed: import("./store.js").ErrorCode | null, limitedUntil?: number,
 *   windows: import("./store.js").PlanWindow[],
 *   spent?: import("./store.js").Spent & {budgetDollars: number | null, usedPercent: number | null},
 *   note?: string}} ProviderView
 */

const PROVIDER_ORDER = ["claude", "copilot", "codex"];
// These say the machine has no such plan, so the provider shows no line.
const HIDDEN_CODES = new Set(["no-tool", "no-login", "no-plan"]);

/**
 * Returns the views to show, in the order Claude, Copilot, Codex. Each
 * provider shows its newest good answer among its sources. A provider with
 * no good answer shows `check failed`, unless its newest error says the
 * machine has no tool, login or plan for it, or it was never checked; then
 * it shows nothing. `claudeLimit` adds Claude's hard limit. `settings` and
 * `now` are for Copilot's budget, which Claude does not use.
 */
export function pickProviderViews(files, claudeLimit, settings, now) {
  return PROVIDER_ORDER.flatMap((provider) => {
    const view = pickView(provider, Object.values(files).filter((file) => file.provider === provider));
    if (provider === "claude" && claudeLimit !== undefined) {
      return [{ ...(view ?? { provider, source: null, checkedAt: null, failed: null, windows: [] }), limitedUntil: claudeLimit }];
    }
    return view ? [view] : [];
  });
}

/**
 * Returns Claude's hard-limit end from the OpenCode Claude plugin's file
 * while it is in the future, else undefined. That plugin leaves an old
 * `limitedUntil` behind after the limit ends.
 */
export function readClaudeLimit(saved, now) {
  return saved?.limited === true && typeof saved.limitedUntil === "number" && saved.limitedUntil > now ? saved.limitedUntil : undefined;
}

/** Returns where the OpenCode Claude plugin saves its latest rate-limit event. */
export function claudeLimitPath(env = process.env) {
  const dataHome = env.XDG_DATA_HOME || join(homedir(), ".local/share");
  return env.OPENCODE_CLAUDE_RATE_LIMIT_STORE || join(dataHome, "opencode-claude/rate-limit.json");
}

/** Returns one provider's view from its files, or undefined when it shows nothing. */
function pickView(provider, files) {
  const good = files.filter((file) => file.checkedAt !== null).sort((a, b) => b.checkedAt - a.checkedAt)[0];
  if (good) {
    // A failure newer than the shown number is reported under it.
    const failed = good.lastError && good.lastError.at > good.checkedAt ? good.lastError.code : null;
    return { provider, source: good.source, checkedAt: good.checkedAt, failed, windows: good.windows, ...(good.note && { note: good.note }) };
  }
  const newest = files.filter((file) => file.lastError).sort((a, b) => b.lastError.at - a.lastError.at)[0];
  if (!newest || HIDDEN_CODES.has(newest.lastError.code)) return undefined;
  return { provider, source: newest.source, checkedAt: null, failed: newest.lastError.code, windows: [] };
}
