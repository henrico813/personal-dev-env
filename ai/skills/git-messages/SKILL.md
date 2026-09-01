---
name: git-messages
description: Use when writing commit messages or creating and updating pull requests.
---

# Git Messages

## Principles

- Use conventional commit style.
- Keep each commit focused on one reason for change.
- Keep commit and pull request titles at 50 characters or less.
- Write titles as `<type>: <action taken>`.
- Wrap body text at 72 characters.
- Add a body when the change needs context.
- Use clear high-school-level English.
- Explain why the change matters and what changed.
- Mention important risks or limits.
- Prefer concise bodies over long design notes.
- Do not add AI attribution.

## Shape

    <type>: <short action>

    <why this matters>

    <what changed>

    <any risk or important limit>

### Pull Requests

- Use the same title rules as commits.
- Preserve required Overview and Testing sections.
- Put the concise why, change, risk, and verification details in those
  sections.

## Example

    fix: stabilize frontend compose service

    Local Compose starts Vite from compose.dev.yml, not from the frontend
    Dockerfile.

    Use Node 22 and npm ci in the local Compose service so startup uses the
    package lockfile. This makes local installs more repeatable.
