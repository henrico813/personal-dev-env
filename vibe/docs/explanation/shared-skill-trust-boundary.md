# Shared-skill trust boundary

When the host `~/.agents/skills` directory exists, Vibe automatically mounts
that host directory read-only at `/vibe-home/.agents/skills`. The directory is
trusted executor input even though the bind mount is read-only: its files can
instruct Pi, and Vibe does not review their contents.

Pi is started with `--no-skills`, which disables normal discovery, and with an
explicit `--skill /vibe-home/.agents/skills` selection when the mount exists.
This keeps project-local discovery from shadowing the reviewed host directory.
The automatic directory mount is the shared-skill mechanism.

Vibe allows a missing directory. When the path exists, setup or launch-time
validation rejects a non-directory, a symlinked path or symlinked ancestry, an
unreadable directory, a path whose device/inode changes during validation, and
paths unsafe for Docker mount syntax. It also rejects overlap with writable
Docker mounts. A missing directory during the later validation is treated as
optional.

Validation happens before Docker launch and is repeated immediately before the
bind arguments are built. These checks narrow replacement risks, but they are
not a complete race-prevention guarantee: an interval remains between final
validation and Docker resolving the bind source.
