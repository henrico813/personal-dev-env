# Use shared skills

Shared skills are reviewed host instructions installed under
`~/.agents/skills`. Create one directory per reviewed skill and place its
instruction files there, for example:

```bash
mkdir -p ~/.agents/skills/reviewed-skill
cp ./reviewed-skill/* ~/.agents/skills/reviewed-skill/
```

The automatic directory mount is Vibe's shared-skill mechanism.

Run Vibe normally after installing them:

```bash
vibe run \
  --key pdev-shared-skills \
  --prompt-file /tmp/vibe-task.txt \
  --model openai-codex/gpt-5.6-luna
```

Vibe automatically mounts `~/.agents/skills` read-only when it is present.
Inside the container Pi disables normal skill discovery with `--no-skills` and
then explicitly selects `/vibe-home/.agents/skills` with `--skill`. A missing
`~/.agents/skills` directory is allowed.

Find the run under `~/.local/state/vibe/<repo>/<key>/runs/` and inspect
`system-prompt.txt`, `combined-prompt.txt`, `events.jsonl`, and
`agent.stderr.log`. The prompt artifacts identify the executor prompt used;
the event and stderr logs provide the observable record of the agent's run and
skill-driven work. Also check `run.json` or `summary.json` for the final
result. The Docker command itself is not stored in the artifacts, so these
artifacts confirm the run's observable workflow rather than reproducing every
launch argument.

Only install skills you are willing to trust as executor instructions. The
read-only mount prevents writes through the mount, but it does not review, vet,
or sandbox the skill instructions.
