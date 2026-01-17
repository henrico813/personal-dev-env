---
title: "Tmux Window Layout Function"
description: "Add tw function to create VSCode-style tmux windows with per-project YAML config"
date: 2026-01-17T11:30:27-08:00
author: justin
git_commit: 766b3cd6fb97bc3d18d530a6f62af259968a208e
branch: feature/tmux-window-shortcuts
repository: personal-dev-env
type: implementation-plan
status: completed
tags: [planning]
created: 2026-01-17
---

# Tmux Window Layout Function Implementation Plan

## Overview

Add a `tw` (tmux window) shell function that creates tmux windows with a consistent VSCode-style 4-pane layout. Supports per-project configuration via `.tw.yml` files and command-line argument overrides.

**Why**: Provides a consistent workspace layout for any project, with optional auto-starting of commands in each pane. All windows stay in one tmux session for easy switching with `prefix + 0-9`.

**Security Note**: Commands in `.tw.yml` files execute automatically when `tw` is called. Similar to `.envrc` with direnv, be cautious when running `tw` in directories from untrusted sources.

## Current State

- personal-dev-env repo at `/home/justin/Projects/personal-dev-env`
- Shell config: `pde/config/zsh/.zshrc` (deployed via Ansible)
- Tmux config: `pde/config/tmux/.tmux.conf` (has tmux-resurrect, no continuum)
- Similar pattern exists with `ai()` function in zshrc

## Desired End State

A `tw` function that:
1. Creates a tmux window with VSCode-style layout
2. Names the window after the target directory
3. Reads `.tw.yml` from project root for pane commands (if present)
4. Accepts positional args as override: `tw /path 'left' 'top' 'bottom' 'right'`
5. Works with no config (creates empty panes)
6. Fails fast with clear errors when prerequisites are missing

### Layout
```
+------+------------------+------+
|      |                  |      |
|      |    top-middle    |      |
| left |    (main)        | right|
|      +------------------+      |
|      |                  |      |
|      |  bottom-middle   |      |
|      |   (secondary)    |      |
+------+------------------+------+
```
- Left/right sidebars: ~15% each
- Middle: ~70%, split horizontally

### Config File Format
`.tw.yml` in project root:
```yaml
left: "git log --oneline -20"
top: "claude"
bottom: ""
right: "htop"
```

---

## Phase 1: Install yq (Mike Farah's Go version)

### Changes Required

**File**: `pde/tasks/install_shell.yml`

**Important**: The apt `yq` package is Python-based (kislyuk/yq) and does NOT support the `-r` flag or `// ""` syntax. We must install Mike Farah's Go-based yq via binary download.

Add after the eza cargo install block (around line 21):

```yaml
- name: Check if yq is already installed
  stat:
    path: /usr/local/bin/yq
  register: yq_installed

- name: Install Mike Farah's yq (Go-based YAML processor)
  block:
    - name: Download yq binary
      get_url:
        url: https://github.com/mikefarah/yq/releases/latest/download/yq_linux_amd64
        dest: /tmp/yq_linux_amd64
        mode: '0755'

    - name: Install yq to /usr/local/bin
      become: yes
      copy:
        src: /tmp/yq_linux_amd64
        dest: /usr/local/bin/yq
        mode: '0755'
        remote_src: yes

    - name: Cleanup yq download
      file:
        path: /tmp/yq_linux_amd64
        state: absent
  when: not yq_installed.stat.exists
```

### Deployment

```bash
cd /home/justin/Projects/personal-dev-env/pde
ansible-playbook main.yml -e "user=$USER" --tags shell
```

### Success Criteria

#### Automated
- [ ] Ansible playbook completes without errors
- [ ] `which yq` returns `/usr/local/bin/yq`
- [ ] `yq --version` shows mikefarah/yq version (v4.x)
- [ ] `yq -r '.test // "default"' <<< 'test: hello'` outputs `hello`

#### Pre-deployment Verification
- [x] Ansible playbook syntax check passes
- [x] install_shell.yml changes in place

#### Manual
- [ ] None

---

## Phase 2: Add tw function to zshrc

### Changes Required

**File**: `pde/config/zsh/.zshrc`

Add after the existing `ai()` function (after line 160):

