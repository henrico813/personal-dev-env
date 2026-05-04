import assert from "node:assert/strict";
import fs from "node:fs";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const samplePath = fileURLToPath(new URL("../testdata/pi-events-sample.jsonl", import.meta.url));
const formatterPath = fileURLToPath(new URL("./stderr-progress.mjs", import.meta.url));
const sample = fs.readFileSync(samplePath, "utf8");

function runFormatter(stderrLevel) {
  const outDir = fs.mkdtempSync("/tmp/vibe-stderr-progress-");
  const result = spawnSync(process.execPath, [formatterPath], {
    input: sample,
    encoding: "utf8",
    env: {
      ...process.env,
      VIBE_STDERR_LEVEL: stderrLevel,
      VIBE_EVENTS_LOG: `${outDir}/events.jsonl`,
    },
  });
  const eventsLog = fs.readFileSync(`${outDir}/events.jsonl`, "utf8");
  fs.rmSync(outDir, { recursive: true, force: true });
  return { result, eventsLog };
}

const infoRun = runFormatter("info");
assert.equal(infoRun.result.status, 0);
assert.match(infoRun.result.stderr, /agent: started/);
assert.match(infoRun.result.stderr, /tool: write ok/);
assert.match(infoRun.result.stderr, /tool: bash failed/);
assert.match(infoRun.result.stderr, /agent: finished/);
assert.doesNotMatch(infoRun.result.stderr, /"type":"tool_execution_end"/);
assert.equal(infoRun.eventsLog, sample);

const traceRun = runFormatter("trace");
assert.equal(traceRun.result.status, 0);
assert.equal(traceRun.result.stderr, sample);
assert.equal(traceRun.eventsLog, sample);
