# Personal Dev Environment - Improvement Plan

**Based on:** Code Review + Security Analysis + Implementation Review
**Date:** 2025-12-18 (Revised: 2026-01-17)
**Target:** Ubuntu/Debian x86_64 only
**Focus:** Simplicity, Maintainability, Security

---

## Executive Summary

Refactor personal-dev-env to be more maintainable, secure, and user-friendly while keeping it simple for Ubuntu/Debian x86_64 environments.

**Key Changes:**
1. Split into modular system/user playbooks (security separation) - DONE
2. Add comprehensive documentation - DONE
3. Fix variable usage inconsistencies - DONE
4. Improve version management - DONE
5. Remove unnecessary sudo operations - DONE
6. **NEW:** Fix bugs identified in implementation review

**Status:** All phases complete (1-6).

---

## Implementation Status

| Phase | Description | Status |
|-------|-------------|--------|
| 1 | Security & Structure | COMPLETE |
| 2 | Fix Variable Usage | COMPLETE |
| 3 | Refactor Task Files | COMPLETE |
| 4 | Documentation | COMPLETE |
| 5 | Update Makefile | COMPLETE |
| 6 | Bug Fixes (NEW) | COMPLETE |

---

## Phase 6: Bug Fixes (NEW)

### Goal
Address blocking issues and suggestions from implementation review.

### 6.1 Remove Legacy Task File Confusion

**Problem:** Both old (`install_shell.yml`, `install_tools.yml`, `install_deps.yml`) and new (`install_shell_user.yml`, `install_tools_user.yml`, `install_rust.yml`) task files exist, creating maintenance confusion.

**Solution:** Rename legacy files to clearly mark them as deprecated.

```bash
# Rename legacy files
mv pde/tasks/install_shell.yml pde/tasks/install_shell_legacy.yml
mv pde/tasks/install_tools.yml pde/tasks/install_tools_legacy.yml
mv pde/tasks/install_deps.yml pde/tasks/install_deps_legacy.yml

# Update main.yml to use renamed files
```

**Update main.yml references:**
```yaml
# In pde/main.yml, update task imports:
- import_tasks: tasks/install_shell_legacy.yml  # Was: install_shell.yml
- import_tasks: tasks/install_tools_legacy.yml  # Was: install_tools.yml
- include_tasks: tasks/install_deps_legacy.yml  # Was: install_deps.yml
```

### 6.2 Fix Node Version Path Idempotency Bug

**Problem:** `install_tools_user.yml:22` uses `creates: ~/.nvm/versions/node/v{{ node_version }}` but NVM creates directories like `v20.11.1`, not `v20`. This breaks idempotency - node install runs every time.

**Location:** `/home/justin/Projects/personal-dev-env/pde/tasks/install_tools_user.yml:22`

**Current:**
```yaml
- name: Install Node.js via nvm
  shell: |
    source {{ ansible_env.HOME }}/.nvm/nvm.sh
    nvm install {{ node_version }}
    nvm alias default {{ node_version }}
  args:
    executable: /bin/bash
    creates: "{{ ansible_env.HOME }}/.nvm/versions/node/v{{ node_version }}"  # BUG: v20 != v20.11.1
```

**Fixed:**
```yaml
- name: Check if Node.js is installed via nvm
  shell: |
    source {{ ansible_env.HOME }}/.nvm/nvm.sh
    nvm ls {{ node_version }} 2>/dev/null | grep -q "v{{ node_version }}"
  args:
    executable: /bin/bash
  register: node_check
  ignore_errors: yes
  changed_when: false
  when: not (skip_node | default(false))

- name: Install Node.js via nvm
  shell: |
    source {{ ansible_env.HOME }}/.nvm/nvm.sh
    nvm install {{ node_version }}
    nvm alias default {{ node_version }}
  args:
    executable: /bin/bash
  when: not (skip_node | default(false)) and (node_check.rc != 0)
```

### 6.3 Add Rust/Cargo Dependency Guard

**Problem:** `install_shell_user.yml` calls cargo-binstall without verifying Rust is installed. Fails if `--tags shell` skips rust installation.

**Location:** `/home/justin/Projects/personal-dev-env/pde/tasks/install_shell_user.yml:13,27,41`

**Add at top of install_shell_user.yml:**
```yaml
---
# Shell environment setup - user level
# REQUIRES: install_rust.yml must run before this file

- name: Verify cargo-binstall is available
  stat:
    path: "{{ ansible_env.HOME }}/.cargo/bin/cargo-binstall"
  register: cargo_binstall_check

- name: Verify cargo is available (fallback)
  stat:
    path: "{{ ansible_env.HOME }}/.cargo/bin/cargo"
  register: cargo_check

- name: Fail if Rust toolchain not installed
  fail:
    msg: "Rust toolchain not found. Run install_rust.yml first or use full playbook."
  when: not cargo_check.stat.exists
```

