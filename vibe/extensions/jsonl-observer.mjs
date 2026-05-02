import fs from "node:fs";

const logPath = process.env.VIBE_EXTENSION_LOG;

function append(entry) {
  if (!logPath) return;
  fs.appendFileSync(logPath, `${JSON.stringify(entry)}\n`);
}

export default function (pi) {
  pi.on("agent_start", async () => {
    append({ type: "agent_start_ext", ts: new Date().toISOString() });
  });

  pi.on("tool_execution_end", async (event) => {
    append({
      type: "tool_execution_end_ext",
      ts: new Date().toISOString(),
      tool: event.toolName,
      tool_call_id: event.toolCallId,
      is_error: event.isError,
    });
  });
}
