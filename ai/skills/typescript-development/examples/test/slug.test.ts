import assert from "node:assert/strict";
import { describe, test } from "node:test";

import { toSlug } from "../src/slug.js";

describe("toSlug", () => {
  const cases = [
    {
      name: "drops accents and punctuation",
      title: "Crème Brûlée, Please!",
      maxLength: 64,
      expected: "creme-brulee-please",
    },
    {
      name: "keeps a word ending at the limit",
      title: "alpha beta gamma",
      maxLength: 10,
      expected: "alpha-beta",
    },
    {
      name: "cuts long slugs at a hyphen",
      title: "alpha beta gamma",
      maxLength: 12,
      expected: "alpha-beta",
    },
  ];

  for (const { name, title, maxLength, expected } of cases) {
    test(name, () => {
      assert.equal(toSlug(title, { maxLength }), expected);
    });
  }
});
