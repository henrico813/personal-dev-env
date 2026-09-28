# Shared-skill trust boundary

Vibe accepts two explicit skill roots: host `~/.agents/skills` and the managed
worktree's `.agents/skills`. Both are trusted executor input even though their
bind mounts are read-only: their files can instruct Pi, and Vibe does not review
their contents. Repository skills come from the exact checked-out revision.

Pi is started with `--no-skills`, which disables normal discovery, and one
explicit `--skill` selection per existing root. User skills are selected first;
repository skills are selected second. No other project-local skills are
discovered implicitly.

The user root is mounted at `/vibe-home/.agents/skills`. The repository root is
mounted over its path inside the writable managed worktree. This more-specific
read-only bind prevents the worker from changing the same host files through the
parent worktree mount.

Vibe allows either directory to be missing. When a path exists, setup or launch-time
validation rejects a non-directory, a symlinked path or symlinked ancestry, an
unreadable directory, a path whose device/inode changes during validation, and
paths unsafe for Docker mount syntax. User skills cannot overlap writable Docker
mounts. Repository skills may overlap only the managed worktree at their exact
expected subtree, which receives the nested read-only mount. Repository skills
must contain files tracked by `HEAD`, must have no tracked changes, untracked or
ignored contents, and cannot contain descendant symlinks or multiply-linked
files. Index flags that hide worktree changes are rejected. A missing directory
during the later validation is treated as optional.

Validation happens before Docker launch and is repeated immediately before the
bind arguments are built. These checks narrow replacement risks, but they are
not a complete race-prevention guarantee: an interval remains between final
validation and Docker resolving the bind source.
