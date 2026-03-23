# Personal Dev Environment

Ansible-based setup for Ubuntu/Debian development environments.

## Architecture

The playbook uses privilege separation:

- **system.yml** - System-level tasks requiring sudo (apt packages, locale, neovim)
- **user.yml** - User-level tasks without sudo (rust, shell config, dev tools)
- **site.yml** - Orchestrator that imports both

## Common Issues

### Package Availability

Some tools are not available via apt on all Ubuntu versions and must be installed via cargo-binstall instead:

| Tool | Ubuntu 24.04+ | Ubuntu 22.04 | Solution |
|------|---------------|--------------|----------|
| zoxide | apt | Not available | cargo-binstall |
| eza | Not available | Not available | cargo-binstall |
| bat | apt (as `bat`) | apt (as `bat`) | apt |
| fd | apt (as `fd-find`) | apt (as `fd-find`) | apt |
| ripgrep | apt | apt | apt |

**Rule:** If a tool isn't universally available via apt across supported Ubuntu versions, install it via cargo-binstall in user-level tasks (`install_shell_user.yml`), not in system-level apt tasks.

### Variable Override Hierarchy

Variables are resolved in this order (later overrides earlier):

1. `defaults.yml` - Base defaults
2. `vars/versions.yml` - Version pinning
3. `group_vars/<profile>.yml` - Profile-specific (development, servers, minimal)
4. Command-line `--extra-vars`

**Gotcha:** If you fix a default in `system.yml` but `group_vars/development.yml` has an explicit list, the group_vars wins. Always check group_vars when debugging package issues.

## Deployment

```bash
# Full dev environment on localhost
make deploy-dev

# Server profile (shell QoL only) on remote
make deploy-server HOST=hostname

# Just run user-level tasks (no sudo needed)
ansible-playbook user.yml
```

## Adding New Tools

1. If available via apt on Ubuntu 22.04+: Add to `shell_tools` list in `system.yml`
2. If not universally available: Add cargo-binstall task in `tasks/install_shell_user.yml`
3. Update `group_vars/*.yml` if they override the default lists
