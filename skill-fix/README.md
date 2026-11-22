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
skill-fix sync [--repo personal|extra]
                                    push fixes, open the fix PR
skill-fix status                    show each repo's fix PR
skill-fix held [list|show|drop|retry] [N]
                                    handle fixes held on this PC
```

Each command prints one JSON object, except `held show`, which prints
the held commit. Exit codes: 0 when the command worked or the fix was
committed or held, 1 when a fix rule refused it, and 2 on an error.
The `case` rule refuses a fix that adds no correction case, a new
JSON file in the skill's `evals/` with `"correction": true`; the
session-review skill's `references/trials.md` says how to write one.

A skill's `metadata.scope` tag picks its repository: `personal` is
the checkout pde-installer runs from, and `extra` is the checkout that
`extra_skills_repo` names.

## Settings

skill-fix reads these keys in `~/.config/pde/config.json`:

| Key | Meaning |
| --- | --- |
| `extra_skills_repo` | The checkout of the extra skills repository, when this PC has one |
| `extra_terms_file` | A list of extra terms; a personal fix that holds one stays on this PC as a held fix |
| `extra_forge_repo_url` | The web address of the extra repository on a forge, such as `https://forge.example.com/owner/skills`, when its fix pull requests are there |
| `extra_forge_token_file` | The file that holds the forge API token; set it with `extra_forge_repo_url` |

Public branch names and pull request titles carry a random batch ID,
such as `fix/skill-fixes-3f9a2c-1`, never anything that names the PC.
Each clone keeps its ID in its local git config, under
`skill-fix.batchid`. An unpushed branch with an older name, such as
one from the old `skill_fix_label` setting, is renamed before its
first push.

When both forge keys are set, sync finds and opens the extra
repository's fix pull request through the forge's API at
`/api/v1/repos/OWNER/REPO/pulls`, not `gh` and `pde-gh-write`. The
token needs write access to the repository; keep its file at mode
600. skill-fix opens the pull request at once, and you merge it in the
forge's web page. `extra_forge_repo_url` must name the same repository
as the extra checkout's git remote. That remote must use HTTPS with a
git credential helper, because the update timer has no SSH agent.

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
