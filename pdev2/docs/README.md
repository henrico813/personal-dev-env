# pdev2 Documentation

pdev2 is the program that makes coding agents ask you before they change a
pull request on GitHub. Agents run it as `pde-gh-write` instead of `gh` when
they write to a pull request. You can run it as `pde-pr-approve` to answer a
request in a terminal.

Pick the page for what you need to do:

- Set it up on a new computer:
  [Set up approval](how-to/set-up-approval.md).
- An agent is waiting for your answer:
  [Approve or decline a request](how-to/approve-a-request.md).
- A request seems stuck, or nothing happened after you approved:
  [Debug a stuck or unanswered request](how-to/debug-a-stuck-request.md).
- Look up an exact detail, such as an exit code, a popup key, or where
  requests are saved:
  [States, exit codes, and files](reference/requests.md).
- Find the code to change:
  [Where the code lives](reference/code-layout.md).
- Understand why it works the way it does:
  [How a request runs gh once](explanation/request-flow.md) and
  [Why popup and phone answers are safe](explanation/popup-and-phone-approval.md).

Pages in `how-to/` are steps to follow, pages in `reference/` are facts to
look up, and pages in `explanation/` give the reasons. Paths that start with
`internal/` or name `main.go` are inside `pdev2/`, paths that start with `~`
are in your home directory, and every other path starts at the top of the
repository.
