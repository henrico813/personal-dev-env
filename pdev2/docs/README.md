# pdev2 Documentation

`pdev2` is one Go program with two command names. Run as `pde-gh-write`, it
records a pull-request write and runs `gh` once a human approves it. Run as
`pde-pr-approve`, it shows a waiting request in a terminal and records the
answer. You can also answer in a popup in Herdr, the terminal program the
agents run in, or on your phone through the Moshi app.

The explanations describe what you see and why the program works this way.
The reference lists exact commands, states, exit codes, files, and limits
for lookup. Paths and commands are relative to `pdev2/` unless they start
at the repository root.

The installer does not build this program yet. The shell scripts in
`chezmoi/dot_local/bin/` are still the installed commands.

## Explanation

- [How a request runs gh once](explanation/request-flow.md)
- [Approving in the Herdr popup or on the phone](explanation/popup-and-phone-approval.md)
- [Where a request came from](explanation/request-context.md)
- [Approving after the agent stopped waiting](explanation/late-approvals.md)

## Reference

- [States, exit codes, and files](reference/requests.md)

## User Guides

How to approve requests, and why approval exists, is covered in the chezmoi
docs:

- [Approve a pull-request write](../../chezmoi/docs/how-to/pr-write-approval.md)
- [Pull-request approval reference](../../chezmoi/docs/reference/pr-write-approval.md)
- [Pull-request write approval](../../chezmoi/docs/explanation/pr-write-approval.md)
- [Use phone approval for a pull-request write](../../chezmoi/docs/how-to/moshi-phone-approval.md)
