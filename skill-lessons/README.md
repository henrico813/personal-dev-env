# skill-lessons

A small command that shows the lessons saved in agent skills before
the `skill-fix` tool existed. A new correction goes to `skill-fix`,
which writes it into the skill's text.

The tool writes each saved lesson into the installed copy of its skill,
so any harness that loads the skill (OpenCode, Claude Code, Codex) sees
it. The lesson store stays on each machine.

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
skill-lessons list SKILL            show lessons and repeat counts
skill-lessons retire SKILL ID...    remove lessons, re-render
skill-lessons render                re-fill every marked skill
```

`list` warns at 15 lessons, so you can decide whether to move the
stable ones into the skill's text with `skill-fix`.

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

`retire` refuses a skill without the marker, and `render`
leaves such skills untouched.

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
