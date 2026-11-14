# personal-dev-env

This repository is public. Keep names, paths, hosts and addresses from
your machines and work out of every file, commit and pull request.

- Before each push, run `ci/scan commits <remote>/main HEAD`.
- Before each `pde-gh-write pr create` or `pr edit`, run
  `ci/scan text FILE` on the title and description file.
- On a finding, replace the text and scan again. Never search for or
  print the private names the scan found.
- Name a commit's scope, the part in parentheses after its type, for
  the app whose config it changes, such as
  `fix(i3): ...` for a change under `chezmoi/dot_config/i3/`. Use the
  `chezmoi` scope only for chezmoi itself or its templates.

## New skills

The installer copies every file Git tracks in `ai/skills/` to a full
install. A new skill there also needs these entries:

- A row in the installed-layout table in `README.md`, and its name in
  the skill list in `ai/README.md`.
- A row in `ai/evals/skill-routing.md`.
- When a client laptop needs it, an entry in
  `chezmoi/.chezmoiexternal.toml.tmpl`.
- Otherwise, a line in `chezmoi/.chezmoiremove` and in the remove list
  of `pde-installer/internal/chezmoi/template_render_test.go`, so a
  laptop switched to a client install deletes it.

A new `<language>-development` skill has the same kinds of files as
`go-development` and `rust-development`: references, examples and
`references/evaluation.md`. Like them, it sums up or quotes its sources
instead of only linking them.

## Agent tools

- Build an agent tool as a command-line program, or an MCP server,
  that any agent harness can run. Never build it as a plugin for one
  harness. Turn the tool on only for the skills that use it, not
  for every skill.
- Design approval and notification screens for the phone first.
  Fit Herdr's popup, 40 columns by 16 lines, and the cards of the
  Moshi phone app, about 37 columns wide. Put the lines that identify
  the request first.
- When a tool is retired, delete it and name only the tool that
  replaced it. Never warn readers about the old one.
- Run scratch `opencode` commands as `opencode run --standalone`, with
  `XDG_CONFIG_HOME` set to a scratch folder. A command without
  `--standalone`, such as `opencode models`, reaches the user's
  running OpenCode service. Leave `XDG_DATA_HOME` alone, since it
  holds the credentials.
