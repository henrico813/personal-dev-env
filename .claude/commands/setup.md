---
description: Configure project documentation structure
---

# Setup

Configure the docs directory structure for the current project.

## When Invoked

1. Check if `docs/` already exists in the current directory
2. If exists, inform user and ask if they want to add missing directories
3. If not, ask user to confirm creation
4. Run the setup script: `bash ~/.claude/scripts/setup-docs.sh`
5. Report what was created

## Interaction Flow

If docs/ doesn't exist:
```
I'll set up the docs directory structure for this project.

This will create:
  docs/
  ├── planning/active/     # Plans in progress
  ├── planning/completed/  # Finished plans
  ├── planning/archive/    # Superseded plans
  ├── research/            # Research documents
  ├── operational/         # Operational guides
  └── archive/             # General archive

Create this structure in [current directory]?
```

Then use AskUserQuestion with options:
- "Yes, create it"
- "No, cancel"

If user confirms, run:
```bash
bash ~/.claude/scripts/setup-docs.sh
```

Report the result.
