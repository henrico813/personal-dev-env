# skill-fix

A small command that turns your correction to an agent skill into a
commit in the skill's own repository.

When you correct an agent, the `skill-fix` skill has it edit the
skill in a fix worktree, show you the diff and, once you agree, run
`skill-fix commit`. The command checks the fix, commits it to a fix
branch and pushes that branch. The fixes made on one PC collect on
one fix pull request, which `pde-gh-write` asks you to approve. A fix
reaches every PC after that pull request merges and each PC updates.

## Install

`pde-installer install` builds it on the client and full profiles, at
`~/.local/bin/skill-fix`, and `pde-installer update` runs
`skill-fix sync` after each update. `build.py` packs the `.py` files
in `src/skill_fix/` into one file that runs with `/usr/bin/python3`,
so a machine needs only Python 3.10 or later.

Each repository's fix worktree is `~/.local/share/skill-fix/<repo>`.
It shares your checkout's `.git` but has its own branch and files, so
your checkout keeps its branch and its uncommitted work.

## Commands

```text
skill-fix --version                 print the version
skill-fix start SKILL [--reset]     get the fix worktree ready
skill-fix commit SKILL --message-file FILE
                                    check, commit and sync one fix
skill-fix sync [--repo personal|work]
                                    push fixes, open the fix PR
skill-fix status                    show each repo's fix PR
skill-fix held [list|show|drop|retry] [N]
                                    handle fixes held on this PC
```

Each command prints one JSON object, except `held show`, which prints
the held commit. Exit codes: 0 when the command worked or the fix was
committed or held, 1 when a fix rule refused it, and 2 on an error.

A skill's `metadata.scope` tag picks its repository: `personal` is
the checkout pde-installer runs from, and `work` is the checkout that
`work_skills_repo` names.

## Settings

skill-fix reads these keys in `~/.config/pde/config.json`:

| Key | Meaning |
| --- | --- |
| `skill_fix_label` | A plain word for this PC, such as `desk`, in lowercase letters and digits |
| `work_skills_repo` | The checkout of the work skills repository, when this PC has one |
| `work_terms_file` | A list of work terms; a personal fix that holds one stays on this PC as a held fix |

The label appears in public branch names and pull request titles, so
it must name no machine. Until it is set, fixes are committed but
nothing is pushed.

Sync fetches and pushes by the HTTPS form of a GitHub SSH remote,
because the update timer has no SSH agent. Git then needs a
credential helper for GitHub, such as the one `gh auth setup-git`
sets up.

## Development

```sh
uv sync --locked
uv run pytest
```

The tests build the packed file with `build.py` and run it against
temporary repositories, with stubs for `gh`, `pde-gh-write` and
`ci/scan`.
