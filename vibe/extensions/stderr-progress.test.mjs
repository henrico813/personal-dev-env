import assert from "node:assert/strict";
import fs from "node:fs";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const samplePath = fileURLToPath(new URL("../testdata/pi-events-sample.jsonl", import.meta.url));
const formatterPath = fileURLToPath(new URL("./stderr-progress.mjs", import.meta.url));
const sample = fs.readFileSync(samplePath, "utf8");

function runFormatter(stderrLevel, input = sample) {
  const outDir = fs.mkdtempSync("/tmp/vibe-stderr-progress-");
  const result = spawnSync(process.execPath, [formatterPath], {
    input,
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

const terminalErrorEvent = `${JSON.stringify({
  type: "message_end",
  message: {
    role: "assistant",
    stopReason: "error",
    errorMessage: "No API key for provider: openai-codex",
  },
})}\n`;

// Redacted from a failed Pi run that Vibe previously reported as noop. This
// locks the real terminal error shape; it does not contact a live provider.
for (const stderrLevel of ["error", "warn", "info", "debug"]) {
  const errorRun = runFormatter(stderrLevel, terminalErrorEvent);
  assert.equal(errorRun.result.status, 1, stderrLevel);
  assert.match(
    errorRun.result.stderr,
    /agent: failed: No API key for provider: openai-codex/,
    stderrLevel,
  );
  assert.doesNotMatch(errorRun.result.stderr, /agent: finished/, stderrLevel);
  assert.equal(errorRun.eventsLog, terminalErrorEvent, stderrLevel);
}

const traceErrorRun = runFormatter("trace", terminalErrorEvent);
assert.equal(traceErrorRun.result.status, 1);
assert.equal(traceErrorRun.result.stderr, terminalErrorEvent);
assert.equal(traceErrorRun.eventsLog, terminalErrorEvent);