**Update eza/zoxide/alacritty install blocks to use guard:**
```yaml
- name: Install eza
  block:
    - name: Try eza via cargo-binstall
      shell: "{{ ansible_env.HOME }}/.cargo/bin/cargo-binstall -y --no-confirm eza"
  rescue:
    - name: Fallback to cargo install eza
      shell: "{{ ansible_env.HOME }}/.cargo/bin/cargo install eza"
  when: not eza_check.stat.exists and cargo_check.stat.exists  # Added guard
```

### 6.4 Add NVM Existence Check for npm Commands

**Problem:** Tasks source nvm.sh without checking if it exists. Fails silently if NVM installation was skipped or failed.

**Location:** `/home/justin/Projects/personal-dev-env/pde/tasks/install_tools_user.yml:88,97`

**Add NVM check before npm installs:**
```yaml
- name: Check if NVM is installed
  stat:
    path: "{{ ansible_env.HOME }}/.nvm/nvm.sh"
  register: nvm_installed

- name: Install codex via npm
  shell: |
    source {{ ansible_env.HOME }}/.nvm/nvm.sh
    npm install -g @openai/codex
  args:
    executable: /bin/bash
    creates: "{{ ansible_env.HOME }}/.npm-global/bin/codex"
  when: install_ai_tools | default(false) and nvm_installed.stat.exists  # Added guard

- name: Install Claude Code via npm
  shell: |
    source {{ ansible_env.HOME }}/.nvm/nvm.sh
    npm install -g @anthropic-ai/claude-code
  args:
    executable: /bin/bash
    creates: "{{ ansible_env.HOME }}/.npm-global/bin/claude"
  when: install_ai_tools | default(false) and nvm_installed.stat.exists  # Added guard
```

### 6.5 Remove Duplicate Variable Definitions

**Problem:** `node_version` is defined in both `vars/versions.yml:15` AND `group_vars/development.yml:43`. Per Ansible precedence, group_vars wins, making versions.yml entry ignored.

**Location:** `/home/justin/Projects/personal-dev-env/pde/group_vars/development.yml:43`

**Fix:** Remove duplicate from group_vars/development.yml:
```yaml
# DELETE this line from group_vars/development.yml
# node_version: "20"  # Already defined in vars/versions.yml
```

### 6.6 Add Missing Default Variables

**Problem:** `install_lazyvim` defaults to `true` in task `when` clauses but isn't defined in `defaults.yml`, making it non-discoverable.

**Location:** `/home/justin/Projects/personal-dev-env/pde/defaults.yml`

**Add to defaults.yml:**
```yaml
# Tool Installation Flags
# These control which optional tools are installed
install_lazyvim: true      # LazyVim starter config for Neovim
install_yazi: true         # Yazi terminal file manager
install_alacritty: false   # Alacritty terminal (requires build deps)
```

### 6.7 Fix Redundant Package Lists

**Problem:** `group_vars/servers.yml:16-17` lists `zsh` and `tmux` in `shell_tools`, but `system.yml:40` already installs `zsh` unconditionally. This creates confusion about where packages come from.

**Location:** `/home/justin/Projects/personal-dev-env/pde/group_vars/servers.yml:16-22`

**Current:**
```yaml
shell_tools:
  - zsh      # Already installed by system.yml
  - tmux     # Should be in system.yml
  - fd-find
  - fzf
  - ripgrep
  - bat
  - jq
```

**Fixed:**
```yaml
# Note: zsh is always installed by system.yml
# tmux is built from source by system.yml for modern features
shell_tools:
  - fd-find
  - fzf
  - ripgrep
  - bat
  - jq
```

### 6.8 Add Deprecation Timeline to main.yml

**Problem:** `main.yml:3` says "will be removed in a future release" but doesn't specify when.

**Location:** `/home/justin/Projects/personal-dev-env/pde/main.yml:1-8`

**Update header:**
```yaml
# DEPRECATED: Use site.yml instead
# This file is maintained for backwards compatibility only.
# REMOVAL DATE: After 2026-06-01 or v2.0 release (whichever comes first)
#
# Migration:
#   Old: ansible-playbook main.yml -e "user=myuser"
#   New: ansible-playbook site.yml --ask-become-pass
```

### 6.9 Add Pre-task System Prerequisite Check (Optional)

**Problem:** Running `site.yml --tags user` skips system prerequisites (git, zsh, curl). User tasks may fail unexpectedly.

**Location:** `/home/justin/Projects/personal-dev-env/pde/user.yml`

