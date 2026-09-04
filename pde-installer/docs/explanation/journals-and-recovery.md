# Journals and Recovery

Filesystem journals make user-owned activation reversible. They are stored in
`~/.local/state/pde/fs-journals/`, always under the detected `HOME`.

Before replacing a path, a manager records the destination, staged path, backup,
and path identity. Journal state is written before activation. On a normal full
run, all active journals are marked committed as a group before backup cleanup.

If a stage fails, the installer rolls back that stage and earlier journaled
stages in reverse order. On the next non-dry-run `install`, `update`, or
`config`, recovery handles journals before new work:

- an uncommitted journal restores old paths;
- a committed journal finishes removing backups and temporary paths;
- a recorded commit group finishes marking all member journals committed.

The saved profile rolls back with `config.json`. A failed expansion restores
the previous profile. Dry runs never save a profile.

Recovery refuses paths outside the same `HOME` and stops on unexpected path
identity changes. One installer lock at
`~/.local/state/pde/installer.lock` prevents concurrent mutating runs.

The boundary is important: apt package and index changes are not rolled back.
`fc-cache -f` also runs outside the filesystem journal, although a failure from
it rolls back the managed font directory.

For recovery steps, see [recover an installation](../how-to/recover-an-installation.md).
