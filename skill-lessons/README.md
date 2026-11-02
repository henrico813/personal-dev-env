# skill-lessons

A small command that keeps your corrections inside agent skills.

A skill that opts in (see below) tells the agent to save your
correction with `skill-lessons add` when you say "remember". The tool
writes the lesson into the installed copy of the skill, so any harness
that loads the skill (OpenCode, Claude Code, Codex) sees it next time.
The lesson store stays on each machine. A rule reaches a repository
only through a promotion you approve.

## Install

`pde-installer install` builds and installs it on the full profile, at
`~/.local/bin/skill-lessons`, and runs `skill-lessons render` after
chezmoi copies the skills. `build.py` packs the `.py` files in
`src/skill_lessons/` into one file that runs with `/usr/bin/python3`, so
a machine needs only Python 3.10 or later.

| Location | Default | Override |
| --- | --- | --- |
| Installed skills | `~/.agents/skills/<skill>/SKILL.md` | `SKILL_LESSONS_SKILLS_DIR` |
| Lesson store, one JSON file per skill | `~/.local/share/skill-lessons/` | `SKILL_LESSONS_HOME` |

## Commands

```text
skill-lessons --version             print the version
skill-lessons add FILE              save a lesson, re-render its skill
skill-lessons list SKILL            show lessons and repeat counts
skill-lessons retire SKILL ID... [--after-push --checkout PATH]
                                    remove lessons, re-render
skill-lessons render                re-fill every marked skill
skill-lessons promote-start SKILL --checkout PATH
skill-lessons promote-check SKILL --checkout PATH [--growth-reason TEXT]
```

`FILE` holds a JSON object; `-` reads it from standard input. `skill` is
required, and `text` is required unless `repeat_of` is set. `project`,
`epic` and `repo` set an optional scope. Unknown keys are refused, and
so is text that says "this epic" with no `epic` set. New text is also
refused when it names a PR or issue number such as `#12` or `PR 12`, a
ticket key such as `ABC-12` or a file path such as `~/notes` or
`ai/demo/SKILL.md`, or runs over 60 words. Names that start with
`UTF-`, `SHA-`, `ISO-`, `RFC-` or `IEEE-`, such as `UTF-8`, and paths
under `/dev/`, `/tmp` and `/usr/` or starting with `$VAR/` are allowed.
A repeat that sends the saved text unchanged is not checked.

```json
{"skill": "demo", "epic": "ABC-12", "text": "Children are Tasks; match the siblings' type."}
```

When you correct something already saved, the agent sets `repeat_of`
to the lesson's ID. That lesson's repeat count goes up by one, and any
`text` or scope given replaces the old value. `list` warns at 15
lessons, so you can decide whether to fold stable ones into the skill's
source.

Exit codes: 0 on success, 1 when an operation is refused or fails, 2 on
bad arguments.

## How a skill opts in

A skill takes lessons only if its `SKILL.md` has a `## Lessons` heading
followed directly by the marker line:

```markdown
## Lessons
<!-- skill-lessons: filled from the local store; do not edit -->
```

Keep the section empty in the source. Rendering replaces everything
between the marker and the next `## ` heading, or the end of the file:

```markdown
## Lessons
<!-- skill-lessons: filled from the local store; do not edit -->
- (epic ABC-12) Children are Tasks; match the siblings' type. [L1]
- Don't call a bug cosmetic unless only its look is wrong. [L2]
```

`add` and `retire` refuse a skill without the marker, and `render`
leaves such skills untouched.

## Promotion checks

These commands expect skills at `skills/<name>/SKILL.md` in the
checkout, so they cannot yet promote into this repository, which uses
`ai/skills/`.

`promote-start` and `promote-check` help fold lessons into a skill's
source. `promote-start` refuses a dirty checkout, a source without the
marker, or a `user.email` that differs from the last commit's author (so
a promotion is not committed under another identity, such as a work
address), then lists the lessons. It marks a lesson
`(likely stays a lesson)` when it has an `epic` scope or says "this
epic" or "this ticket", since such a lesson rarely becomes skill text.
`promote-check` fails when the added lines hold a ticket key or copy
five words in a row from a lesson, when the source's Lessons section is
not empty, or when the skill grew without `--growth-reason`; when it
passes, it prints the diff and line counts.
`retire --after-push --checkout PATH` refuses unless the checkout's HEAD
is on its upstream branch (`origin/main` when it has none), so a rule is
never only in an unpushed commit.

## Development

```sh
uv sync --locked
uv run pytest
```

Every regular `.py` file in `src/skill_lessons/` is packed, tracked by
Git or not; links are skipped. The tests build the packed file with
`build.py` and run it, so they check what pde-installer ships. To try a
build by hand, write it to a scratch path:

```sh
python3 build.py /tmp/skill-lessons
/tmp/skill-lessons --version
```
