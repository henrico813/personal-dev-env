# Run the comment checker

Run the review tool's comment check in the repository:

```bash
glimmer check-comments --base <commit your task started from>
```

Without `--base`, it checks the changes since the branch start, where
the branch split from main. If your harness lists the MCP tool
`check_comments`, even under a prefix, use it instead, with `dir`
set to the checkout's absolute path and `base` to that commit. Both
give the same JSON.

It checks every Go, Python and Rust file the branch changed, and
writes nothing. Act on its JSON:

- `result.failures`: fix each one, then run the check again. Each
  names its `rule` and quotes that rule's `sentence` from SKILL.md.
- `result.reported`: list each one in your final message, and edit
  its text only where Editing and review in SKILL.md allows a later
  pass to change existing prose. Its `note` says why it does not
  fail: `approved` text, `existing` text from before your task, a
  license `header`, or a `report_only` rule.
- `result.unsupported`: changed files the check cannot read. Check
  their comments and function names yourself.
- `error`: the check did not run. Do what its `message` says, such
  as fixing a file that does not parse.

Act on each failure by its rule:

- `approved`: you changed a comment the owner approved. Put back the
  owner's text, or ask to change it as the `comment-review` skill
  says.
- `approved-removed`: you removed a comment the owner approved, and
  its code stays. Put the comment back.
- `filler` or `avoided-word`: fix the sentence. `avoid` in the
  approvals file, `.git/pde/code-comments.json` in the main checkout,
  gives the word to use instead.
- `verb-first` and `restated-name`: act as the Comment checker
  section of SKILL.md says.
- Any other rule: rename the function or fix the comment.

Run the check again until `result.failures` is empty. `next` is then
`make_feedback_request_list`, even when findings are reported. Never
edit approved text to get past a failure. A function you renamed
keeps its comment without a failure, and the owner is asked about
that comment as a new one, so you list no renames.
