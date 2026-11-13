# Evaluation

Four cases check whether the skill changes what an agent does with a
request for a new skill or tool. Run each case twice on the same
model, each time in a fresh session: once with no skill, and once with
SKILL.md in the prompt. Score each reply against the case's checks,
one point each.

## How to run

Run from a scratch home, a folder from `mktemp -d`, so the agent sees
no installed skills or rules. Set `HOME` to it and `XDG_CONFIG_HOME`
to its `.config` folder. Copy only the local model's provider entry
from your own `opencode.json` into `.config/opencode/opencode.json`
there. OpenCode reads its logins from the data folder. Choose one
of these two ways to log in:

- Set `XDG_DATA_HOME` to the full path of your own `~/.local/share`,
  and `XDG_STATE_HOME` to the full path of your own `~/.local/state`.
- Point both at folders inside the scratch home. Then set the
  provider entry's `apiKey` to `{file:<path>}`, where `<path>` is
  your key file.

Save these two lines as `prompt.txt`:

```text
You are a coding agent that starts other agents for a user.
Reply with only what you would do next, in order.
```

Save each case's text block below as `caseN.txt`, next to
`prompt.txt` and a copy of this skill's SKILL.md. Make an empty
folder beside them and run from it, so the run with no skill cannot
read SKILL.md. The commands read the files from the folder above as
`../prompt.txt`. Run each case without the skill and with it, using
the local model that `model-routing` names, in place of
`<local model>`:

```bash
m=<local model>
p=../prompt.txt
opencode run --standalone -m $m "$(cat $p ../case1.txt)"
opencode run --standalone -m $m "$(cat $p ../SKILL.md ../case1.txt)"
```

Repeat with `case2.txt` to `case4.txt`. Read each reply and score its
points by hand. The skill works when the run with it scores higher on
cases 1 to 3 and gets every point on case 4.

## Case 1: a new skill from the start

```text
The user says: "Make a skill that lets agents read their own terminal
screen and check a change at phone size. Same process as last time:
plan it, build it on the local model, test it for real."
The repository is a checkout of the user's skills.
```

Checks:

1. A planner on the strongest model writes the plan with
   `create-plan`.
2. The plan has a prod-test step: install from the branch, run on one
   real task, record what was and was not checked.
3. Fresh agents review the plan before the user sees it.
4. The user reviews the plan before any build starts.
5. The build uses `implement-plan`, with steps on the local model.

## Case 2: unit tests pass, the timer has not fired

```text
The approved plan adds a nightly job on a systemd user timer that
fires at 03:00. The build finished on the local model and every unit
test passes. It is 15:00. You are about to open the pull request.
```

Checks:

1. The agent does not open the pull request on unit tests alone.
2. It installs from the branch on this machine and forces one run of
   the job, instead of waiting for 03:00.
3. The pull request's Testing section gets one line on what ran and
   what happened.
4. The reply lists what the forced run did not check.

## Case 3: the reviews of a skill pull request

```text
The build added one new skill folder and changed a Python script that
another skill uses. The prod test passed. You are about to open the
pull request.
```

Checks:

1. A fresh agent reviews the diff with `review-plan`.
2. Fresh agents review the Python change under `code-documentation`
   and `behavior-focused-testing`.
3. The new skill gets the reviews in "Review a skill change" of
   `skill-development`, each by its own fresh agent.
4. The agent that built the change reviews none of it.
5. The final report names each kind of review that ran.

## Case 4: not this path

```text
The user says: "Build the remaining three pull requests of the
approved sync roadmap overnight. I will review them in the morning."
```

Checks:

1. The agent picks `auto-run`, not this skill.
2. It does not ask the user to review each plan before its build.
