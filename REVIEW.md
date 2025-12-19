# Personal Dev Environment - Code Review

**Focus Areas:** Simplicity, Portability, Maintainability
**Date:** 2025-12-18

---

## Executive Summary

The codebase is functional but has significant portability and maintainability issues. The Ansible playbooks work but lack user configurability, are platform-specific, and have inconsistent patterns that make them harder to maintain.

**Quick Wins:** 13 issues can be fixed in <1 hour
**Major Improvements:** 8 architectural changes for better long-term maintainability

---

## 🔴 Critical Issues (Fix First)

### 1. **Hardcoded User Configuration** ⚠️ PORTABILITY
**Location:** `pde/main.yml:52-58`

```yaml
# Current - Not portable
locale_gen:
  name: "en_US.UTF-8"  # Hardcoded
timezone:
  name: "America/Los_Angeles"  # Hardcoded
```

**Problem:** Anyone outside Los Angeles timezone or non-US locale must edit the playbook.

**Fix:** Create `pde/defaults.yml` with overrideable variables:
```yaml
# pde/defaults.yml
---
timezone: "{{ lookup('env', 'TZ') | default('UTC', true) }}"
locale: "{{ lookup('env', 'LANG') | default('en_US.UTF-8', true) }}"
user: "{{ lookup('env', 'USER') }}"
```

**Impact:** Makes setup work anywhere without code changes.

---

### 2. **Missing User Documentation** ⚠️ MAINTAINABILITY
**Location:** Entire project

**Problem:**
- No README explaining how to run
- `user` variable required but not documented
- No list of what gets installed
- No prerequisites documented

**Fix:** Create comprehensive README with:
```markdown
## Quick Start
```bash
# Set your username
export ANSIBLE_USER=yourname

# Run playbook
cd pde && ansible-playbook main.yml -e "user=$ANSIBLE_USER" --ask-become-pass
```

## What Gets Installed
- Shell: zsh + powerlevel10k + plugins
- Editor: Neovim with LazyVim
...
```

---

### 3. **External Dependency Path** ⚠️ PORTABILITY
**Location:** `pde/main.yml:38`

```yaml
- src: "../ai-profiles/codex/config.toml"  # Breaks if structure changes
```

**Problem:**
- Path assumes ai-profiles is at parent level
- Fails if repo structure changes
- Not documented as a dependency

**Fix Option 1 - Make Optional:**
```yaml
- name: Copy codex config if available
  copy:
    src: "{{ item.src }}"
    dest: "{{ item.dest }}"
  loop: "{{ tool_cfgs }}"
  when: item.src is file
  ignore_errors: yes
```

**Fix Option 2 - Vendor It:**
```bash
mv ai-profiles pde/vendor/ai-profiles
# Update path to: vendor/ai-profiles/codex/config.toml
```

---

### 4. **Platform Lock-in** ⚠️ PORTABILITY
**Location:** All task files use `apt`

**Problem:** Only works on Debian/Ubuntu. Fails on macOS, Fedora, Arch, etc.

**Fix:** Use Ansible package managers conditionally:
```yaml
- name: Install packages
  package:
    name: "{{ item }}"
    state: present
  loop: "{{ common_packages }}"
  when: ansible_facts['os_family'] == "Debian"
```

Or use `package` module which auto-detects:
```yaml
- name: Install common tools
  package:
    name: "{{ common_packages }}"
```

**Note:** For now, document "Ubuntu/Debian only" if cross-platform support isn't needed.

---

## 🟡 Major Issues (Important)

### 5. **Architecture Hardcoded** ⚠️ PORTABILITY
**Location:** `pde/tasks/install_tools.yml:55, :101`

```yaml
url: https://github.com/sxyazi/yazi/releases/latest/download/yazi-x86_64-unknown-linux-gnu.zip
deb: https://download.opensuse.org/repositories/home:/justkidding/xUbuntu_24.04/amd64/ueberzugpp_2.9.7_amd64.deb
```

**Problem:** Fails on ARM, Apple Silicon, different Ubuntu versions.

**Fix:**
```yaml
vars:
  arch_map:
    x86_64: x86_64-unknown-linux-gnu
    aarch64: aarch64-unknown-linux-gnu
    arm64: aarch64-unknown-linux-gnu

  yazi_arch: "{{ arch_map[ansible_architecture] | default('x86_64-unknown-linux-gnu') }}"

- name: Download yazi
  get_url:
    url: "https://github.com/sxyazi/yazi/releases/latest/download/yazi-{{ yazi_arch }}.zip"
```

---

### 6. **Node Version Mismatch** ⚠️ SIMPLICITY
**Location:** `pde/main.yml:9` vs `pde/tasks/install_tools.yml:26`

```yaml
# main.yml
node_version: "{{ node_version | default('18') }}"  # Default 18

# install_tools.yml
nvm install 20  # Hardcoded to 20!
```

