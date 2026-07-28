import fs from "node:fs";
import readline from "node:readline";

const LEVELS = { error: 0, warn: 1, info: 2, debug: 3, trace: 4 };
const stderrLevel = (process.env.VIBE_STDERR_LEVEL || "info").toLowerCase();
const currentLevel = LEVELS[stderrLevel] ?? LEVELS.info;
const eventsLogPath = process.env.VIBE_EVENTS_LOG;
const writer = eventsLogPath ? fs.createWriteStream(eventsLogPath, { flags: "w" }) : null;
let agentErrorMessage = null;

function enabled(level) {
  return currentLevel >= LEVELS[level];
}

function emit(text, level = "info") {
  if (!enabled(level) || currentLevel === LEVELS.trace) return;
  process.stderr.write(`${text}\n`);
}

function terminalErrorMessage(event) {
  const message = event?.message;
  if (message?.stopReason !== "error") return null;

  return message.errorMessage || "agent stopped because of a provider error";
}

function handleProgressEvent(event) {
  if (!event || typeof event.type !== "string") return;

  if (event.type === "agent_start") {
    emit("agent: started", "info");
    return;
  }

  if (event.type === "tool_execution_end") {
    // Tool failures are recoverable; only terminal assistant errors fail the run.
    const status = event.isError ? "failed" : "ok";
    const level = event.isError ? "error" : "info";
    emit(`tool: ${event.toolName} ${status}`, level);
  }
}

const rl = readline.createInterface({ input: process.stdin, crlfDelay: Infinity });
rl.on("line", (line) => {
  writer?.write(`${line}\n`);

  try {
    const event = JSON.parse(line);
    agentErrorMessage ??= terminalErrorMessage(event);
    handleProgressEvent(event);
  } catch {
    if (currentLevel !== LEVELS.trace) {
      emit("warning: skipped non-JSON event line", "warn");
    }
  }

  if (currentLevel === LEVELS.trace) {
    process.stderr.write(`${line}\n`);
  }
});

rl.on("close", () => {
  writer?.end();
  if (agentErrorMessage) {
    emit(`agent: failed: ${agentErrorMessage}`, "error");
    process.exitCode = 1;
    return;
  }

  emit("agent: finished", "info");
});
