# Blocked cards

Read this before you move a card into or out of `blocked`,
the stage for work that waits only on people outside the user's team.

- Move a card to `blocked` when its work waits only on
  people outside the user's team, such as another team's reviewers or
  a change another team must make first. It is a staff move, without
  `--user`. Name who it waits on with `--why`, such as
  `--why "platform team's review"`; `card` refuses the move without
  it. Work blocked on another team's change moves there from
  `active`. A pull request moves there from `in-review` only after the
  user reviewed it. Then take it off `reviews.md`, and send its worker
  no merge step, since the merge waits on the outside reviewers.
  `hub status` then shows the worker as `to-park`, and `hub clean`
  parks it. These cards count against neither limit. `hub brief`
  lists them, with the days since each moved there, under "Blocked
  (no action needed)", and `hub board` folds them into a
  "Blocked" group.
- Move a `blocked` card back once the user tells you, or
  the pull request shows, that the outside reviewers answered. These
  are staff moves too. When they approved, move it to `in-review`; the
  user's earlier review stands, so hand on the merge as SKILL.md's
  Merges says.
  When they asked something only the user can answer, move it to
  `in-review` and ask the user on the card with `hub ask`, as
  SKILL.md's Questions for the user says. When they asked
  for changes, move it to `active`. When that stage is full, leave
  the card where it is, tell the user, and move it once a place is
  free.