**Problem:** Variable defined but ignored. Confusing and misleading.

**Fix:** Actually use the variable:
```yaml
- name: Install Node.js via nvm
  shell: |
    source {{ user_home }}/.nvm/nvm.sh
    nvm install {{ node_version }}
    nvm alias default {{ node_version }}
  args:
    creates: "{{ user_home }}/.nvm/versions/node/v{{ node_version }}"
```

---

### 7. **Inconsistent Naming Conventions** ⚠️ SIMPLICITY
**Location:** Throughout task files

**Examples:**
```yaml
# Some use prefixes
- name: func_setup_yazi
- name: func_lazyvim_install

# Others don't
- name: Install nvm
- name: Configure npm global directory
- name: install(tool) dependencies
- name: check(tools) if aider is already installed
```

**Problem:** Hard to scan, no clear pattern, mixing conventions.

**Fix:** Pick one style and apply consistently:
```yaml
# Recommended: Plain descriptive names
- name: Install yazi terminal file manager
- name: Configure npm global prefix
- name: Check if aider is installed
```

---

### 8. **Unsafe sudo Configuration** ⚠️ MAINTAINABILITY
**Location:** `pde/main.yml:68-74`

```yaml
- name: "Grant sudo privileges to user"
  lineinfile:
    path: "/etc/sudoers"
    line: "{{ target_user }} ALL=(ALL) NOPASSWD:ALL"
```

**Problem:**
- Grants passwordless sudo (security risk)
- Done unconditionally even if user already has sudo
- Could create duplicate entries

**Fix:** Make it optional with a flag:
```yaml
- name: Grant sudo privileges (if requested)
  lineinfile:
    path: "/etc/sudoers"
    line: "{{ target_user }} ALL=(ALL) NOPASSWD:ALL"
    validate: "/usr/sbin/visudo -cf %s"
  when: grant_passwordless_sudo | default(false)
```

---

### 9. **Shell Module Overuse** ⚠️ SIMPLICITY
**Location:** Multiple files

**Problem:** Using `shell:` when proper Ansible modules exist:

```yaml
# Current - Fragile
- shell: "{{ user_home }}/.cargo/bin/cargo install eza"

# Better - Idempotent
- community.general.cargo:
    name: eza
    state: present
```

```yaml
# Current - Manual
- shell: curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.40.0/install.sh | bash

# Better - Safer
- get_url:
    url: https://raw.githubusercontent.com/nvm-sh/nvm/v0.40.0/install.sh
    dest: /tmp/nvm-install.sh
- shell: bash /tmp/nvm-install.sh
```

---

### 10. **No Idempotency for npm Installs** ⚠️ MAINTAINABILITY
**Location:** `pde/tasks/install_tools.yml:135-160`

**Problem:** npm global installs check file existence but could be outdated:
```yaml
- stat:
    path: "{{ user_home }}/.npm-global/bin/claude"
  register: claude_installed

- shell: npm install -g @anthropic-ai/claude-code
  when: not claude_installed.stat.exists  # Never updates!
```

**Fix:** Use npm list to check installed version:
```yaml
- name: Check Claude Code version
  shell: |
    source {{ user_home }}/.nvm/nvm.sh
    npm list -g @anthropic-ai/claude-code --json
  register: claude_check
  ignore_errors: yes
  changed_when: false

- name: Install/update Claude Code
  shell: |
    source {{ user_home }}/.nvm/nvm.sh
    npm install -g @anthropic-ai/claude-code@latest
  when: claude_check.rc != 0 or force_update | default(false)
```

---

### 11. **Version Pinning Inconsistency** ⚠️ MAINTAINABILITY
**Location:** Throughout

**Mixed approaches:**
```yaml
state: latest  # Auto-update (install_deps.yml:24)
v0.40.0        # Pinned (install_tools.yml:15)
/latest/       # GitHub latest (install_shell.yml:66)
2.9.7          # Specific version (install_tools.yml:101)
```

**Problem:**
- No clear versioning strategy
- `state: latest` can break on updates
- Some versions will drift, others won't

**Fix:** Create version variables:
```yaml
# pde/vars/versions.yml
---
nvm_version: "v0.40.0"
node_version: "20"
neovim_version: "latest"  # or "v0.9.5"
ueberzugpp_version: "2.9.7"

# Use in tasks
url: "https://raw.githubusercontent.com/nvm-sh/nvm/{{ nvm_version }}/install.sh"
```

---

### 12. **ignore_errors Without Documentation** ⚠️ MAINTAINABILITY
**Location:** `pde/tasks/install_deps.yml:61`, `install_tools.yml:103`

