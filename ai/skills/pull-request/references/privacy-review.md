# Privacy review

Guardrail 5 in [SKILL.md](../SKILL.md) says when to run this review.
The kinds of hit follow `git-messages` item 6. The brief lists them in
full, since the fresh agent reads only the brief.
Send a fresh agent the brief below, with `<worktree>`, `<BASE>` and
`<PR message file>` filled in. When you open a pull request, BASE is
`<remote>/<parent>`, the branch the pull request merges into: the
default branch, or the parent branch in a stack. For a review round,
the Update steps, it is `<remote>/<branch>`, the branch as last
pushed, so a round checks only what it adds.

For a commit that merges the base in and resolves a conflict, BASE is
`<remote>/<branch>` too. Change the brief you send in three ways. In
item 1, use `git -C <worktree> show --remerge-diff --format= HEAD`,
which shows only the lines the resolution wrote. Add `--first-parent`
to item 2's command. A plain diff and log would show every base line
and commit the merge brings in, and those are public already. Leave
out item 4, since a merge changes no PR text.

````text
You check text that is about to go public for anything private. Edit
nothing.

Read all of these, every line. Never sample, and never stop at the
first hit:
1. Each added or changed line, new files included:
   git -C <worktree> diff --unified=0 <BASE>...HEAD
2. Each commit message:
   git -C <worktree> log --format=%B <BASE>..HEAD
3. The branch name:
   git -C <worktree> branch --show-current
4. The PR title and body in <PR message file>.

A hit is a line that names or hints at:
- one particular machine: a hostname, a home folder path such as
  /home/<name>, a phrase such as "the office laptop", or a sentence
  that sets one machine apart from the rest, such as one with its own
  rules, files or limits. It counts even when it says "some PCs" or
  "one machine" and names none.
- a job: the user's company, clients or job projects, or any word
  that ties the text to a job, such as "office", "shift",
  "colleague", "boss" or "work" in the sense of a job. It counts when
  it is one part of a longer name, such as a skill or branch name, and
  even when it names no company.
- a private vault, note or project that is not in this repository.
- personal information, such as a person's name, email, address,
  health or money.
- a secret, such as a token, key, password or private URL.

A hint counts even with no name in it. Text that holds for every
machine and every user, such as "run this on each PC", is not a hit.
When you are unsure, report the line as a hit. A word quoted as an
example of a hit, in a rule or a test case like this brief, is not a
hit.

Every hit blocks. Never call a hit optional, low risk or a wording
choice.

Reply in this form:

Lines checked: <number of lines you read in the items above>
Hits: <number>
<file>:<line>: "<quote>" - <what it names or hints at>
Verdict: BLOCKED when Hits is above 0, else CLEAR

For a commit message, the branch name or PR text, write "commit",
"branch" or "PR" in place of <file>.
````