```bash
# Tmux Window - create window with VSCode-style layout
# Usage: tw [directory] [left_cmd] [top_cmd] [bottom_cmd] [right_cmd]
# Config: Place .tw.yml in project root with keys: left, top, bottom, right
tw() {
    # Validate prerequisites
    if [[ -z "$TMUX" ]]; then
        echo "Error: tw must be run from inside a tmux session"
        echo "Start tmux first: tmux new -s dev"
        return 1
    fi

    # Resolve directory
    local dir="${1:-.}"
    dir="$(cd "$dir" 2>/dev/null && pwd)" || {
        echo "Error: Directory not found: $1"
        return 1
    }

    # Sanitize window name (tmux uses : . [ ] as special chars)
    local name="$(basename "$dir" | tr ':.[]-' '_')"
    [[ "$name" == "/" ]] && name="root"
    [[ -z "$name" ]] && name="unnamed"

    local config="$dir/.tw.yml"

    # Parse commands from config or positional args
    local cmd_left="" cmd_top="" cmd_bottom="" cmd_right=""

    if [[ -f "$config" ]]; then
        if ! command -v yq &>/dev/null; then
            echo "Error: yq is required to parse .tw.yml but not installed"
            echo "Run the PDE ansible playbook to install it"
            return 1
        fi
        if ! yq -e '.' "$config" &>/dev/null; then
            echo "Error: Invalid YAML in $config"
            return 1
        fi
        cmd_left="$(yq -r '.left // ""' "$config")"
        cmd_top="$(yq -r '.top // ""' "$config")"
        cmd_bottom="$(yq -r '.bottom // ""' "$config")"
        cmd_right="$(yq -r '.right // ""' "$config")"
        # Handle explicit YAML null
        [[ "$cmd_left" == "null" ]] && cmd_left=""
        [[ "$cmd_top" == "null" ]] && cmd_top=""
        [[ "$cmd_bottom" == "null" ]] && cmd_bottom=""
        [[ "$cmd_right" == "null" ]] && cmd_right=""
    elif [[ $# -gt 1 ]]; then
        cmd_left="${2:-}"
        cmd_top="${3:-}"
        cmd_bottom="${4:-}"
        cmd_right="${5:-}"
    fi

    # Create window and capture ID for robust pane targeting
    local window_id
    window_id=$(tmux new-window -n "$name" -c "$dir" -P -F "#{window_id}")

    if [[ -z "$window_id" ]]; then
        echo "Error: Failed to create tmux window"
        return 1
    fi

    # Build layout: left | middle (top/bottom) | right
    # Note: Pane indices depend on this exact split order
    tmux split-window -t "$window_id" -hb -l 15% -c "$dir"
    tmux select-pane -t "$window_id" -R
    tmux split-window -t "$window_id" -h -l 18% -c "$dir"
    tmux select-pane -t "$window_id" -L
    tmux split-window -t "$window_id" -v -c "$dir"

    # Send commands to panes (0=left, 1=top, 2=bottom, 3=right)
    [[ -n "$cmd_left" ]] && tmux send-keys -t "${window_id}.0" "$cmd_left" C-m
    [[ -n "$cmd_top" ]] && tmux send-keys -t "${window_id}.1" "$cmd_top" C-m
    [[ -n "$cmd_bottom" ]] && tmux send-keys -t "${window_id}.2" "$cmd_bottom" C-m
    [[ -n "$cmd_right" ]] && tmux send-keys -t "${window_id}.3" "$cmd_right" C-m

    # Focus top-middle (main editor area)
    tmux select-pane -t "${window_id}.1"
}
```

### Deployment

```bash
cd /home/justin/Projects/personal-dev-env/pde
ansible-playbook main.yml -e "user=$USER" --tags shell
```

Then reload shell: `source ~/.zshrc` or restart terminal.

### Success Criteria

#### Automated
- [ ] `source ~/.zshrc` completes without errors
- [ ] `type tw` shows the function definition

#### Pre-deployment Verification
- [x] zshrc syntax check passes (`zsh -n` returns no errors)
- [x] tw() function added to .zshrc

#### Manual
- [x] Running `tw` outside tmux shows clear error message
- [x] `tw ~/Projects/someproject` creates a 4-pane window named "someproject"
- [x] `tw ~/Projects/someproject '' 'echo hello' '' ''` runs "echo hello" in top-middle pane
- [x] Create `.tw.yml` in a project, verify commands auto-run
- [x] Invalid YAML in `.tw.yml` shows clear error message

---

## Phase 3: Optional - Add tmux-continuum

> **Note**: This phase is orthogonal to the tw() function and can be done as a separate PR.

### Changes Required

**File**: `pde/config/tmux/.tmux.conf`

Add after the tmux-resurrect plugin line:

```bash
set -g @plugin 'tmux-plugins/tmux-continuum'
set -g @continuum-restore 'on'
```

### Deployment

```bash
cd /home/justin/Projects/personal-dev-env/pde
ansible-playbook main.yml -e "user=$USER" --tags shell
```

Then in tmux: `prefix + I` to install the plugin.

### Success Criteria

#### Automated
- [ ] `prefix + I` installs the plugin without errors

#### Manual
- [ ] Close and reopen tmux, verify session is auto-restored

---

## References

- `pde/config/zsh/.zshrc` - Shell config with similar `ai()` function pattern
- `pde/config/tmux/.tmux.conf` - Tmux config with TPM and resurrect
- `pde/tasks/install_shell.yml` - Shell tool installation (where yq goes)
- `pde/main.yml` - Main Ansible playbook with deployment tags
