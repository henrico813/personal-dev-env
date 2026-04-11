#!/usr/bin/env bash
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SOURCE_DIR="$SCRIPT_DIR/.claude"
TARGET_DIR="$HOME/.claude"
ENGINE_SOURCE_DIR="$SCRIPT_DIR/planner"
ENGINE_BUILD_DIR=""

build_planner() {
    if ! command -v go >/dev/null 2>&1; then
        echo "install.sh: Go is required to build planner" >&2
        exit 1
    fi

    ENGINE_BUILD_DIR="$(mktemp -d)"
    if ! (cd "$ENGINE_SOURCE_DIR" && go build -o "$ENGINE_BUILD_DIR/planner" .); then
        echo "install.sh: failed to build planner" >&2
        exit 1
    fi
}

cleanup_engine_build() {
    if [ -n "$ENGINE_BUILD_DIR" ] && [ -d "$ENGINE_BUILD_DIR" ]; then
        rm -rf "$ENGINE_BUILD_DIR"
    fi
}

install_planner_runtime() {
    local planner_output="$ENGINE_BUILD_DIR/planner"

    echo "Installing shared planner runtime..."

    install -d "$HOME/.claude/bin"
    rm -f "$HOME/.claude/bin/create_plan" "$HOME/.claude/bin/create-plan"
    install -Dm755 "$planner_output" "$HOME/.claude/bin/planner"

    install -d "$HOME/.config/opencode/bin"
    rm -f "$HOME/.config/opencode/bin/create_plan" "$HOME/.config/opencode/bin/create-plan"
    install -Dm755 "$planner_output" "$HOME/.config/opencode/bin/planner"

    install -d "$HOME/.codex/skills/create-plan/bin"
    rm -f "$HOME/.codex/skills/create-plan/bin/create_plan" "$HOME/.codex/skills/create-plan/bin/create-plan"
    install -Dm755 "$planner_output" "$HOME/.codex/skills/create-plan/bin/planner"
}

trap cleanup_engine_build EXIT

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

    # Avoid touching the package manager when jq is already present.
    if command -v jq &>/dev/null; then
        echo "jq already installed, skipping"
    elif command -v apt-get &>/dev/null; then
        sudo apt-get install -y jq
    elif command -v dnf &>/dev/null; then
        sudo dnf install -y jq
    elif command -v pacman &>/dev/null; then
        sudo pacman -S --noconfirm jq
    else
        echo "Warning: Unknown package manager. Please install manually: jq"
    fi

    # Install specstory if not already present
    # Note: upstream installer is broken (wrong filename pattern), so we download directly
    if ! command -v specstory &>/dev/null; then
        echo "Installing specstory..."
        SPECSTORY_VERSION=$(curl -sI https://github.com/specstoryai/getspecstory/releases/latest | grep -i location | sed 's/.*tag\///' | tr -d '\r\n')
        if [ -z "$SPECSTORY_VERSION" ]; then
            echo "Warning: Could not detect specstory version, skipping installation"
        else
            SPECSTORY_ARCH=$(uname -m)
            SPECSTORY_URL="https://github.com/specstoryai/getspecstory/releases/download/${SPECSTORY_VERSION}/SpecStoryCLI_Linux_${SPECSTORY_ARCH}.tar.gz"
            SPECSTORY_TMP=$(mktemp -d)
            curl -sL "$SPECSTORY_URL" | tar -xz -C "$SPECSTORY_TMP"
            sudo mv "$SPECSTORY_TMP/specstory" /usr/local/bin/
            rm -rf "$SPECSTORY_TMP"
            echo "specstory ${SPECSTORY_VERSION} installed"
        fi
    else
        echo "specstory already installed, skipping"
    fi
fi

# OpenCode compatibility: copy opencode-specific config
OC_DIR="$HOME/.config/opencode"
if [ -d "$SCRIPT_DIR/.opencode" ]; then
    echo "Installing OpenCode configuration..."
    mkdir -p "$OC_DIR"
    # Remove stale symlinks from previous installs
    [ -L "$OC_DIR/agents" ] && rm "$OC_DIR/agents"
    [ -L "$OC_DIR/commands" ] && rm "$OC_DIR/commands"
    cp -r "$SCRIPT_DIR/.opencode/." "$OC_DIR/"
fi

# Codex compatibility: install repo-managed skills without replacing user state
CODEX_DIR="$HOME/.codex"
CODEX_SKILLS_DIR="$HOME/.codex/skills"
CODEX_BACKUP_DIR=""
if [ -d "$SCRIPT_DIR/.codex/skills" ]; then
    echo "Installing Codex skills..."
    mkdir -p "$CODEX_SKILLS_DIR"

    for skill_path in "$SCRIPT_DIR/.codex/skills/"*; do
        [ -d "$skill_path" ] || continue
        skill_name="$(basename "$skill_path")"

        if [ -d "$CODEX_SKILLS_DIR/$skill_name" ]; then
            if [ -z "$CODEX_BACKUP_DIR" ]; then
                CODEX_BACKUP_DIR="$HOME/.codex/skills.backup.$(date +%Y%m%d_%H%M%S)"
                mkdir -p "$CODEX_BACKUP_DIR"
                echo "Backing up managed Codex skills to $CODEX_BACKUP_DIR"
            fi

            cp -r "$CODEX_SKILLS_DIR/$skill_name" "$CODEX_BACKUP_DIR/"
            rm -rf "$CODEX_SKILLS_DIR/$skill_name"
        fi

        cp -r "$skill_path" "$CODEX_SKILLS_DIR/"
    done
fi

if [ -d "$SCRIPT_DIR/planner" ]; then
    build_planner
    install_planner_runtime
fi

# Codex compatibility: point global instructions at the installed Claude config
CODEX_AGENTS_PATH="$CODEX_DIR/AGENTS.md"
CLAUDE_GLOBAL_INSTRUCTIONS="$TARGET_DIR/CLAUDE.md"
if [ -f "$CLAUDE_GLOBAL_INSTRUCTIONS" ]; then
    echo "Linking Codex global instructions to $CLAUDE_GLOBAL_INSTRUCTIONS"
    mkdir -p "$CODEX_DIR"

    if [ -L "$CODEX_AGENTS_PATH" ] && [ "$(readlink "$CODEX_AGENTS_PATH")" = "$CLAUDE_GLOBAL_INSTRUCTIONS" ]; then
        :
    else
        if [ -e "$CODEX_AGENTS_PATH" ] || [ -L "$CODEX_AGENTS_PATH" ]; then
            CODEX_AGENTS_BACKUP="$CODEX_DIR/AGENTS.md.backup.$(date +%Y%m%d_%H%M%S)"
            echo "Backing up existing Codex AGENTS.md to $CODEX_AGENTS_BACKUP"
            mv "$CODEX_AGENTS_PATH" "$CODEX_AGENTS_BACKUP"
        fi

        ln -s "$CLAUDE_GLOBAL_INSTRUCTIONS" "$CODEX_AGENTS_PATH"
    fi
fi

echo ""
echo "Done! Configuration installed to $TARGET_DIR"

# Show macOS instructions if applicable
if [[ "$(uname)" == "Darwin" ]]; then
    echo ""
    echo "macOS detected. Install dependencies manually:"
    echo "  brew install jq"
    echo "  brew tap specstoryai/tap && brew install specstory"
fi
