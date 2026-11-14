// Prices subagent tokens at API rates, measures their context and describes
// the lines OpenCode shows above the prompt, in the sidebar, on the /usage
// screen and when a subagent finishes. view.jsx calls these with OpenCode's
// session data. Start with priceSubagents, describeShortLine,
// describeGroupLines and describeSubagentLines.
import { readFileSync } from "node:fs";
import { cut } from "./quota/lines.js";

// Providers whose listed prices are the model makers' own API rates.
const PRICE_MAKERS = ["anthropic", "openai"];
// A narrow screen wraps a longer line and hides the parts at its end.
const LINE_LIMIT = 30;
// A phone in Moshi shows about 40 columns, less the row's padding of 2, and a
// wrapped line splits numbers.
const GROUP_LIMIT = 37;
const PARTS = " · ";
const CATALOG_URL = "https://models.dev/api.json";
const CATALOG_TIMEOUT_MS = 10_000;

/** OpenCode provider IDs whose use counts against a plan, such as `claude-code` to `claude`. */
export const PLAN_PROVIDERS = { "claude-code": "claude" };

/**
 * Returns the subagents under a session, largest first, with their total
 * tokens and API price. Each info is an OpenCode session with its `messages`.
 * A row's `dollars` is undefined for a model with no listed price, such as a
 * local one, and its `context` is undefined before the first reply. An info's
 * `running` says whether OpenCode still runs that subagent.
 */
export function priceSubagents(infos, models) {
  const rows = infos
    .map((info) => {
      const tokens = info.tokens ?? {};
      const price = findApiPrice(models, info.model);
      return {
        name: info.title || info.id,
        model: info.model?.id ?? "unknown model",
        providerID: info.model?.providerID,
        tokens: countTokens(tokens),
        dollars: price && countApiDollars(price, tokens),
        context: measureContext(info.messages ?? [], models, info.model),
        running: Boolean(info.running),
      };
    })
    .sort((a, b) => b.tokens - a.tokens);
  return {
    rows,
    tokens: rows.reduce((sum, row) => sum + row.tokens, 0),
    dollars: rows.reduce((sum, row) => sum + (row.dollars ?? 0), 0),
  };
}

/**
 * Returns the one short line above a parent session's prompt, such as
 * `3 agents · $1.94 cost`, or an empty string when the session has no
 * subagents. Mode `left` asks what is still available instead of what the run
 * cost, so it shows context left instead of the cost.
 */
export function describeShortLine(priced, mode = "spent") {
  const rows = priced?.rows ?? [];
  if (rows.length === 0) return "";
  const parts = [describeCount(rows.length)];
  if (mode === "left") {
    const used = largestContext(rows);
    if (used !== undefined) parts.push(`${100 - used}% context left`);
  } else {
    parts.push(formatPrice(priced.dollars));
  }
  return cut(parts.join(PARTS), LINE_LIMIT);
}

/**
 * Returns the group lines for all of a session's subagents together, for the
 * usage sidebar and the /usage screen, then `planLines`. The first line
 * counts the agents and says who still runs, and the second totals tokens
 * and API price. Each line is a plan line object, so plan lines keep their
 * level. Each fits 37 characters.
 */
export function describeGroupLines(priced, planLines) {
  const rows = priced?.rows ?? [];
  const lines = [];
  if (rows.length > 0) {
    const running = rows.filter((row) => row.running).length;
    const state = running > 0 ? `${running} running` : "all done";
    lines.push(cut(`${describeCount(rows.length)}, ${state}`, GROUP_LIMIT));
    // $0.00 would read as free API use when no subagent has a price.
    const price = rows.some(isPriced) ? `${formatDollars(priced.dollars)} API cost` : "$0 cost";
    lines.push(cut(`${formatTokens(priced.tokens)} tokens used · ${price}`, GROUP_LIMIT));
  }
  return [...lines.map((text) => ({ text, level: "normal" })), ...planLines];
}

/**
 * Returns the plan providers a session group used, from its subagents' and
 * its own model providers, such as `["claude"]`.
 */
export function listPlanProvidersUsed(priced, rootProviderID) {
  const providerIDs = [rootProviderID, ...(priced?.rows ?? []).map((row) => row.providerID)];
  return [...new Set(providerIDs.map((providerID) => PLAN_PROVIDERS[providerID]).filter(Boolean))];
}

