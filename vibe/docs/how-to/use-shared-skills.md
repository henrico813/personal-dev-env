# Use user and repository skills

Shared skills are reviewed host instructions installed under
`~/.agents/skills`. Create one directory per reviewed skill and place its
instruction files there, for example:

```bash
mkdir -p ~/.agents/skills/reviewed-skill
cp ./reviewed-skill/* ~/.agents/skills/reviewed-skill/
```

Repository-specific skills can instead be committed under `.agents/skills` in
the target repository. Vibe reads them from the exact revision checked out in
the managed worktree; do not copy them into the user directory. Vibe rejects
repository skill roots with changed, untracked, ignored, symlinked, or
multiply-linked contents, and rejects index flags that can hide changes.

Run Vibe normally after installing them:

```bash
vibe run \
  --key pdev-shared-skills \
  --prompt-file /tmp/vibe-task.txt \
  --model openai-codex/gpt-5.6-luna
```

Vibe mounts each existing skill root read-only. The user root appears at
`/vibe-home/.agents/skills`. The repository root remains at
`<managed-worktree>/.agents/skills`, where its nested mount protects the files
from writes through the worktree mount. Inside the container Pi disables normal
skill discovery with `--no-skills`, then explicitly selects the user root first
and the repository root second. Either directory may be missing.

Find the run under
`~/.local/state/vibe/<repo>-<16-hex-git-common-dir-hash>/<slug>/runs/`.
State under the old `~/.local/state/vibe/<basename>/` layout is intentionally
orphaned and is not migrated. Inspect
`system-prompt.txt`, `combined-prompt.txt`, `events.jsonl`, and
`agent.stderr.log`. The prompt artifacts identify the executor prompt used;
the event and stderr logs provide the observable record of the agent's run and
skill-driven work. Also check `run.json` or `summary.json` for the final
result. The Docker command itself is not stored in the artifacts, so these
artifacts confirm the run's observable workflow rather than reproducing every
launch argument.

Only install or commit skills you are willing to trust as executor
instructions. Read-only mounts prevent writes through the mounted paths, but
Vibe does not review, vet, or sandbox the instructions.
