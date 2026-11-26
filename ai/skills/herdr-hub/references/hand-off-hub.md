# Hand off the hub

Read this when you hand the hub to a new session.

When you hand the hub to a new session, write its handoff with
`herdr-handoff`, holding only the run's state. First put each
question that waits on the user on its card with `hub ask`, as
Questions for the user says, so the new hub reads it from
`hub brief`. Copy into the handoff only a question that no card
could hold, which you asked in your own message. Never paste
`hub brief` output into the handoff: a copied question stays in the
next hub's list after the user answers it. The new hub runs
`hub brief` itself and checks each question and task as Questions
for the user says before its first reply. Start it with:

    hub handoff --run "<run folder>" <name> <handoff file>

Name it as Sidebar names says, with `<n>` one higher than yours, such
as `kanban-hub-14` after `kanban-hub-13`.

It starts `<name>` in a tab of your workspace on the model of the last
`--model '...'` line in `rules.md`, so keep the hub start command there
in that form. The new hub's tab gets `HUB_RUN` set to the run folder.
It checks its inputs before it starts anything. Before it
sends the handoff, it writes your session review's handoff. It adds to
the new hub's handoff that `hub clean` closes your tab once you are
idle, and that `hub next --start-reviews` starts that review if the
user asks for it. It
then reads back the reasoning effort of the new hub's first reply. Tell
the user the effort it printed, never one you assume. When it exits 3
with `started <name>, but`, the new hub runs: tell the user, who can
switch its effort in that session, and never run `hub handoff` again.
