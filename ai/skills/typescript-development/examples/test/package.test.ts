import assert from "node:assert/strict";
import { describe, test } from "node:test";

async function listExportNames(specifier: string): Promise<string[]> {
  const loaded: unknown = await import(specifier);
  assert.ok(typeof loaded === "object" && loaded !== null);
  return Object.keys(loaded).sort();
}

describe("package entry point", () => {
  test("exposes only the public API", async () => {
    assert.deepEqual(await listExportNames("typescript-skill-examples"), [
      "ConfigError",
      "HttpError",
      "LOG_LEVELS",
      "fetchJson",
      "loadConfig",
      "parseConfig",
      "toSlug",
    ]);
    await assert.rejects(
      listExportNames("typescript-skill-examples/dist/config.js"),
      { code: "ERR_PACKAGE_PATH_NOT_EXPORTED" },
    );
  });
});
