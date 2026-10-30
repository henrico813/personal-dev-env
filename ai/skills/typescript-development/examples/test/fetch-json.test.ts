import assert from "node:assert/strict";
import { createServer, type Server } from "node:http";
import { after, before, describe, test } from "node:test";

import { fetchJson } from "../src/fetch-json.ts";

describe("fetchJson", () => {
  let server: Server;
  let baseUrl = "";
  before(async () => {
    server = createServer((request, response) => {
      if (request.url === "/ok") {
        response.setHeader("content-type", "application/json");
        response.end('{"ready":true}');
      } else if (request.url === "/unavailable") {
        response.statusCode = 503;
        response.end("try later");
      }
      // Any other path never answers, like a stalled server.
    });
    await new Promise<void>((resolve) => server.listen(0, resolve));
    const address = server.address();
    assert.ok(typeof address === "object" && address !== null);
    baseUrl = `http://127.0.0.1:${address.port}`;
  });
  after(() => {
    server.closeAllConnections();
    server.close();
  });

  test("returns the parsed JSON body", async () => {
    assert.deepEqual(await fetchJson(`${baseUrl}/ok`), { ready: true });
  });

  test("rejects with the failed status", async () => {
    await assert.rejects(fetchJson(`${baseUrl}/unavailable`), {
      name: "HttpError",
      status: 503,
    });
  });

  test("times out despite a caller signal", { timeout: 1_000 }, async () => {
    const controller = new AbortController();
    const request = fetchJson(`${baseUrl}/stall`, {
      timeoutMs: 50,
      signal: controller.signal,
    });

    await assert.rejects(request, { name: "TimeoutError" });
  });

  test("stops when the caller aborts", { timeout: 1_000 }, async () => {
    const controller = new AbortController();
    const request = fetchJson(`${baseUrl}/stall`, {
      signal: controller.signal,
    });
    controller.abort();

    await assert.rejects(request, { name: "AbortError" });
  });
});
