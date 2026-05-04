import fs from "node:fs";

const MUTATING_TOOLS = new Set(["edit", "write"]);
const FALLBACK_COMMIT_MESSAGE = "chore: snapshot changes";

let snapshotQueue = Promise.resolve();

function shellSingleQuote(value) {
  return `'${String(value).replace(/'/g, `'"'"'`)}'`;
}

function snapshotCommitMessage() {
  const commitMessageFile = process.env.VIBE_COMMIT_MESSAGE_FILE;
  if (!commitMessageFile) return FALLBACK_COMMIT_MESSAGE;

  try {
    const firstLine = fs.readFileSync(commitMessageFile, "utf8").split(/\r?\n/u)[0]?.trim();
    return firstLine || FALLBACK_COMMIT_MESSAGE;
  } catch {
    return FALLBACK_COMMIT_MESSAGE;
  }
}

function enqueueSnapshot(pi, toolName, toolCallId) {
  snapshotQueue = snapshotQueue
    .catch(() => {})
    .then(async () => {
      const commitMessage = snapshotCommitMessage();
      const script = `
        set -euo pipefail
        parent="$(git rev-parse HEAD)"
        if [[ -z "$(git status --porcelain)" ]]; then
          exit 0
        fi
        git add -A
        commit_message=${shellSingleQuote(commitMessage)}
        VIBE_COMMIT_KIND=snapshot git -c core.hooksPath=${shellSingleQuote(process.env.VIBE_GIT_HOOKS_DIR)} commit -m "$commit_message" >/dev/null
        sha="$(git rev-parse HEAD)"
        git update-ref ${shellSingleQuote(process.env.VIBE_SNAPSHOT_REF)} "$sha"
        git reset --soft "$parent"
      `;
      const result = await pi.exec("bash", ["-lc", script], { timeout: 30_000 });
      if (result.code !== 0) {
        throw new Error(`snapshot failed for ${toolName} ${toolCallId}: exit ${result.code}`);
      }
    });
  return snapshotQueue;
}

export default function (pi) {
  pi.on("tool_execution_end", async (event) => {
    if (event.isError) return;
    if (!MUTATING_TOOLS.has(event.toolName)) return;
    await enqueueSnapshot(pi, event.toolName, event.toolCallId);
  });

  pi.on("agent_end", async () => {
    await snapshotQueue;
  });
}