/**
 * Returns the two lines above a subagent's own prompt, such as
 * `claude-sonnet-4-5 ▓▓▓░░░░░ 40%` and `88k tokens · $0.42 cost · done`.
 * `row` is one of priceSubagents' rows.
 */
export function describeSubagentLines(row) {
  if (!row) return [];
  const percent = row.context?.percent;
  const share = typeof percent === "number" ? `${percent}%` : "--";
  const state = row.running ? "running" : "done";
  return [
    fitName(row.model, ` ${describeContextBar(percent)} ${share}`),
    `${formatTokens(row.tokens)} tokens · ${formatPrice(row.dollars)} · ${state}`,
  ];
}

/**
 * Returns a context bar such as `▓▓▓░░░░░`, where the filled cells are the
 * percent of the model's context window in use. An unknown percent gives only
 * dots, because the window is unknown rather than empty.
 */
export function describeContextBar(percent, cells = 8) {
  if (typeof percent !== "number" || !Number.isFinite(percent)) return "·".repeat(cells);
  const filled = Math.max(0, Math.min(cells, Math.round((percent / 100) * cells)));
  return "▓".repeat(filled) + "░".repeat(cells - filled);
}

/**
 * Returns the notice line shown above the prompt when a subagent stops, such
 * as `locator done · 88k · 45s · $0 cost`. `seconds` is how long it ran. A
 * long name is cut so the numbers still fit a phone line.
 */
export function describeFinishNotice(row, seconds) {
  const state = row.running ? "running" : "done";
  const parts = [formatTokens(row.tokens), formatDuration(seconds), formatPrice(row.dollars)];
  return fitName(row.name, ` ${state}${PARTS}${parts.join(PARTS)}`);
}

/**
 * Returns one line for several subagents that finished close together, such
 * as `4 done · Repo C +3 · 204k · $0 cost`, so 20 finishing at once still take
 * one row. `rows` are priceSubagents' rows, newest last.
 */
export function describeFinishGroup(rows) {
  const newest = rows.at(-1);
  const tokens = rows.reduce((sum, row) => sum + row.tokens, 0);
  const dollars = rows.reduce((sum, row) => sum + (row.dollars ?? 0), 0);
  const numbers = ` +${rows.length - 1}${PARTS}${formatTokens(tokens)}${PARTS}${formatPrice(dollars)}`;
  return `${rows.length} done${PARTS}${fitName(newest.name, numbers, GROUP_LIMIT - `${rows.length} done${PARTS}`.length)}`;
}

/**
 * Returns which short line to show, `left` when the user asked for what is
 * left over, `spent` otherwise. Either the environment variable or the
 * plugin's own options can ask for `left`.
 */
export function readLineMode(options, env = process.env) {
  return env?.PDE_USAGE_LINE === "left" || options?.line === "left" ? "left" : "spent";
}

/**
 * Returns the IDs of every subagent under a session, nested ones included.
 * `sessions` is OpenCode's `api.data.session`.
 */
export function listSubagents(sessions, sessionID) {
  return sessions.family(sessionID).filter((id) => {
    for (let info = sessions.get(id); info?.parentID; info = sessions.get(info.parentID)) {
      if (info.parentID === sessionID) return true;
    }
    return false;
  });
}

/**
 * Returns a model's API price in dollars per million tokens, or undefined.
 * `models` is OpenCode's model list, and the price is one of its `cost`
 * entries. The session's own provider comes first, then the model's maker. A
 * reseller may charge less, so it never sets the price. An ID such as
 * `claude-opus-5-5[1m]` also tries `claude-opus-5-5`.
 */
export function findApiPrice(models, model) {
  if (!model?.id) return undefined;
  const ids = [model.id, model.id.split("[")[0]];
  for (const providerID of [model.providerID, ...PRICE_MAKERS]) {
    for (const id of ids) {
      const listed = models.find((entry) => entry.providerID === providerID && entry.id === id);
      // An entry with a tier is a long-prompt rate, so the base rate has none.
      const price = listed?.cost?.find((cost) => !cost.tier);
      // A free tier lists zero rates, which are not the API price.
      if (price && listRates(price).some(Boolean)) return price;
    }
  }
  return undefined;
}

/**
 * Returns every model in the models.dev catalog, or an empty list when the
 * fetch fails. OpenCode's own list may leave out providers the owner has not
 * connected, such as `anthropic` when Claude runs through a plan.
 */
