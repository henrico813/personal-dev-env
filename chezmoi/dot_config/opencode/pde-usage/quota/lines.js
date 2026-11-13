// Turns provider views into plan lines that fit a 40-column phone, such as
// `Claude weekly 48% used · 2h ago` over `  check failed · resets Nov 18`.
// The plugin and `pde-quota show` both draw with describePlanLines.

/** @typedef {{text: string, level: "normal" | "warning" | "error"}} PlanLine */

const PROVIDER_NAMES = { claude: "Claude", copilot: "Copilot", codex: "Codex" };
const PARTS = " · ";
// A newer number than this needs no age, since checks run every 5 minutes.
const AGE_SHOWN_MS = 10 * 60_000;
// A model's own weekly row matters only once it nears its limit.
const SCOPED_SHOWN_PERCENT = 50;

/**
 * Returns the lines for `views`, each within `width`. Each window gets a
 * line with its used share, and its age from 10 minutes, over a line with
 * `check failed`, its reset and any note, keeping as many as fit. A window
 * that has reset since its check reads `unknown`. `timeZone` defaults to
 * the machine's own.
 */
export function describePlanLines(views, { now, width = 37, timeZone } = {}) {
  return views.flatMap((view) => {
    const name = PROVIDER_NAMES[view.provider];
    const lines = [];
    if (view.limitedUntil > now) {
      // A limit that ends on a later day shows its date only, to fit, and the weekly reset below gives the hour.
      lines.push({ text: cut(`${name} limit reached until ${formatReset(view.limitedUntil, now, timeZone, true)}`, width), level: "error" });
    }
    const windows = view.windows.filter((window) => !isScopedWindow(view, window) || window.usedPercent >= SCOPED_SHOWN_PERCENT);
    if (windows.length === 0 && view.failed) lines.push(normal(`${name} plan unknown`, width), normal("  check failed", width));
    for (const window of windows) {
      if (window.resetsAt !== null && window.resetsAt <= now) {
        lines.push(normal(`${name} ${window.name} unknown`, width), normal("  reset since last check", width));
        continue;
      }
      const age = now - view.checkedAt >= AGE_SHOWN_MS ? `${PARTS}${formatAge(now - view.checkedAt)}` : "";
      lines.push({ text: cut(`${name} ${window.name} ${Math.round(window.usedPercent)}% used${age}`, width), level: levelOf(window.usedPercent) });
      const short = Boolean(view.failed || view.note);
      const resets = window.resetsAt !== null && `resets ${formatReset(window.resetsAt, now, timeZone, short)}`;
      const below = fitParts([view.failed && "check failed", resets, view.note].filter(Boolean), width);
      if (below !== "") lines.push(normal(below, width));
    }
    return lines;
  });
}

/** Returns "warning" at 80% or more, "error" at 95% or more, else "normal", the same for every provider. */
export function levelOf(usedPercent) {
  if (usedPercent >= 95) return "error";
  if (usedPercent >= 80) return "warning";
  return "normal";
}

/** Returns a line cut to the limit, ending in … when cut. */
export function cut(line, limit) {
  return line.length > limit ? `${line.slice(0, limit - 1)}…` : line;
}

/** Returns whether a window is Claude's weekly row for one model, such as `weekly Fable`. */
function isScopedWindow(view, window) {
  return view.provider === "claude" && window.name.startsWith("weekly ");
}

/** Returns a plain line cut to the width. */
function normal(text, width) {
  return { text: cut(text, width), level: "normal" };
}

/** Returns the parts joined on one indented line, dropping parts from the end until it fits. */
function fitParts(parts, width) {
  for (let count = parts.length; count > 0; count--) {
    const line = `  ${parts.slice(0, count).join(PARTS)}`;
    if (line.length <= width) return line;
  }
  return parts.length === 0 ? "" : cut(`  ${parts[0]}`, width);
}

/** Returns how long ago a number was taken, such as `12m ago`, `2h ago` or `3d ago`. */
function formatAge(milliseconds) {
  const minutes = Math.floor(milliseconds / 60_000);
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.floor(minutes / 60);
  return hours < 48 ? `${hours}h ago` : `${Math.floor(hours / 24)}d ago`;
}

/**
 * Returns a reset time such as `14:00` later today, else `Nov 18, 02:00`, or
 * `Nov 18` when `short` because the line holds more than the reset.
 */
function formatReset(milliseconds, now, timeZone, short = false) {
  const day = { month: "short", day: "numeric", timeZone };
  const clock = { hour: "2-digit", minute: "2-digit", hourCycle: "h23", timeZone };
  const date = new Date(milliseconds);
  if (date.toLocaleDateString("en-US", day) === new Date(now).toLocaleDateString("en-US", day)) {
    return date.toLocaleTimeString("en-US", clock);
  }
  return date.toLocaleString("en-US", short ? day : { ...day, ...clock });
}
