# OpenCode Claude Code Adapter

This tutorial installs the full profile, which sets up the OpenCode Claude Code
adapter that lets OpenCode use Claude Code models, then shows how to
authenticate Claude Code.

## 1. Apply the Full Profile

Run from the repository root:

```bash
pde-installer install full
```

The configuration prepares Claude Code and the managed OpenCode Claude
adapter. It does not authenticate Claude Code.

## 2. Authenticate Claude Code and Discover Models

In a login shell, authenticate with:

```bash
claude auth login --claudeai
claude auth status --json
opencode auth login claude-code
```

Restart OpenCode (quit and start it again) after authenticating Claude Code or
changing its credentials so the adapter can see the updated authentication
state.

List the Claude Code models available through the managed adapter with:

```bash
opencode models --standalone | grep '^claude-code/'
```
