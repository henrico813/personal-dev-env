# Skill-lesson review

Read this page when the user picks "Skill-lesson review of <skill>"
from conclude-session's pick list. The review reads what the sessions
that used the skill taught it, and judges each open pull request that
changes the skill.

Each job goes to the cheapest model that does it well. Scripts do the
reading and checking, and a local model sorts. This session judges
only what is left. Never hand a job to Opus or another top model
unless the user asks.

Work in a folder from `mktemp -d`. The commands use these names:

```bash
S=<skill>
R=~/.agents/skills/session-review/scripts/skill_review.py
```

## Steps

1. Print the user's messages from each session that loaded the skill
   since its last review. For a skill never reviewed, counting starts
   when the script first ran. Each message is one line of at most 200
   characters:

   ```bash
   python3 "$R" corrections "$S" > corrections.txt
   ```

2. A local model, `goog/qwen3.8`, sorts the messages. Run it in the
   folder with this brief, after you replace `<tests>` with the first
   two tests in step 2 of [skill-lessons](../../skill-lessons/SKILL.md):

   ```bash
   opencode run --standalone -m goog/qwen3.8 "<brief>"
   ```

   The brief:

       Read corrections.txt. Keep each message that teaches a rule
       that passes these tests: <tests>. Write candidates.txt with
       one line per rule, '<session> <seq>: <rule>', or the single
       line 'none'. Touch no other file and run nothing else. Never
       answer an approval request.

   Then check the file:

   ```bash
   python3 "$R" check "$S" candidates.txt
   ```

   After two failed runs of the check, give the job to the cheapest
   cloud model.
3. In the personal-dev-env checkout, list the open pull requests that
   change the skill, and save each one's diff:

   ```bash
   gh pr list --state open --limit 100 --json number,title,files |
     jq -r --arg d "ai/skills/$S/" '.[]
       | select(any(.files[]; .path | startswith($d)))
       | "\(.number) \(.title)"'
   gh pr diff <number> > pr-<number>.diff
   ```

4. Judge the candidates and pull requests yourself. Judging is a
   decision, so this session does it on its own model. Load the
   `skill-development` skill, and read the skill, `candidates.txt`
   and each diff. These items form the pick list of step 5:
   - Each candidate worth keeping, as a lesson or as a change to the
     skill's text, with a one-line reason.
   - For each pull request, one of change (push a new commit to its
     branch), build on (stack a new pull request on it) or close,
     with a one-line reason, judged against skill-development.
5. Send the user those items as one numbered pick list, as
   conclude-session's step 2 does. Then wait.
6. Do only what the user picked:
   - Save a picked lesson by step 4 of skill-lessons.
   - Write a picked change to the skill by skill-development. Open it
     with the [pull-request](../../pull-request/SKILL.md) skill as a
     new pull request, or, for build on, stacked on that pull
     request's branch.
   - Change or close an open pull request only when the user picked
     it, with the pull-request skill.
7. Once the list is sent, record the review, whether or not the user
   picked anything:

   ```bash
   python3 "$R" mark "$S"
   ```

A review started from conclude-session then returns to its next
Closing step.
