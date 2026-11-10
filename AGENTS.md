# personal-dev-env

This repository is public. Keep names, paths, hosts and addresses from
your machines and work out of every file, commit and pull request.

- Before each push, run `scripts/privacy-scan commits <remote>/main HEAD`.
- Before each `pde-gh-write pr create` or `pr edit`, run
  `scripts/privacy-scan text FILE` on the title and description file.
- On a finding, replace the text and scan again. Never search for or
  print the private names the scan found.
