# pdev2 Documentation

`pdev2` is one Go program with two command names. Run as `pde-gh-write`, it
records a pull-request write and runs `gh` once a human approves it. Run as
`pde-pr-approve`, it shows a waiting request in a terminal or a Herdr popup
and records the answer. A request can also be answered on the phone through
Moshi. Herdr is the terminal multiplexer the agents run in; Moshi is a phone
app that the `moshi-hook` command sends yes-or-no questions to.

These pages are for people changing the program. Paths and commands are
relative to `pdev2/` unless they start at the repository root.

The installer does not build this program yet. The shell scripts in
`chezmoi/dot_local/bin/` are still the installed commands.

## How-to Guides

- [Test popup changes without live Herdr](how-to/test-popup-changes.md)

## Explanation

- [How a request runs gh once](explanation/request-flow.md)
- [The Herdr popup and the phone ask](explanation/herdr-popup-and-phone.md)

## Reference

- [States, exit codes, and files](reference/requests.md)

## User Guides

How to approve requests, and why approval exists, is covered in the chezmoi
docs:

- [Approve a pull-request write](../../chezmoi/docs/how-to/pr-write-approval.md)
- [Pull-request approval reference](../../chezmoi/docs/reference/pr-write-approval.md)
- [Pull-request write approval](../../chezmoi/docs/explanation/pr-write-approval.md)
- [Use phone approval for a pull-request write](../../chezmoi/docs/how-to/moshi-phone-approval.md)
