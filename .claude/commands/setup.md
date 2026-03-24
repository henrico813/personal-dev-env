---
description: Configure project documentation structure
---

# Setup

Configure the docs directory for the current project.

## When Invoked

1. Check if `docs/` already exists in the current directory
2. If exists, inform user — no further action needed
3. If not, ask user to confirm creation
4. Run the setup script: `bash ~/.claude/scripts/setup-docs.sh`
5. Report what was created

## Interaction Flow

If docs/ doesn't exist:
```
I'll set up the docs directory for this project.

This will create:
  docs/

Documents use filename prefixes to indicate type:
  research-YYYY-MM-DD-description.md   # Research documents
  design-description.md                 # Design/implementation plans
  NNN-description.md                    # Issues/tickets

Create docs/ in [current directory]?
```

Then use AskUserQuestion with options:
- "Yes, create it"
- "No, cancel"

If user confirms, run:
```bash
bash ~/.claude/scripts/setup-docs.sh
```

Report the result.
