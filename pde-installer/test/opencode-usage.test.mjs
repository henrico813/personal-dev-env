import assert from "node:assert/strict";
import test from "node:test";
import {
  describeContextBar,
  describeFinishGroup,
  describeFinishNotice,
  describeGroupLines,
  describeShortLine,
  describeSubagentLines,
  findApiPrice,
  listCatalogModels,
  listPlanProvidersUsed,
  listSubagents,
  priceSubagents,
  readLineMode,
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
      { name: "plan review", model: "claude-opus-5-5[1m]", providerID: "claude-code", tokens: 1_162_000, dollars: 1.6125, context: { tokens: 88_000, percent: 44 }, running: false },
      { name: "locator", model: "claude-opus-5-5", providerID: "anthropic", tokens: 88_000, dollars: 0.44, context: undefined, running: false },
    ],
    tokens: 1_250_000,
    dollars: 2.0525,
  });
});

/** A priced subagent row, with fields a test can override. */
const row = (fields = {}) => ({
  name: "locator",
  model: "claude-sonnet-4-5",
  tokens: 88_000,
  dollars: 0.42,
  context: { tokens: 88_000, percent: 40 },
  running: false,
  ...fields,
});

/** The priced result for its rows, with totals that match them. */
const priced = (...rows) => ({
  rows,
  tokens: rows.reduce((sum, entry) => sum + entry.tokens, 0),
  dollars: rows.reduce((sum, entry) => sum + (entry.dollars ?? 0), 0),
});

/** One plan line, as quota/lines.js draws it. */
const weekly = [{ text: "Claude weekly 41% used", level: "normal" }];

/** Returns the text of each line. */
const texts = (lines) => lines.map((line) => line.text);

/**
 * A phone shows only this line above the prompt, so it must carry the agent
 * count with what the run cost, or the context left when asked.
 */
test("describeShortLine shows cost or context left", () => {
  const three = priced(row({ dollars: 1.2 }), row({ dollars: 0.5 }), row({ dollars: 0.24 }));

  assert.equal(describeShortLine(three), "3 agents · $1.94 cost", "spent");
  assert.equal(describeShortLine(three, "left"), "3 agents · 60% context left", "left");
  assert.equal(describeShortLine(priced(row({ context: undefined })), "left"), "1 agent", "left, no context");
});

/** A total of $0.00 for a local model would read as free API use. */
test("describeShortLine unpriced shows zero", () => {
  assert.equal(describeShortLine(priced(row({ dollars: undefined }))), "1 agent · $0 cost");
});

/** A figure above the prompt of a session with no subagents reads as a phantom. */
test("describeShortLine empty without subagents", () => {
  assert.equal(describeShortLine(priced()), "");
});

/** A wrapped line splits numbers on a 40-column phone. */
test("describeShortLine cuts long line", () => {
  const three = priced(row(), row(), row());

  assert.equal(describeShortLine({ ...three, dollars: 1e16 }), "3 agents · $10000000000000000…");
});

/**
 * The sidebar and the /usage screen are the only group view, so they must
 * carry the running count, the token and price totals, and the plan lines.
 */
test("describeGroupLines totals agents tokens price", () => {
  const three = priced(row({ running: true }), row({ running: true }), row());

  assert.deepEqual(texts(describeGroupLines(three, weekly)), [
    "3 agents, 2 running",
    "264k tokens used · $1.26 API cost",
    "Claude weekly 41% used",
  ]);
});

/**
 * A machine with no plan login must show no plan line at all. The plugin
 * used to print `Claude plan unknown` there, about a plan nobody uses.
 */
test("describeGroupLines omits plans without login", () => {
  const local = priced(row({ model: "llama-3.3", dollars: undefined }));

  assert.deepEqual(texts(describeGroupLines(local, [])), ["1 agent, all done", "88k tokens used · $0 cost"]);
});

/** A known plan still guides a session with no subagents, while no plan shows nothing. */
test("describeGroupLines plan without subagents", () => {
  assert.deepEqual(texts(describeGroupLines(priced(), weekly)), ["Claude weekly 41% used"], "known plan");
  assert.deepEqual(describeGroupLines(priced(), []), [], "no plan");
});

