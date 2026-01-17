#!/usr/bin/env bash
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SOURCE_DIR="$SCRIPT_DIR/.claude"
TARGET_DIR="$HOME/.claude"

echo "Installing Claude Code configuration..."

# Preserve session history and credentials
PRESERVE_BACKUP=""
if [ -d "$TARGET_DIR/projects" ] || [ -f "$TARGET_DIR/.credentials.json" ]; then
    PRESERVE_BACKUP=$(mktemp -d)
    echo "Preserving session history and credentials..."
    [ -d "$TARGET_DIR/projects" ] && mv "$TARGET_DIR/projects" "$PRESERVE_BACKUP/"
    [ -f "$TARGET_DIR/.credentials.json" ] && mv "$TARGET_DIR/.credentials.json" "$PRESERVE_BACKUP/"
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

# Restore session history and credentials
if [ -n "$PRESERVE_BACKUP" ]; then
    echo "Restoring session history and credentials..."
    [ -d "$PRESERVE_BACKUP/projects" ] && mv "$PRESERVE_BACKUP/projects" "$TARGET_DIR/"
    [ -f "$PRESERVE_BACKUP/.credentials.json" ] && mv "$PRESERVE_BACKUP/.credentials.json" "$TARGET_DIR/"
    rmdir "$PRESERVE_BACKUP" 2>/dev/null || true
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
