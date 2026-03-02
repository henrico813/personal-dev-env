#!/usr/bin/env bash
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SOURCE_DIR="$SCRIPT_DIR/.claude"
TARGET_DIR="$HOME/.claude"

echo "Installing Claude Code configuration..."

# Preserve user data (sessions, credentials, prompt history)
PRESERVE_BACKUP=""
if [ -d "$TARGET_DIR/projects" ] || [ -f "$TARGET_DIR/.credentials.json" ] || [ -f "$TARGET_DIR/history.jsonl" ]; then
    PRESERVE_BACKUP=$(mktemp -d)
    echo "Preserving user data:"
    [ -d "$TARGET_DIR/projects" ] && echo "  - projects/ (session data)" && mv "$TARGET_DIR/projects" "$PRESERVE_BACKUP/"
    [ -f "$TARGET_DIR/.credentials.json" ] && echo "  - .credentials.json (auth)" && mv "$TARGET_DIR/.credentials.json" "$PRESERVE_BACKUP/"
    [ -f "$TARGET_DIR/history.jsonl" ] && echo "  - history.jsonl (prompt history)" && mv "$TARGET_DIR/history.jsonl" "$PRESERVE_BACKUP/"
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

# Restore user data
if [ -n "$PRESERVE_BACKUP" ]; then
    echo "Restoring user data..."
    [ -d "$PRESERVE_BACKUP/projects" ] && mv "$PRESERVE_BACKUP/projects" "$TARGET_DIR/"
    [ -f "$PRESERVE_BACKUP/.credentials.json" ] && mv "$PRESERVE_BACKUP/.credentials.json" "$TARGET_DIR/"
    [ -f "$PRESERVE_BACKUP/history.jsonl" ] && mv "$PRESERVE_BACKUP/history.jsonl" "$TARGET_DIR/"
    rmdir "$PRESERVE_BACKUP" 2>/dev/null || true
fi

# Make hooks executable
chmod +x "$TARGET_DIR/hooks/"*.py 2>/dev/null || true
chmod +x "$TARGET_DIR/statusline.sh" 2>/dev/null || true

# Make scripts executable
chmod +x "$TARGET_DIR/scripts/"*.sh 2>/dev/null || true

# Install Linux dependencies
if [[ "$(uname)" == "Linux" ]]; then
    echo ""
    echo "Installing Linux dependencies..."

    # Detect package manager and install
    if command -v apt-get &>/dev/null; then
        sudo apt-get install -y jq
    elif command -v dnf &>/dev/null; then
        sudo dnf install -y jq
    elif command -v pacman &>/dev/null; then
        sudo pacman -S --noconfirm jq
    else
        echo "Warning: Unknown package manager. Please install manually: jq"
    fi

    # Install specstory if not already present
    if ! command -v specstory &>/dev/null; then
        echo "Installing specstory..."
        curl -sL https://raw.githubusercontent.com/specstoryai/getspecstory/main/install.sh | bash
    else
        echo "specstory already installed, skipping"
    fi
fi

# OpenCode compatibility: symlink commands and agents
OC_DIR="$HOME/.config/opencode"
mkdir -p "$OC_DIR"
ln -sfn "$TARGET_DIR/commands" "$OC_DIR/commands"
ln -sfn "$TARGET_DIR/agents" "$OC_DIR/agents"

echo ""
echo "Done! Configuration installed to $TARGET_DIR"

# Show macOS instructions if applicable
if [[ "$(uname)" == "Darwin" ]]; then
    echo ""
    echo "macOS detected. Install dependencies manually:"
    echo "  brew install jq"
    echo "  brew tap specstoryai/tap && brew install specstory"
fi
