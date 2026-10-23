# Pitfalls of a roadmap run

Each entry says what happens and what to do.

**Two agents work the same PR.** After your session restarts, an
agent you started may run twice. One finds plan files it did not make
and stops, or both write. Before starting a planner, check the vault
for plan files made in the last hour, and check your session's child
agents in OpenCode's database
(`~/.local/share/opencode/opencode.db`, table `session`, by
`parent_id`). Let the one that is further along finish.

**Agents share a scratch folder.** An agent that runs
`rm -rf /tmp/<name>` can delete another agent's work, and one that
clones into a fixed path can overwrite it. Every brief says to use
`mktemp -d` only. When a report admits a slip, check whether another
agent lost files.

**A lint failure does not stop the script.** `lint FILE && echo ok`
prints the failure and carries on, so a commit with a bad title gets
built and pushed. Write `lint FILE || exit 1`.

**A stopped script leaves files missing.** If the script stops before
it writes the Vibe prompt files, the next run fails with "read prompt
file". Check that every file the run reads exists before starting it.

**A child's base is a commit that no longer exists.** Squashing a
parent makes a new commit, and a child plan may name a scratch commit.
Move the child's `Base commit:` line to the parent's final commit and
run `planner check` before building.

**The comment checker fails on a lower PR's change.** A PR that
removes code with an approved comment makes the checker fail in every
PR above it, until the user's comment review of that PR records the
removal. Name it as known in every brief, so reviewers do not chase
it.

**A parent's new comment becomes false in a child.** It is not
approved until the parent's comment review, which comes first. Do not
edit it in the child's code. Add a reword to the child's comment
review, with the text the parent's review approved as `old`.

**A new branch starts behind the remote.** `wt switch --create`
starts from the local default branch, which may be behind the remote.
Before the first commit, reset the new branch to the remote's default
branch.

**A reviewer writes outside its folder.** A failed `cd` once wrote a
file into the worktrees folder. After reviews, run `git status` in the
main checkout and check that folder for new files.

**A new dependency breaks installs that CI never runs.** CI did not
build tools the way the installer does, so a module cache the
installer could not delete went unnoticed until review. When a PR adds
a dependency or a build step, have a reviewer build it the installer's
way, including its cleanup.

**A subagent's report is not proof.** Reports can say a skill was
loaded when it was not. Check the agent's skill loads in OpenCode's
database before trusting its review, and load any skill it used that
you have not.

**Skills are lost after a summary.** When the conversation is
summarized, the skills you loaded are gone from context. Load them
again before the next step.
