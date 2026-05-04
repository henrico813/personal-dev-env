import fs from "node:fs";
import readline from "node:readline";

const LEVELS = { error: 0, warn: 1, info: 2, debug: 3, trace: 4 };
const stderrLevel = (process.env.VIBE_STDERR_LEVEL || "info").toLowerCase();
const currentLevel = LEVELS[stderrLevel] ?? LEVELS.info;
const eventsLogPath = process.env.VIBE_EVENTS_LOG;
const writer = eventsLogPath ? fs.createWriteStream(eventsLogPath, { flags: "w" }) : null;

function enabled(level) {
  return currentLevel >= LEVELS[level];
}

function emit(text, level = "info") {
  if (!enabled(level) || currentLevel === LEVELS.trace) return;
  process.stderr.write(`${text}\n`);
}

function handleEvent(event) {
  if (!event || typeof event.type !== "string") return;

  if (event.type === "agent_start") {
    emit("agent: started", "info");
    return;
  }

  if (event.type === "tool_execution_end") {
    const status = event.isError ? "failed" : "ok";
    const level = event.isError ? "error" : "info";
    emit(`tool: ${event.toolName} ${status}`, level);
  }
}

const rl = readline.createInterface({ input: process.stdin, crlfDelay: Infinity });
rl.on("line", (line) => {
  writer?.write(`${line}\n`);

  if (currentLevel === LEVELS.trace) {
    process.stderr.write(`${line}\n`);
    return;
  }

  try {
    handleEvent(JSON.parse(line));
  } catch {
    emit("warning: skipped non-JSON event line", "warn");
  }
});

rl.on("close", () => {
  writer?.end();
  emit("agent: finished", "info");
});
