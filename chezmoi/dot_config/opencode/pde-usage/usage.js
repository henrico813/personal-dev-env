// Prices subagent tokens at API rates, measures their context and describes
// the Claude plan limits. view.jsx calls these with OpenCode's session data.
// Start with priceSubagents, describePromptLines, describeSubagentLines and
// describeSubagents.
import { readFileSync } from "node:fs";

// Providers whose listed prices are the model makers' own API rates.
const PRICE_MAKERS = ["anthropic", "openai"];
const PLAN_WINDOWS = {
  five_hour: "5-hour",
  seven_day: "weekly",
  seven_day_opus: "weekly Opus",
  seven_day_sonnet: "weekly Sonnet",
};
const SUBAGENTS_SHOWN = 5;
const NAME_WIDTH = 34;
// A phone in Moshi shows about 40 columns, less the row's padding of 2, and a
// wrapped line splits numbers.
const LINE_WIDTH = 37;
const CATALOG_URL = "https://models.dev/api.json";
const CATALOG_TIMEOUT_MS = 10_000;

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
 * Returns sidebar lines for priced subagents. Under each name come the model,
 * tokens and price, such as `opus-5-5 412k $1.71`, then `context 88k 44%`.
 */
export function describeSubagents({ rows, dollars }) {
  const noun = rows.length === 1 ? "subagent" : "subagents";
  const total = rows.some(isPriced) ? `${formatDollars(dollars)} at API rates` : "price unknown";
  const lines = [`${rows.length} ${noun}, ${total}`];
  for (const row of rows.slice(0, SUBAGENTS_SHOWN)) {
    const price = isPriced(row) ? formatDollars(row.dollars) : "price unknown";
    lines.push(row.name.slice(0, NAME_WIDTH), `  ${row.model} ${formatTokens(row.tokens)} ${price}`);
    if (row.context) lines.push(`  context ${formatContext(row.context)}`);
  }
  if (rows.length > SUBAGENTS_SHOWN) lines.push(`${rows.length - SUBAGENTS_SHOWN} more`);
  return lines;
}

/**
 * Returns the Claude plan limits the OpenCode Claude plugin saved, such as
 * `weekly, resets Nov 18, 02:00`, or undefined when nothing was saved. The
 * plugin saves only the window the latest Claude reply reported, and a share
 * only when that reply carried one.
 */
export function describePlanUsage(saved, { now = Date.now(), timeZone } = {}) {
  if (typeof saved !== "object" || saved === null) return undefined;
  let line = PLAN_WINDOWS[saved.rateLimitType] ?? saved.rateLimitType ?? "plan";
  if (isLimited(saved, now)) {
    line += `, limit reached until ${formatTime(saved.limitedUntil, timeZone)}`;
  } else if (typeof saved.utilization === "number") {
    line += ` ${Math.round(saved.utilization * 100)}%`;
  }
  // The plugin keeps the last reset time when an event leaves it out.
  if (saved.resetsAt > now) line += `, resets ${formatTime(saved.resetsAt, timeZone)}`;
  return line;
}

/**
 * Returns the two lines above a parent session's prompt, such as
 * `2 subagents, 1 running: 500k tokens` and `$1.94, largest context 44%`, or
 * an empty list without subagents. A phone-width screen hides the sidebar, so
 * these lines are all it shows. Before the first reply the first line ends in
 * `no usage yet` and the second is left out.
 */
export function describePromptLines({ rows, tokens, dollars }, saved, now = Date.now()) {
  const limit = isLimited(saved, now) ? ["plan limit reached"] : [];
  if (rows.length === 0) return limit;
  const noun = rows.length === 1 ? "subagent" : "subagents";
  const running = rows.filter((row) => row.running).length;
  const count = running > 0 ? `${rows.length} ${noun}, ${running} running` : `${rows.length} ${noun}`;
  if (tokens <= 0) return [fitLine(`${count}: no usage yet`), ...limit];
  // $0.00 would read as free API use when no subagent has a price.
  const parts = [rows.some(isPriced) ? formatDollars(dollars) : "price unknown"];
  const largest = rows
    .map((row) => row.context)
    .filter(Boolean)
    .sort((a, b) => b.tokens - a.tokens)[0];
  if (largest) parts.push(`largest context ${formatPercent(largest)}`);
  return [fitLine(`${count}: ${formatTokens(tokens)} tokens`), fitLine([...parts, ...limit].join(", "))];
}

/**
 * Returns the two lines above a subagent's own prompt, such as
 * `Subagent: plan review` and `412k tokens, $1.71, context 44%`. Before its
 * first reply the second line is `no usage yet`. `row` is one of
 * priceSubagents' rows.
 */
export function describeSubagentLines(row) {
  if (!row) return [];
  const title = fitLine(`Subagent: ${row.name}`);
  if (row.tokens <= 0) return [title, "no usage yet"];
  const parts = [`${formatTokens(row.tokens)} tokens`, isPriced(row) ? formatDollars(row.dollars) : "price unknown"];
  if (row.context) parts.push(`context ${formatPercent(row.context)}`);
  return [title, fitLine(parts.join(", "))];
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

/** Returns whether a saved plan limit still holds, since the plugin leaves an old limitedUntil behind. */
function isLimited(saved, now) {
  return Boolean(saved?.limited && saved.limitedUntil > now);
}

function isPriced(row) {
  return row.dollars !== undefined;
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

function formatContext({ tokens, percent }) {
  return percent === undefined ? formatTokens(tokens) : `${formatTokens(tokens)} ${percent}%`;
}

/** Returns a context's share of the window, or its size when the window is unknown. */
function formatPercent(context) {
  return context.percent === undefined ? formatTokens(context.tokens) : `${context.percent}%`;
}

/** Returns a line cut to the phone's width, ending in … when cut. */
function fitLine(line) {
  return line.length > LINE_WIDTH ? `${line.slice(0, LINE_WIDTH - 1)}…` : line;
}

function formatTime(milliseconds, timeZone) {
  return new Date(milliseconds).toLocaleString("en-US", {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
    hourCycle: "h23",
    timeZone,
  });
}
