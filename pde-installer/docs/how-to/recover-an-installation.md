# Recover an Installation

The installer normally recovers itself after an interruption.

1. Stop any other `pde-installer` process.
2. Rerun the failed mutating command:

   ```bash
   pde-installer install
   ```

   Use `update` or `config` instead if that was the failed command.
3. Run checks:

   ```bash
   pde-installer doctor
   pde-installer list
   ```

Before a non-dry-run mutating command starts work, it takes an installer lock
and processes unfinished journals in
`~/.local/state/pde/fs-journals/`. Uncommitted journals are rolled back;
committed journals finish cleanup. The command stops if recovery fails.

Do not delete journal files or backup paths by hand. They hold the information
needed to restore earlier files.

Recovery covers journaled changes under `HOME`. It does not undo packages or
package-index changes made by `sudo apt-get`. A font-cache refresh by `fc-cache`
also happens outside rollback.

See [journals and recovery](../explanation/journals-and-recovery.md).