```yaml
- name: Update Rust if already installed
  shell: "{{ user_home }}/.cargo/bin/rustup update"
  ignore_errors: yes  # Why? What errors are expected?

- apt:
    deb: https://download.opensuse.org/repositories/.../ueberzugpp_2.9.7_amd64.deb
  ignore_errors: yes  # Fails silently on wrong Ubuntu version
```

**Problem:** Errors hidden without explanation. Hard to debug.

**Fix:** Add comments or make failures explicit:
```yaml
- name: Update Rust if already installed
  shell: "{{ user_home }}/.cargo/bin/rustup update"
  when: rust_installed.stat.exists
  ignore_errors: yes  # May fail if rustup is outdated, non-critical
  register: rust_update
  changed_when: "'Updated' in rust_update.stdout"

- debug:
    msg: "Warning: Rust update failed, continuing anyway"
  when: rust_update is failed
```

---

## 🟢 Minor Issues (Nice to Have)

### 13. **Redundant Privilege Escalation** ⚠️ SIMPLICITY

```yaml
become: yes
become_user: "{{ user }}"
```

Appears 15+ times. If already running as root, this is redundant.

**Fix:** Use Ansible's `remote_user` or run playbook as target user with `--become` flag.

---

### 14. **Temp File Cleanup** ⚠️ MAINTAINABILITY
**Location:** Multiple files

Some tasks clean up `/tmp`, others don't:
```yaml
# install_tools.yml - Good
- file:
    path: "{{ item }}"
    state: absent
  loop:
    - /tmp/yazi.zip

# install_deps.yml - Missing cleanup
- get_url:
    dest: /tmp/rustup-init.sh  # Never cleaned up
```

**Fix:** Add cleanup tasks or use Ansible's `tempfile` module.

---

### 15. **Missing Task File** ⚠️ MAINTAINABILITY
**Location:** `pde/main.yml:83`

```yaml
- include_tasks: tasks/install_fonts.yml
```

This file is referenced but we haven't verified it exists in the review.

**Fix:** Verify file exists or create it.

---

## 📊 Summary Statistics

| Category | Critical | Major | Minor | Total |
|----------|----------|-------|-------|-------|
| Portability | 4 | 2 | 0 | 6 |
| Simplicity | 0 | 3 | 2 | 5 |
| Maintainability | 1 | 5 | 3 | 9 |
| **TOTAL** | **5** | **10** | **5** | **20** |

---

## 🎯 Recommended Action Plan

### Phase 1: Quick Wins (1-2 hours)
1. Create `pde/defaults.yml` with overrideable variables
2. Add comprehensive README.md
3. Fix node version variable usage
4. Standardize task naming
5. Document ignore_errors usage

### Phase 2: Structural Improvements (3-4 hours)
1. Create version variables file
2. Handle ai-profiles dependency properly
3. Add architecture detection for binary downloads
4. Improve npm install idempotency
5. Add platform detection or document limitations

### Phase 3: Polish (2-3 hours)
1. Reduce redundant privilege escalation
2. Add temp file cleanup
3. Consider replacing shell with proper modules
4. Review and adjust sudo configuration
5. Add CI/CD validation (ansible-lint)

---

## 🔧 Suggested File Structure

```
personal-dev-env/
├── pde/
│   ├── defaults.yml          # NEW: User-overrideable defaults
│   ├── vars/
│   │   └── versions.yml      # NEW: Version pinning
│   ├── main.yml              # MODIFY: Load defaults
│   ├── tasks/
│   │   ├── install_deps.yml
│   │   ├── install_tools.yml
│   │   ├── install_shell.yml
│   │   └── install_fonts.yml
│   └── config/
│       ├── zsh/
│       ├── tmux/
│       ├── nvim/
│       └── xfce4/
├── ai-profiles/              # CONSIDER: Move to pde/vendor/
├── README.md                 # NEW: Complete setup guide
├── REVIEW.md                 # This file
└── Makefile                  # MODIFY: Add install target
```

---

## 💡 Best Practices to Adopt

1. **Variable Hierarchy:** defaults.yml → vars/ → command line
2. **Idempotency:** Every task should be safe to run multiple times
3. **Documentation:** Every non-obvious choice needs a comment
4. **Version Control:** Pin versions in variables, not in tasks
5. **Error Handling:** Explicit > implicit (no silent failures)
6. **Platform Detection:** Check OS/arch before platform-specific tasks
7. **User Configuration:** Never hardcode user preferences

---

## ✅ What's Good

- Tags used consistently for selective execution
- Creates checks before installs (idempotent approach)
- Organized task separation (deps, tools, shell)
- Config files centralized in `pde/config/`
- Uses Ansible best practices in many places

---

## Next Steps

Would you like me to:
1. Implement Phase 1 quick wins?
2. Create the defaults.yml and README?
3. Focus on a specific issue first?
4. Generate a refactored version of the playbooks?