**Add pre_tasks section:**
```yaml
- name: User-level setup
  hosts: all
  become: no

  pre_tasks:
    - name: Verify system prerequisites are installed
      command: which {{ item }}
      register: prereq_check
      failed_when: false
      changed_when: false
      loop:
        - git
        - zsh
        - curl
        - unzip
      tags: [always]

    - name: Warn if system prerequisites missing
      debug:
        msg: "WARNING: {{ item.item }} not found. Run system.yml first for full functionality."
      when: item.rc != 0
      loop: "{{ prereq_check.results }}"
      loop_control:
        label: "{{ item.item }}"
      tags: [always]
```

---

## Updated Implementation Checklist

### Phase 1-5: Original Plan (COMPLETE)
- [x] Create `pde/system.yml`
- [x] Create `pde/user.yml`
- [x] Create `pde/site.yml` as orchestrator
- [x] Create `pde/defaults.yml`
- [x] Create `pde/vars/versions.yml`
- [x] Guard sudoers modification task
- [x] Fix node version usage
- [x] Use version variables throughout
- [x] Rename `install_deps.yml` → `install_rust.yml`
- [x] Create user-level task files
- [x] Create `install_configs.yml`
- [x] Create comprehensive `README.md`
- [x] Update Makefile with deployment targets

### Phase 6: Bug Fixes (COMPLETE)
- [x] Rename legacy task files (`*_legacy.yml`)
- [x] Update main.yml to reference renamed files
- [x] Fix node version idempotency check
- [x] Add Rust/cargo dependency guard to install_shell_user.yml
- [x] Add NVM existence check for npm install tasks
- [x] Remove duplicate `node_version` from group_vars/development.yml
- [x] Add `install_lazyvim`, `install_yazi`, `install_alacritty` to defaults.yml
- [x] Remove redundant zsh/tmux from servers.yml shell_tools
- [x] Add deprecation timeline to main.yml header
- [x] Add pre-task system prerequisite warnings to user.yml

---

## Updated Success Criteria

- [x] Can run full install with: `make deploy-dev`
- [x] Can update user tools with: `ansible-playbook user.yml` (no sudo)
- [x] Clear separation: system.yml vs user.yml
- [x] No hardcoded values (all in defaults.yml or versions.yml)
- [x] Comprehensive README exists
- [x] All tasks have clear, consistent names
- [x] Passwordless sudo guarded by flag (default: false)
- [x] **NEW:** No duplicate variable definitions across files
- [x] **NEW:** All task files clearly named (legacy vs current)
- [x] **NEW:** Idempotent node installation
- [x] **NEW:** Dependency guards prevent partial installation failures
- [x] **NEW:** Clear deprecation timeline for legacy files

---

## Testing Plan for Phase 6

1. **Test legacy file rename:**
   ```bash
   ansible-playbook main.yml -e "user=$USER" --check
   # Should reference *_legacy.yml files without error
   ```

2. **Test node idempotency:**
   ```bash
   ansible-playbook user.yml --tags tools
   # Run twice - second run should show no changes for node
   ```

3. **Test dependency guards:**
   ```bash
   # Without rust installed, should fail gracefully
   ansible-playbook user.yml --tags shell --skip-tags rust
   # Should show clear error about missing cargo
   ```

4. **Test npm guards:**
   ```bash
   # Without nvm installed, should skip npm packages
   ansible-playbook user.yml --tags tools -e "skip_node=true"
   # Should skip codex/claude install, not fail
   ```

5. **Test prerequisite warnings:**
   ```bash
   # On fresh system without git/zsh
   ansible-playbook user.yml --tags user
   # Should show warnings but continue
   ```

---

## Files to Modify (Phase 6)

| File | Change |
|------|--------|
| `tasks/install_shell.yml` | Rename to `install_shell_legacy.yml` |
| `tasks/install_tools.yml` | Rename to `install_tools_legacy.yml` |
| `tasks/install_deps.yml` | Rename to `install_deps_legacy.yml` |
| `main.yml` | Update imports + add deprecation date |
| `tasks/install_tools_user.yml` | Fix node check + add NVM guards |
| `tasks/install_shell_user.yml` | Add cargo dependency guard |
| `group_vars/development.yml` | Remove duplicate node_version |
| `group_vars/servers.yml` | Remove redundant zsh/tmux |
| `defaults.yml` | Add install_lazyvim, install_yazi, install_alacritty |
| `user.yml` | Add pre_tasks prerequisite check |

---

## Notes

- Phase 1-5 implemented privilege separation successfully
- Phase 6 addresses edge cases and improves robustness
- Legacy main.yml preserved for backwards compatibility until 2026-06-01
- All changes are backwards compatible - no breaking changes
- Focus remains on Ubuntu/Debian x86_64 only
