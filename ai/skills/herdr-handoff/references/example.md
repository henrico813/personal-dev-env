# Example

A real handoff, from the brief to the message to the user. One
session was building a comment review tool as a stack of pull
requests. The user found that a pull request did not follow the
design, and asked for the fixes to the design process to go to a new
session.

## The handoff file

The session saved
`~/.local/state/pde/handoffs/20261101-design-process.md` with these
parts:

- The project, in a few lines, with paths to the design, the roadmap
  page and the skill that builds the pull requests.
- What went wrong: planners were sent only some design sections, so
  they missed a rule that sat in another section.
- The user's decisions, numbered.
- The work in order: propose a new layout for the design and wait for
  a yes, gather the pending design edits into one section, then
  change the build skill in a pull request stacked on an open one.
- Open questions inside the task they belong to, each with the old
  session's proposal.
- Coordination, the part that kept the two sessions apart:

  ```markdown
  - The other session keeps the fix rounds for the open pull
    requests. It is also adding a `## Style Rules` section to the
    design right now. **Do not edit the design until the user or that
    session says the Style Rules section is done.** After that, you
    are the only session that edits the design.
  - Never touch the open pull requests' branches, worktrees or plans.
  ```

- Rules for vault notes, such as never changing a note with a `nobs`
  command.
- An appendix of design review findings. They existed only in the old
  session's conversation.

## Starting the session

The prompt was one line: "Read the handoff at <path> and follow it.
Load the skills it names first. Start by proposing the design layout.
Do not edit the design yet."

```sh
test "${HERDR_ENV:-}" = 1
herdr agent get "$HERDR_PANE_ID"    # .result.agent.agent is opencode
herdr workspace create \
  --cwd /home/justin/Projects/personal-dev-env \
  --label pdev-design-process --no-focus
# .result.root_pane.pane_id gives <pane>
herdr agent start design-process --kind opencode --pane <pane> \
  --timeout 90000
herdr agent prompt design-process "<prompt>" --wait \
  --timeout 60000
herdr agent get design-process      # agent_status is working
```

The prompt used plain `--wait`, which waits until the agent finishes,
and it timed out after 60 seconds with no status. `herdr agent get`
then showed the agent `working`. Use this instead, so the prompt
returns once the agent starts:

```sh
herdr agent prompt design-process "<prompt>" --wait \
  --until working --until blocked --timeout 30000
```

OpenCode has no model flag, so the new session ran the default model
from `~/.config/opencode/opencode.json`.

## The message to the user

```text
Comment review tool, the Rust program that asks you about agents'
code comments:

A new session now fixes the design process.
- Workspace: pdev-design-process (your screen did not move)
- Agent: design-process, OpenCode, on the default model
  claude-opus-5-5. Pick another model in that session if you want.
- Handoff: ~/.local/state/pde/handoffs/20261101-design-process.md

It first proposes how to split the design into parts, and waits for
your yes before moving any text. It does not edit the design until
the Style Rules section is done. This session keeps that section and
the fixes to the open pull requests.
```
