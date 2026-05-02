const MUTATING_TOOLS = new Set(["edit", "write"]);

let snapshotQueue = Promise.resolve();

function enqueueSnapshot(pi, toolName, toolCallId) {
  snapshotQueue = snapshotQueue
    .catch(() => {})
    .then(async () => {
      const script = `
        set -euo pipefail
        parent="$(git rev-parse HEAD)"
        if [[ -z "$(git status --porcelain)" ]]; then
          exit 0
        fi
        git add -A
        VIBE_COMMIT_KIND=snapshot git -c core.hooksPath="${process.env.VIBE_GIT_HOOKS_DIR}" commit -m "vibe snapshot: ${toolName} ${toolCallId}" >/dev/null
        sha="$(git rev-parse HEAD)"
        git update-ref "${process.env.VIBE_SNAPSHOT_REF}" "$sha"
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
