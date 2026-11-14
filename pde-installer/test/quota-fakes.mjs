// Fake provider tools for the quota tests, so no test starts the real
// `claude` or `pde-installer`. Each fake is put first on PATH.
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { delimiter, join } from "node:path";

/**
 * Returns a folder of fake tools and an environment with it first on PATH.
 * With `claude` set to `answer`, the fake `claude` answers `get_usage` with
 * `reply` and then stays up while its input is open, as the real one does.
 * It answers with `rate_limits` null unless given `--setting-sources
 * project`, as the real one does under the user's "no non-essential traffic"
 * setting. With `hang` it never answers, and with `missing` there is no
 * `claude` and PATH holds only the folder. The fake writes its process ID to
 * `pidFile`. The fake `pde-installer` knows no settings. The test deletes the
 * folder when it ends.
 */
export function makeFakeTools(t, reply, { claude: mode = "answer" } = {}) {
  const binDir = mkdtempSync(join(tmpdir(), "pde-quota-bin-"));
  t.after(() => rmSync(binDir, { recursive: true, force: true }));
  const pidFile = join(binDir, "claude.pid");
  const claude = `#!/usr/bin/env node
const { writeFileSync } = require("node:fs");
writeFileSync(${JSON.stringify(pidFile)}, String(process.pid));
const reply = ${JSON.stringify(reply)};
const skipsUserSettings = process.argv.join(" ").includes("--setting-sources project");
// A fake that nobody stops exits by itself, so a broken test fails instead of hanging.
setTimeout(() => process.exit(1), 20_000);
process.stdout.write(JSON.stringify({ type: "system", subtype: "init" }) + "\\n");
let pending = "";
process.stdin.on("data", (chunk) => {
  if (${JSON.stringify(mode === "hang")}) return;
  pending += chunk;
  if (!pending.includes("\\n")) return;
  const request = JSON.parse(pending.split("\\n")[0]);
  const usage = skipsUserSettings ? reply : { ...reply, rate_limits: null };
  const response = { subtype: "success", request_id: request.request_id, response: usage };
  process.stdout.write(JSON.stringify({ type: "control_response", response }) + "\\n");
});
`;
  if (mode !== "missing") writeFileSync(join(binDir, "claude"), claude, { mode: 0o755 });
  writeFileSync(join(binDir, "pde-installer"), "#!/bin/sh\nexit 1\n", { mode: 0o755 });
  // The real tools may sit beside node on PATH, so a missing fake leaves PATH with nothing else.
  const path = mode === "missing" ? binDir : `${binDir}${delimiter}${process.env.PATH}`;
  return { binDir, pidFile, env: { ...process.env, PATH: path } };
}