/** A machine that runs Copilot through OpenCode shows no plan in its sidebar unless that use counts. */
test("listPlanProvidersUsed maps Copilot use", () => {
  const group = priced(row({ providerID: "github-copilot" }), row({ providerID: "ollama" }));

  assert.deepEqual(listPlanProvidersUsed(group, "claude-code"), ["claude", "copilot"]);
});

/**
 * Inside a subagent these lines are its only usage view on a phone, so the
 * context bar, share, tokens, price and state must all show.
 */
test("describeSubagentLines shows bar, price and state", () => {
  assert.deepEqual(describeSubagentLines(row()), [
    "claude-sonnet-4-5 ▓▓▓░░░░░ 40%",
    "88k tokens · $0.42 cost · done",
  ]);
  assert.deepEqual(describeSubagentLines(row({ context: undefined, dollars: undefined, running: true })), [
    "claude-sonnet-4-5 ········ --",
    "88k tokens · $0 cost · running",
  ]);
});

/** A subagent with no priced row must draw no lines rather than a broken one. */
test("describeSubagentLines empty without row", () => {
  assert.deepEqual(describeSubagentLines(undefined), []);
});

/** A wrapped line splits words and numbers on a 40-column phone, so a long model name gives way to the context share. */
test("describeSubagentLines cuts model, keeps share", () => {
  const lines = describeSubagentLines(row({ model: "m".repeat(40) }));

  assert.equal(lines[0], `${"m".repeat(23)}… ▓▓▓░░░░░ 40%`);
  assert.ok(lines.every((line) => line.length <= 37));
});

/**
 * A wrong bar makes the user end a good subagent early or run it past its
 * context window, so the filled cells must match the percent.
 */
test("describeContextBar fills cells for percent", () => {
  assert.equal(describeContextBar(40), "▓▓▓░░░░░");
  assert.equal(describeContextBar(20, 4), "▓░░░");
  assert.equal(describeContextBar(0), "░░░░░░░░");
  assert.equal(describeContextBar(120), "▓▓▓▓▓▓▓▓");
});

/** An unknown window must not look like an empty one, so the bar shows dots. */
test("describeContextBar unknown shows dots", () => {
  assert.equal(describeContextBar(undefined), "········");
});

/**
 * The notice is the only summary of a stopping subagent, so it must carry
 * its name, state, runtime and price.
 */
test("describeFinishNotice shows state, duration and cost", () => {
  assert.equal(describeFinishNotice(row({ dollars: undefined }), 45), "locator done · 88k · 45s · $0 cost");
  assert.equal(describeFinishNotice(row({ name: "a", running: true }), 125), "a running · 88k · 2m05s · $0.42 cost");
});

/** Ten or twenty subagents finishing together must still take one row above the prompt, not one row each. */
test("describeFinishGroup sums several finishes into one line", () => {
  const rows = [row({ dollars: undefined }), row({ dollars: undefined }), row({ name: "Read the installer README", dollars: undefined })];

  assert.equal(describeFinishGroup(rows), "3 done · Read th… +2 · 264k · $0 cost");
});

/** A finish notice must fit one phone line with its numbers whole. */
test("describeFinishNotice cuts long name, keeps numbers", () => {
  const notice = describeFinishNotice(row({ name: "Read the installer README and summarize it", dollars: undefined }), 118);

  assert.equal(notice, "Read th… done · 88k · 1m58s · $0 cost");
});

/** The short line's mode comes from env or options, and a wrong mode hides the run's cost. */
test("readLineMode chooses left or spent", () => {
  assert.equal(readLineMode(undefined, {}), "spent");
  assert.equal(readLineMode({ line: "spent" }, { PDE_USAGE_LINE: "left" }), "left");
  assert.equal(readLineMode({ line: "left" }, {}), "left");
  assert.equal(readLineMode(undefined, { PDE_USAGE_LINE: "spent" }), "spent");
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
