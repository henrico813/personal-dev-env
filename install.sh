#!/usr/bin/env bash
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SOURCE_DIR="$SCRIPT_DIR/.claude"
TARGET_DIR="$HOME/.claude"

echo "Installing Claude Code configuration..."

# Preserve session history
PROJECTS_BACKUP=""
if [ -d "$TARGET_DIR/projects" ]; then
    PROJECTS_BACKUP=$(mktemp -d)
    echo "Preserving session history..."
    mv "$TARGET_DIR/projects" "$PROJECTS_BACKUP/"
fi

# Backup existing
if [ -d "$TARGET_DIR" ]; then
    BACKUP="$TARGET_DIR.backup.$(date +%Y%m%d_%H%M%S)"
    echo "Backing up existing config to $BACKUP"
    mv "$TARGET_DIR" "$BACKUP"
fi

# Copy new config
echo "Copying configuration to $TARGET_DIR"
cp -r "$SOURCE_DIR" "$TARGET_DIR"

# Restore session history
if [ -n "$PROJECTS_BACKUP" ] && [ -d "$PROJECTS_BACKUP/projects" ]; then
    echo "Restoring session history..."
    mv "$PROJECTS_BACKUP/projects" "$TARGET_DIR/"
    rmdir "$PROJECTS_BACKUP"
fi

# Make hooks executable
chmod +x "$TARGET_DIR/hooks/"*.py 2>/dev/null || true
chmod +x "$TARGET_DIR/statusline.sh" 2>/dev/null || true

# Make scripts executable
chmod +x "$TARGET_DIR/scripts/"*.sh 2>/dev/null || true

echo "Done! Configuration installed to $TARGET_DIR"
echo ""
echo "Note: statusline.sh requires 'jq' - install with:"
echo "  sudo apt install jq  # Debian/Ubuntu"
echo "  brew install jq      # macOS"
