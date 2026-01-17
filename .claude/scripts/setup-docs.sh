#!/usr/bin/env bash
set -e

# Creates standard docs directory structure in current directory
# Usage: setup-docs.sh [base_dir]

BASE="${1:-.}"
DOCS_DIR="$BASE/docs"

dirs=(
    "$DOCS_DIR/planning/active"
    "$DOCS_DIR/planning/completed"
    "$DOCS_DIR/planning/archive"
    "$DOCS_DIR/research"
    "$DOCS_DIR/operational"
    "$DOCS_DIR/archive"
)

created=0
for dir in "${dirs[@]}"; do
    if [ ! -d "$dir" ]; then
        mkdir -p "$dir"
        touch "$dir/.gitkeep"
        ((created++)) || true
    fi
done

echo "Docs structure ready at $DOCS_DIR ($created directories created)"
