const MUTATING_TOOLS = new Set(["edit", "write"]);

async function commitSnapshot(pi, toolName, toolCallId) {
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
  await pi.exec("bash", ["-lc", script], { timeout: 30_000 });
}

export default function (pi) {
  pi.on("tool_result", async (event) => {
    if (event.isError) return;
    if (!MUTATING_TOOLS.has(event.toolName)) return;
    await commitSnapshot(pi, event.toolName, event.toolCallId);
  });
}
