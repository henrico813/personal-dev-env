#!/usr/bin/env bash
set -e

# Generates YAML frontmatter for implementation plan documents
# Usage: plan-frontmatter.sh <title> [description]
#
# Example:
#   plan-frontmatter.sh "Auth Refactor" "Refactor authentication to use JWT"

TITLE="${1:?Usage: plan-frontmatter.sh <title> [description]}"
DESCRIPTION="${2:-$TITLE}"

# Get git info (with fallbacks)
GIT_COMMIT=$(git rev-parse HEAD 2>/dev/null || echo "not-a-git-repo")
GIT_BRANCH=$(git branch --show-current 2>/dev/null || echo "unknown")
REPO_NAME=$(basename "$(git rev-parse --show-toplevel 2>/dev/null)" 2>/dev/null || basename "$PWD")

# Timestamps
DATE_ISO=$(date -Iseconds)
DATE_SHORT=$(date +%Y-%m-%d)

cat <<EOF
---
title: "$TITLE"
description: "$DESCRIPTION"
date: $DATE_ISO

git_commit: $GIT_COMMIT
branch: $GIT_BRANCH
repository: $REPO_NAME
type: implementation-plan
status: active
tags: [planning]
created: $DATE_SHORT
---
EOF
