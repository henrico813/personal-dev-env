#!/usr/bin/env bash
set -e

BASE="${1:-.}"
DOCS_DIR="$BASE/docs"

if [ ! -d "$DOCS_DIR" ]; then
    mkdir -p "$DOCS_DIR"
    echo "Created $DOCS_DIR"
else
    echo "$DOCS_DIR already exists"
fi
