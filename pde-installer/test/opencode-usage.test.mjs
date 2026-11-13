import assert from "node:assert/strict";
import test from "node:test";
import {
  describePlanUsage,
  describePromptLine,
  describeSubagentRow,
  describeSubagents,
  findApiPrice,
  listCatalogModels,
  listSubagents,
  priceSubagents,
} from "../../chezmoi/dot_config/opencode/pde-usage/usage.js";

/** Anthropic's base rates for Opus, in dollars per million tokens. */
const OPUS_PRICE = { input: 5, output: 25, cache: { read: 0.5, write: 6.25 } };
/** A host's own rates for the same model, below Anthropic's. */
const OWNHOST_PRICE = { input: 3, output: 15, cache: { read: 0.3, write: 3.75 } };

/**
 * A model list where Anthropic also lists a long-prompt tier, a reseller
 * undercuts it, and a free tier lists zero rates.
 */
const models = [
  {
    providerID: "anthropic",
    id: "claude-opus-5-5",
    limit: { context: 200_000 },
    cost: [{ tier: { type: "context", size: 200_000 }, input: 10, output: 37.5, cache: { read: 1, write: 12.5 } }, OPUS_PRICE],
  },
  { providerID: "cheapco", id: "claude-opus-5-5", cost: [{ input: 1, output: 5, cache: { read: 0.1, write: 1.25 } }] },
  { providerID: "freetier", id: "claude-opus-5-5", cost: [{ input: 0, output: 0, cache: { read: 0, write: 0 } }] },
  { providerID: "ownhost", id: "claude-opus-5-5", cost: [OWNHOST_PRICE] },
];

/** Each case is a name, the session's model and the price expected. */
const priceCases = [
  ["own provider first", { providerID: "ownhost", id: "claude-opus-5-5" }, OWNHOST_PRICE],
  ["maker over reseller", { providerID: "claude-code", id: "claude-opus-5-5[1m]" }, OPUS_PRICE],
  ["free tier skipped", { providerID: "freetier", id: "claude-opus-5-5" }, OPUS_PRICE],
  ["local model unpriced", { providerID: "local", id: "qwen3.8" }, undefined],
];

for (const [name, model, price] of priceCases) {
  /**
   * The price must come from the session's own provider or else the model's
   * maker, at base rates. A reseller's or free tier's entry would make a
   * subagent look cheaper than its API price, and a local model has none.
   */
  test(`findApiPrice ${name}`, () => {
    assert.deepEqual(findApiPrice(models, model), price);
  });
}

/** A models.dev catalog entry with a long-prompt rate beside its base rates. */
const catalog = {
  anthropic: {
    models: {
      "claude-opus-5-5": {
        cost: { input: 5, output: 25, cache_read: 0.5, cache_write: 6.25, context_over_200k: { input: 10, output: 37.5 } },
      },
    },
  },
};

/**
 * Without a connected Anthropic provider the catalog is the only source of
 * Claude prices, so a misread cache rate would misprice every cached call.
 */
test("listCatalogModels returns base rates", () => {
  const listed = listCatalogModels(catalog);

  assert.deepEqual(findApiPrice(listed, { providerID: "claude-code", id: "claude-opus-5-5" }), OPUS_PRICE);
});

/**
 * Every token kind has its own rate and reasoning bills as output, so
 * dropping one would understate a subagent's price. The largest subagent
 * comes first, so the five shown are the costliest. The context counts only
 * the last reply since the last compaction, as OpenCode's own panel does, so
 * a summed or stale count would overstate how full the subagent is.
 */
test("priceSubagents bills all tokens, largest first", () => {
  const infos = [
    {
      id: "s2",
      title: "locator",
      model: { providerID: "anthropic", id: "claude-opus-5-5" },
      tokens: { input: 88_000 },
      messages: [{ type: "assistant", tokens: { input: 88_000 } }, { type: "compaction", status: "completed" }],
    },
    {
      id: "s1",
      title: "plan review",
      model: { providerID: "claude-code", id: "claude-opus-5-5[1m]" },
      tokens: { input: 100_000, output: 10_000, reasoning: 2_000, cache: { read: 1_000_000, write: 50_000 } },
      messages: [
        { type: "assistant", tokens: { input: 40_000 } },
        { type: "assistant", tokens: { input: 80_000, output: 8_000 } },
        { type: "user" },
      ],
    },
  ];

  const priced = priceSubagents(infos, models);

  assert.deepEqual(priced, {
    rows: [
      { name: "plan review", model: "claude-opus-5-5[1m]", tokens: 1_162_000, dollars: 1.6125, context: { tokens: 88_000, percent: 44 } },
      { name: "locator", model: "claude-opus-5-5", tokens: 88_000, dollars: 0.44, context: undefined },
    ],
    tokens: 1_250_000,
    dollars: 2.0525,
  });
});

/**
 * The header sums the price, and a local model shows "no price" rather
 * than $0.00, which would read as free API use. With no priced subagent the
 * header says "no price" too, as the row above the prompt does.
 */
