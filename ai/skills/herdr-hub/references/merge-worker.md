# Merge worker

Read this before you hand a merge to a merge worker, and when you read
its summary.

Hand any other merge first to a merge worker. A merge worker is a new
Herdr session on the model `model-routing` picks for a
`merge-script` job, never a
subagent of the hub. A subagent ends when the hub's turn is
interrupted, and the user cannot see it. Start it with
`herdr-handoff`. Pass `--auto`, and set `--model` to that model.
Make a folder with `mktemp -d`. Brief the worker as `model-routing`
says for a local job. In that brief, put the command below in place
of `bash <script>`, and name `<folder>/summary.txt` as the summary
file. When the repository has more than one remote, put
`--remote <remote>` after the test command.
For a personal-dev-env pull request, add `--update` at the end, as in
`'<test command>' --remote <remote> --update`. `--update` makes the
same worker run `pde-installer update` once the pull request has
merged. That command updates the checkout at `install_path` in
`~/.config/pde/config.json` to the newest main and installs it, so
the hub starts no separate session to install the merge.

Start a merge worker only for a pull request the user asked to merge,
as `ai/AGENTS.md` says, and whose Testing records that the change was
installed and tried, as `pull-request` Guardrail 4 asks.
Otherwise hand the merge on as that skill's Merge says. When `rules.md`
names an environment setting that `git push` needs, such as an SSH
agent socket, put it just before `bash` in the brief's command.

    cd <repository> &&
      bash ~/.agents/skills/herdr-hub/scripts/merge-worker.sh \
        <number> <folder> '<test command>'

The script merges the base into a branch that is behind it and pushes
the merge as a new commit. It then runs `pull-request`'s merge checks
and, when they pass, sends the merge request. The user approves it in
the popup or on the phone, which shows the squash message. The script
keeps running when the shell tool stops it.

Read `<folder>/summary.txt` yourself. When the summary fails the check
in `model-routing`, retry the merge worker as that skill says, with
the same folder. The script starts no second run in the same folder.
A new folder would start a second run, which could send a second
merge request. The line before `exit:` says what happened:

- `merged: <commit>`: handle it as a merged pull request, as SKILL.md's
  Reports from sessions says.
- `update installed: main is at <commit>` or
  `update current: main is at <commit>`: the merged line comes just
  before it. Handle it as a merged pull request; the update needs no
  further check. The three `update` lines appear only with
  `--update`.
- `update: failed with exit <code>: <reason>; see update.log`: the
  pull request merged, but this machine does not run the newest main.
  Accept the merged work as for `merged`, and tell the user, with
  that line and the last lines of `<folder>/update.log`.
- `result: conflict with <base>: <files>`: resolving a conflict
  writes code, so it is a local job. Hand it to a worker as
  `herdr-handoff`'s "Hand a local job to a worker" says, with an edit
  brief as `model-routing` says. The brief has the model merge the
  base into the branch in the branch's worktree and resolve the
  conflict, and then stop. The brief lists `<files>` as the only
  files to edit. The model commits nothing and writes no pull request
  text.

  After the model stops, run the edit job's check script yourself, as
  `model-routing` says. The script runs `git diff --check` to catch
  leftover conflict markers, then the merge worker's test command.
  The script commits with your message only when both pass. It pushes
  nothing, so the privacy review below runs before the push. Write the
  message as `git-messages` says. The script writes a summary that
  ends in `done`. A broken resolution then never reaches the pull
  request's branch.

  When the summary ends in `done`, get the privacy review that
  `pull-request` Guardrail 5 asks for a merge commit. Push the merge
  from the branch's worktree with `git push <remote> HEAD` only when
  that review reports zero hits. Then start a new merge worker with a
  new folder. It runs the merge checks on the pushed merge and sends
  the merge request, which the first worker never sent. When the check
  script fails, or the new merge worker reports a conflict again,
  count it as a failed check. Retry and step up as `model-routing`
  says. A retry starts with `git merge --abort` in the branch's
  worktree.
- `result: not ready: ...`: bring the problems it names to the user
  with the pull request's review entry.
- Any other line, such as a declined request or an `error:` line:
  tell the user.

Close the merge worker's tab once you have read its summary.
