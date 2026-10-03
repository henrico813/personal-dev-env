# OpenCode Claude Code Adapter

This tutorial prepares the full profile's managed OpenCode Claude adapter and
authenticates Claude Code.

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

Restart OpenCode after authenticating Claude Code or changing its credentials so
that the adapter can observe the updated authentication state.

List the Claude Code models available through the managed adapter with:

```bash
opencode models --standalone | grep '^claude-code/'
```