test("describeSubagents marks unpriced rows and header", () => {
  const priced = {
    rows: [
      { name: "plan review", model: "claude-opus-5-5[1m]", tokens: 1_162_000, dollars: 1.6125, context: { tokens: 88_000, percent: 44 } },
      { name: "locator", model: "qwen3.8", tokens: 88_000, dollars: undefined, context: undefined },
    ],
    dollars: 1.6125,
  };

  const lines = describeSubagents(priced);

  assert.deepEqual(lines, [
    "2 subagents, $1.61 at API rates",
    "plan review",
    "  claude-opus-5-5[1m] 1.2M $1.61",
    "  context 88k 44%",
    "locator",
    "  qwen3.8 88k no price",
  ]);
  assert.equal(describeSubagents({ rows: [priced.rows[1]], dollars: 0 })[0], "1 subagent, no price");
});

/**
 * A phone-width screen shows only this line, so it must carry the tokens,
 * price and fullest context of all subagents. The fullest sits between
 * smaller ones, so taking the first or last row fails. A total of $0.00 for
 * unpriced local models would read as free API use.
 */
test("describePromptLine sums subagents, shows fullest context", () => {
  const priced = {
    rows: [
      { tokens: 300_000, dollars: 1, context: { tokens: 60_000, percent: 30 } },
      { tokens: 100_000, dollars: undefined, context: { tokens: 88_000, percent: 44 } },
      { tokens: 100_000, dollars: 0.5, context: { tokens: 20_000, percent: 10 } },
    ],
    tokens: 500_000,
    dollars: 1.5,
  };
  const unpriced = { rows: [{ tokens: 51_000, dollars: undefined, context: undefined }], tokens: 51_000, dollars: 0 };

  assert.equal(describePromptLine(priced, undefined), "3 subagents 500k $1.50, max context 88k 44%");
  assert.equal(describePromptLine(unpriced, undefined), "1 subagent 51k no price");
});

/**
 * Subagents count 0 tokens until their first reply, and a bare 0 beside
 * "no price" read as a broken row on the phone, so the row waits.
 */
test("describePromptLine hides subagents before tokens", () => {
  const starting = { rows: [{ tokens: 0, dollars: undefined, context: undefined }], tokens: 0, dollars: 0 };

  assert.equal(describePromptLine(starting, undefined), "");
});

/**
 * Inside a subagent's view this row is the only place its own usage shows
 * on the phone, so it must carry the model, tokens, price and context.
 */
test("describeSubagentRow shows usage, waits for tokens", () => {
  const row = { model: "claude-opus-5-5", tokens: 412_000, dollars: 1.71, context: { tokens: 88_000, percent: 44 } };

  assert.equal(describeSubagentRow(row), "this subagent claude-opus-5-5 412k $1.71, context 88k 44%");
  assert.equal(describeSubagentRow({ ...row, tokens: 0 }), "");
});

/**
 * A subagent's sidebar must not count its parent or siblings, while a
 * root session counts its subagents' own subagents too.
 */
test("listSubagents includes nested, never the parent", () => {
  const parents = { root: undefined, child: "root", sibling: "root", grandchild: "child" };
  const sessions = {
    get: (id) => ({ id, parentID: parents[id] }),
    family: () => Object.keys(parents),
  };

  assert.deepEqual(listSubagents(sessions, "root"), ["child", "sibling", "grandchild"]);
  assert.deepEqual(listSubagents(sessions, "child"), ["grandchild"]);
});

/** The test's current time, and times six days after and eight days before it. */
const NOW = Date.UTC(2026, 10, 12, 9, 0);
const LATER = Date.UTC(2026, 10, 18, 2, 0);
const EARLIER = Date.UTC(2026, 10, 4, 2, 0);

/** Each case is a name, the saved plan file and the line expected. */
const planCases = [
  ["window and reset only", { status: "allowed", rateLimitType: "seven_day", resetsAt: LATER }, "weekly, resets Nov 18, 02:00"],
  ["share and reset", { rateLimitType: "seven_day", utilization: 0.414, resetsAt: LATER }, "weekly 41%, resets Nov 18, 02:00"],
  ["limit reached", { rateLimitType: "five_hour", limited: true, limitedUntil: LATER }, "5-hour, limit reached until Nov 18, 02:00"],
  [
    "stale times dropped",
    { rateLimitType: "seven_day", limited: true, limitedUntil: EARLIER, utilization: 0.2, resetsAt: EARLIER },
    "weekly 20%",
  ],
  ["nothing saved", undefined, undefined],
];

for (const [name, saved, line] of planCases) {
  /**
   * The plan line must show the window, a share when the reply carried one,
   * a live limit and a future reset, and nothing when no file was saved. The Claude plugin keeps old limit and
   * reset times, so showing them would claim a limit or reset that passed.
   */
  test(`describePlanUsage ${name}`, () => {
    assert.equal(describePlanUsage(saved, { now: NOW, timeZone: "UTC" }), line);
  });
}