export async function fetchCatalogModels() {
  try {
    const response = await fetch(CATALOG_URL, { signal: AbortSignal.timeout(CATALOG_TIMEOUT_MS) });
    return response.ok ? listCatalogModels(await response.json()) : [];
  } catch {
    return [];
  }
}

/**
 * Returns the models in a models.dev catalog in the shape of OpenCode's model
 * list, with each model's base rates as its one `cost` entry.
 */
export function listCatalogModels(catalog) {
  return Object.entries(catalog ?? {}).flatMap(([providerID, provider]) =>
    Object.entries(provider?.models ?? {}).map(([id, model]) => ({
      providerID,
      id,
      limit: { context: model?.limit?.context },
      cost: model?.cost
        ? [
            {
              input: model.cost.input,
              output: model.cost.output,
              cache: { read: model.cost.cache_read, write: model.cost.cache_write },
            },
          ]
        : [],
    })),
  );
}

/** Returns parsed JSON from a file, or undefined when it is missing or not JSON. */
export function readJsonFile(path) {
  try {
    return JSON.parse(readFileSync(path, "utf8"));
  } catch {
    return undefined;
  }
}

/** Returns the cost of tokens at a price's rates; reasoning bills as output. */
function countApiDollars(price, tokens) {
  const counts = [
    tokens.input ?? 0,
    (tokens.output ?? 0) + (tokens.reasoning ?? 0),
    tokens.cache?.read ?? 0,
    tokens.cache?.write ?? 0,
  ];
  return listRates(price).reduce((sum, rate, index) => sum + counts[index] * (rate ?? 0), 0) / 1_000_000;
}

/** Returns a price's input, output, cache read and cache write rates, in that order. */
function listRates(price) {
  return [price.input, price.output, price.cache?.read, price.cache?.write];
}

/**
 * Returns the tokens in a session's context and their share of the model's
 * context window, the way OpenCode's Context panel counts them. That is the
 * last reply's tokens since the last finished compaction.
 */
function measureContext(messages, models, model) {
  const compacted = messages.findLastIndex((message) => message.type === "compaction" && message.status === "completed");
  const last = messages.findLast((message, index) => message.type === "assistant" && message.tokens && index > compacted);
  const tokens = last ? countTokens(last.tokens) : 0;
  if (tokens <= 0) return undefined;
  const ids = model?.id ? [model.id, model.id.split("[")[0]] : [];
  const window = [model?.providerID, ...PRICE_MAKERS]
    .flatMap((providerID) => ids.map((id) => models.find((entry) => entry.providerID === providerID && entry.id === id)))
    .map((entry) => entry?.limit?.context)
    .find(Boolean);
  return { tokens, percent: window ? Math.round((tokens / window) * 100) : undefined };
}

function isPriced(row) {
  return row.dollars !== undefined;
}

/** Returns the agent count with its singular or plural noun. */
function describeCount(count) {
  return `${count} ${count === 1 ? "agent" : "agents"}`;
}

/** Returns the largest context share of any row, or undefined when none has one. */
function largestContext(rows) {
  const shares = rows.map((row) => row.context?.percent).filter((percent) => typeof percent === "number");
  return shares.length === 0 ? undefined : Math.max(...shares);
}

/** Returns the cost, or `$0 cost` when there is no dollar amount, such as for a local model. */
function formatPrice(dollars) {
  return dollars ? `${formatDollars(dollars)} cost` : "$0 cost";
}

function countTokens(tokens) {
  const { input = 0, output = 0, reasoning = 0, cache = {} } = tokens;
  return input + output + reasoning + (cache.read ?? 0) + (cache.write ?? 0);
}

function formatDollars(dollars) {
  return `$${dollars.toFixed(2)}`;
}

function formatTokens(count) {
  if (count < 1000) return String(count);
  if (count < 1_000_000) return `${Math.round(count / 1000)}k`;
  return `${(count / 1_000_000).toFixed(1)}M`;
}

/** Returns how long a subagent ran, in seconds, or minutes and seconds past a minute. */
function formatDuration(seconds) {
  const whole = Math.max(0, Math.round(seconds ?? 0));
  if (whole < 60) return `${whole}s`;
  return `${Math.floor(whole / 60)}m${String(whole % 60).padStart(2, "0")}s`;
}

/** Returns the name and its numbers in one phone line, cutting the name, never the numbers. */
function fitName(name, numbers, limit = GROUP_LIMIT) {
  return cut(name, Math.max(1, limit - numbers.length)) + numbers;
}
