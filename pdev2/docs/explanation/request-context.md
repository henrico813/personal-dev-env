# Where a Request Came From

You may run several agents at once, and they can ask for approval within
seconds of each other. Earlier versions of the approval program made every
request look alike. The popup started with the request ID, and the phone
asked `Approve gh pr merge in current repository (185)?`, which does not
say which agent asked or what PR 185 is.

Now the popup and the phone question start with the same context lines,
built from what was saved with the request. This page explains those lines
and where they come from. It builds on
[How a request runs gh once](request-flow.md), which explains requests and
records, and on
[Approving in the Herdr popup or on the phone](popup-and-phone-approval.md),
which draws the popup.

## The Context Lines

For a comment on PR 12, asked by an agent in the Herdr workspace `app`, the
lines are:

```
comment #12
octo/app
app / review login fix
Fix login timeout
```

The popup shows them first. The phone question asks the first one as a
question, `Approve comment #12?`, followed by the other three.

1. What is asked, and of which PR: `comment #12`, `merge #185`, or
   `create PR`.
2. The repository. When `gh` cannot find one, for example outside a GitHub
   checkout, this line is `dir: ` and the folder name instead, so a folder
   is never mistaken for a repository.
3. Where the agent runs: the Herdr workspace and the title Herdr shows for
   the agent. When Herdr does not know the agent, for example outside
   Herdr, it is the OpenCode session ID.
4. The PR's title: the new title when the request sets one, otherwise the
   PR's current title from GitHub.

A line that cannot be found is left out rather than guessed.

## Where the Context Comes From

When `pde-gh-write` makes a new request, it looks up the context and saves
it in the record:

- The repository and the PR's title come from read-only `gh` lookups,
  `gh repo view` and `gh pr view`. Each may take up to 3 seconds. A
  `--repo` given by the agent is used as is.
- The OpenCode session ID comes from `OPENCODE_SESSION_ID` in the agent's
  environment. When it is not set, the session line is left out and Herdr
  is not asked.
- Inside Herdr, `herdr agent list` finds the agent running that session,
  which gives its workspace and title. `herdr workspace list` turns
  the workspace ID into the name you see in Herdr.

These lookups happen once, when the request is made, and are not part of
the request ID. If the network is down when the agent reruns the command,
the rerun still finds the same request.

Exact record fields and limits are in the
[reference](../reference/requests.md).
