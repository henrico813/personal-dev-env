# Run the comment checker

Run the review tool's comment check in the repository:

```bash
glimmer check-comments --base <commit your task started from>
```

If your harness lists glimmer's MCP tools, even under a prefix, use
`check_comments` in place of the command. `glimmer help mcp` says
what it takes.

Read `glimmer help check-comments` before the first run. It says what
each part of the JSON reply means, and to list each reported finding
in your final message. Edit a reported finding's text only where
Editing and review in SKILL.md allows a later pass to change existing
prose. For a finding noted `approved`, follow the `comment-review`
skill instead.

Act on each failure by its rule:

- `approved`: you changed a comment the owner approved. Put back the
  owner's text, or ask to change it as the `comment-review` skill
  says.
- `approved-removed`: you removed a comment the owner approved, and
  its code stays. Put the comment back.
- `filler` or `avoided-word`: fix the sentence. The finding's
  `detail` gives the phrase to cut or the word to use instead.
- `verb-first`, failed or reported: act as
  [function-names.md](function-names.md) "A reported word" says.
- Any other rule: rename the function or fix the comment.

Run the check again until `result.failures` is empty. A function you
renamed keeps its comment without a failure, and the owner is asked
about that comment as a new one, so you list no renames.
