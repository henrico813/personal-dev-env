#!/usr/bin/env bash
# Read JSON from stdin, output status line

# Read stdin
INPUT=$(cat)
[ -z "$INPUT" ] && exit 0

# Parse JSON (requires jq)
MODEL=$(echo "$INPUT" | jq -r '.model.display_name // "Claude"')
PROJECT_DIR=$(echo "$INPUT" | jq -r '.workspace.project_dir // .workspace.current_dir // ""')
CURRENT_DIR=$(echo "$INPUT" | jq -r '.workspace.current_dir // ""')
USED_PCT=$(echo "$INPUT" | jq -r '.context_window.used_percentage // ""')

# Project name
if [ -n "$PROJECT_DIR" ]; then
    PROJECT=$(basename "$PROJECT_DIR")
elif [ -n "$CURRENT_DIR" ]; then
    PROJECT=$(basename "$CURRENT_DIR")
else
    PROJECT="no-project"
fi

# Git info (with caching)
CACHE_FILE="$HOME/.claude/.statusline_cache"
CACHE_TTL=300
GIT_BRANCH=""
GIT_STATUS=""

if [ -d "${CURRENT_DIR}/.git" ] 2>/dev/null; then
    # Check cache
    if [ -f "$CACHE_FILE" ]; then
        CACHE_DIR=$(grep "^DIR=" "$CACHE_FILE" | cut -d= -f2-)
        CACHE_TIME=$(grep "^TIME=" "$CACHE_FILE" | cut -d= -f2-)
        NOW=$(date +%s)
        if [ "$CACHE_DIR" = "$CURRENT_DIR" ] && [ $((NOW - CACHE_TIME)) -lt $CACHE_TTL ]; then
            GIT_BRANCH=$(grep "^BRANCH=" "$CACHE_FILE" | cut -d= -f2-)
            GIT_STATUS=$(grep "^STATUS=" "$CACHE_FILE" | cut -d= -f2-)
        fi
    fi

    # Fetch fresh if no cache
    if [ -z "$GIT_BRANCH" ]; then
        cd "$CURRENT_DIR" 2>/dev/null || exit 0
        GIT_BRANCH=$(git branch --show-current 2>/dev/null)
        [ -z "$GIT_BRANCH" ] && GIT_BRANCH="HEAD@$(git rev-parse --short HEAD 2>/dev/null)"

        # Dirty?
        [ -n "$(git status --porcelain 2>/dev/null)" ] && GIT_STATUS="*"

        # Ahead/behind
        UPSTREAM=$(git rev-parse --abbrev-ref '@{u}' 2>/dev/null)
        if [ -n "$UPSTREAM" ]; then
            AHEAD=$(git rev-list --count '@{u}..HEAD' 2>/dev/null)
            BEHIND=$(git rev-list --count 'HEAD..@{u}' 2>/dev/null)
            [ "$AHEAD" -gt 0 ] 2>/dev/null && GIT_STATUS="${GIT_STATUS}+${AHEAD}"
            [ "$BEHIND" -gt 0 ] 2>/dev/null && GIT_STATUS="${GIT_STATUS}-${BEHIND}"
        fi

        # Write cache
        mkdir -p "$(dirname "$CACHE_FILE")"
        cat > "$CACHE_FILE" <<EOF
DIR=$CURRENT_DIR
TIME=$(date +%s)
BRANCH=$GIT_BRANCH
STATUS=$GIT_STATUS
EOF
    fi
fi

# Context color
CTX_DISPLAY=""
if [ -n "$USED_PCT" ] && [ "$USED_PCT" != "null" ]; then
    PCT=${USED_PCT%.*}
    [ "$PCT" -gt 100 ] 2>/dev/null && PCT=100
    if [ "$PCT" -lt 50 ]; then
        COLOR="32"  # green
    elif [ "$PCT" -lt 80 ]; then
        COLOR="33"  # yellow
    else
        COLOR="31"  # red
    fi
    CTX_DISPLAY="\033[${COLOR}m${PCT}% Context\033[0m"
fi

# Build output (text only, no emojis per CLAUDE.md)
OUTPUT="$PROJECT | $MODEL"
[ -n "$GIT_BRANCH" ] && OUTPUT="$OUTPUT | ${GIT_BRANCH}${GIT_STATUS}"
[ -n "$CTX_DISPLAY" ] && OUTPUT="$OUTPUT | $CTX_DISPLAY"

echo -e "$OUTPUT"
