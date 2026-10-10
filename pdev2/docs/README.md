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

In the full profile, the installer builds this program as
`~/.local/bin/pde-gh-write` and links `pde-pr-approve` to it. See
[update local builds](../../pde-installer/docs/how-to/update-local-builds.md)
to rebuild it after a change. The full profile also deploys the Herdr plugin
manifest from `chezmoi/dot_local/share/pde/herdr-plugins/pde-approval/` and
links it into Herdr after each chezmoi apply.

## Explanation

- [How a request runs gh once](explanation/request-flow.md)
- [Approving in the Herdr popup or on the phone](explanation/popup-and-phone-approval.md)

## Reference

- [States, exit codes, and files](reference/requests.md)

## User Guides

How to approve requests, and why approval exists, is covered in the chezmoi
docs:

- [Approve a pull-request write](../../chezmoi/docs/how-to/pr-write-approval.md)
- [Pull-request approval reference](../../chezmoi/docs/reference/pr-write-approval.md)
- [Pull-request write approval](../../chezmoi/docs/explanation/pr-write-approval.md)
- [Use phone approval for a pull-request write](../../chezmoi/docs/how-to/moshi-phone-approval.md)
