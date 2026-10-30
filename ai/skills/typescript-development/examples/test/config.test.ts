import assert from "node:assert/strict";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { after, before, describe, test } from "node:test";

import { ConfigError, loadConfig, parseConfig } from "../src/config.ts";

describe("parseConfig", () => {
  test("fills defaults for missing fields", () => {
    assert.deepEqual(parseConfig({}), { port: 8080, logLevel: "info" });
  });

  test("rejects roots that are not objects", () => {
    assert.throws(() => parseConfig([]), ConfigError);
    assert.throws(() => parseConfig(null), ConfigError);
  });

  test("lists every invalid field", () => {
    assert.throws(() => parseConfig({ port: "80", logLevel: "loud" }), {
      name: "ConfigError",
      issues: [
        "port must be an integer from 1 to 65535",
        "logLevel must be one of debug, info, warn, error",
      ],
    });
  });
});

describe("loadConfig", () => {
  let dir = "";
  before(async () => {
    dir = await mkdtemp(join(tmpdir(), "config-test-"));
  });
  after(async () => {
    await rm(dir, { recursive: true, force: true });
  });

  test("keeps the JSON error as cause", async () => {
    const path = join(dir, "broken.json");
    await writeFile(path, "{ port: 80 }");

    await assert.rejects(loadConfig(path), (error) => {
      assert.ok(error instanceof ConfigError);
      assert.ok(error.cause instanceof SyntaxError);
      return true;
    });
  });
});
