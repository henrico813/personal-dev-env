# pdev2 Documentation

`pdev2` is one Go program with two command names. Run as `pde-gh-write`, it
records a pull-request write and runs `gh` once a human approves it. Run as
`pde-pr-approve`, it shows a waiting request in a terminal and records the
answer. These pages are for people changing the program. Paths and commands
are relative to `pdev2/` unless they start at the repository root.

The installer does not build this program yet. The shell scripts in
`chezmoi/dot_local/bin/` are still the installed commands.

## Explanation

- [How a request runs gh once](explanation/request-flow.md)

## Reference

- [States, exit codes, and files](reference/requests.md)

## User Guides

How to approve requests, and why approval exists, is covered in the chezmoi
docs:

- [Approve a pull-request write](../../chezmoi/docs/how-to/pr-write-approval.md)
- [Pull-request approval reference](../../chezmoi/docs/reference/pr-write-approval.md)
- [Pull-request write approval](../../chezmoi/docs/explanation/pr-write-approval.md)
- [Use phone approval for a pull-request write](../../chezmoi/docs/how-to/moshi-phone-approval.md)
