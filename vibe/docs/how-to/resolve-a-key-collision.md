# Resolve a key collision

A message such as `slug demo-key already belongs to key Demo/key` means the
normalized slug is already claimed by a different original key in this
repository's state directory. The claim is deliberate: changing punctuation
or case must not silently select another task's branch and artifacts.

Use a distinct key whose slug does not collide. If the old key's runs are no
longer needed, stop Vibe and remove only the stale claim file:

`~/.local/state/vibe/<basename>-<16-hex-git-common-dir-hash>/<slug>/key`

Removing that file discards the ownership marker, not the run directories.
The next run with the new key reuses the existing `vibe/<slug>` branch,
`worktrees/<slug>` worktree, and slug state directory. `vibe status --key
<old-key>` then reports a stored-key conflict. Back up or remove the old run
state separately only when it is no longer needed. There is no Vibe command
that clears a claim.

If the error is instead `vibe run already active for slug ...`, wait for the
other process to finish; do not remove `run.lock` while it may still be held.
See [state layout](../reference/state-layout.md) for the directory shape.
