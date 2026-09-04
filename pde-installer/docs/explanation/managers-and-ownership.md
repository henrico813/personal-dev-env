# Managers and Ownership

Each inventory item has one owner:

| Owner | Responsibility |
|---|---|
| `ubuntu` | Missing host packages installed with apt |
| `aqua` | Aqua and standalone binaries |
| `direct` | tmux, exact runtimes, unsupported layouts, Keychain, and fonts |
| `npm` | npm-native command-line tools |
| `local` | Applications built from this repository and `blink.cmp` |
| `chezmoi` | Home configuration and checksummed external content |

Ownership prevents two backends from naming the same item. The manifest
validation rejects duplicate names and missing versions for owners that require
pins.

The manifest feeds validation and `list`. It is inventory, not an uninstall
description. Removing an entry does not provide a general uninstall operation.

Managers stage replacement content and use journals for activation where
possible. Apt changes are system-wide and are not journaled. See
[component metadata](../reference/component-metadata.md).
