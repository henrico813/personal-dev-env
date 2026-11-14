// Reads the quota settings through `pde-installer setting get`, with built-in
// defaults until the per-machine settings catalog has them.
import { execFileSync } from "node:child_process";

/** @typedef {{claudeUsageCheck: "on" | "off"}} QuotaSettings */

// pde-installer answers from local files, so a slow answer means something is wrong.
const SETTING_TIMEOUT_MS = 2_000;

/**
 * Returns the settings, using the default for any key `pde-installer
 * setting get` cannot read. That includes a machine whose pde-installer has
 * no `setting` command yet.
 */
export function readSettings({ run = execFileSync } = {}) {
  return { claudeUsageCheck: readSetting(run, "quota_claude_usage_check", ["on", "off"], "on") };
}

/** Returns the key's value when it is one of `values`, else `fallback`. */
function readSetting(run, key, values, fallback) {
  try {
    const value = run("pde-installer", ["setting", "get", key], {
      encoding: "utf8",
      timeout: SETTING_TIMEOUT_MS,
      stdio: ["ignore", "pipe", "ignore"],
    }).trim();
    return values.includes(value) ? value : fallback;
  } catch {
    return fallback;
  }
}
