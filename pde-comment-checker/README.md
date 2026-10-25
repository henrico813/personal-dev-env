# pde-comment-checker

Checks the comments agents write in code against the
`code-documentation` skill's rules, and keeps the comments you approved
from being changed later. It is the only program that reads or writes
`.code-comments.json`; `pde-comment-review` asks it which comments you
must answer and has it record your answers. The skill owns each rule's
sentence, and a test fails when the checker's quote differs.

`pde-installer install` builds it with `build.py` on the full profile,
at `~/.local/bin/pde-comment-checker`, as one file that runs with
`/usr/bin/python3` 3.10 or later. The skill's `check_comments.py` and
`check_python_rules.py` run this command for agents that still use them.

## Commands

```text
pde-comment-checker --repo . [--base REF] FILE...   check comments
pde-comment-checker --repo . --approve FILE...      record approvals
pde-comment-checker version | list | changes | show JSON for pde-comment-review
pde-comment-checker apply --check < ANSWERS.json    check the owner's answers
pde-comment-checker python-rules --repo . [--base REF] FILE...
```

## Development

```sh
uv sync --locked
uv run pytest
```

Most tests run the file `build.py` packs, as pde-installer ships it;
`test_checker_interface.py` runs the package in process.
