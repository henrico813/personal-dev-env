# personal-dev-env

This repository is public. Keep names, paths, hosts and addresses from
your machines and work out of every file, commit and pull request.

- Before each push, run `ci/scan commits <remote>/main HEAD`.
- Before each `pde-gh-write pr create` or `pr edit`, run
  `ci/scan text FILE` on the title and description file.
- On a finding, replace the text and scan again. Never search for or
  print the private names the scan found.

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
