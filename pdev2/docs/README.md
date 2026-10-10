# pdev2 Documentation

`pdev2` is one Go program with two command names. Run as `pde-gh-write`, it
records a pull-request write and runs `gh` once a human approves it. Run as
`pde-pr-approve`, it shows a waiting request in a terminal or a Herdr popup
and records the answer. A request can also be answered on the phone through
Moshi. Herdr is the terminal multiplexer the agents run in; Moshi is a phone
app that the `moshi-hook` command sends yes-or-no questions to.

These pages are for people changing the program. Paths and commands are
relative to `pdev2/` unless they start at the repository root.

In the full profile, the installer builds this program as
`~/.local/bin/pde-gh-write` and links `pde-pr-approve` to it. See
[update local builds](../../pde-installer/docs/how-to/update-local-builds.md)
to rebuild it after a change. The full profile also deploys the Herdr plugin
manifest from `chezmoi/dot_local/share/pde/herdr-plugins/pde-approval/` and
links it into Herdr after each chezmoi apply.

## How-to Guides

- [Test popup changes without live Herdr](how-to/test-popup-changes.md)
- [Debug a stuck or unanswered request](how-to/debug-a-stuck-request.md)

## Explanation

- [How a request runs gh once](explanation/request-flow.md)
- [The Herdr popup and the phone ask](explanation/herdr-popup-and-phone.md)
- [The approval queue and runner](explanation/approval-queue-and-runner.md)

## Reference

- [States, exit codes, and files](reference/requests.md)

## User Guides

How to approve requests, and why approval exists, is covered in the chezmoi
docs:

- [Approve a pull-request write](../../chezmoi/docs/how-to/pr-write-approval.md)
- [Pull-request approval reference](../../chezmoi/docs/reference/pr-write-approval.md)
- [Pull-request write approval](../../chezmoi/docs/explanation/pr-write-approval.md)
- [Use phone approval for a pull-request write](../../chezmoi/docs/how-to/moshi-phone-approval.md)
