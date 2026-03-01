#!/usr/bin/env python3
"""
Hook to prevent commits containing "Claude" or "Anthropic" in any form, and emojis.
Blocks commits with these terms in:
- Commit messages
- Author fields
- Co-author fields
- Emojis in commit messages
"""
import json
import sys
import re

def contains_emoji(text):
    """Check if text contains any emoji characters."""
    # Unicode ranges for emoji characters
    emoji_pattern = re.compile(
        "["
        "\U0001F600-\U0001F64F"  # emoticons
        "\U0001F300-\U0001F5FF"  # symbols & pictographs
        "\U0001F680-\U0001F6FF"  # transport & map symbols
        "\U0001F1E0-\U0001F1FF"  # flags (iOS)
        "\U00002702-\U000027B0"  # dingbats
        "\U000024C2-\U0001F251"  # enclosed characters
        "\U0001F900-\U0001F9FF"  # supplemental symbols
        "\U0001FA70-\U0001FAFF"  # symbols and pictographs extended-a
        "]+", flags=re.UNICODE)
    return bool(emoji_pattern.search(text))

def extract_commit_metadata(command):
    """Extract message, author, and trailers from a git commit command."""
    result = {'message': None, 'author': None, 'trailers': []}

    # Heredoc: -m "$(cat <<'EOF'\n...\nEOF\n)"
    heredoc = re.search(
        r'''(?:-m|--message)\s*=?\s*"\$\(cat\s+<<'?(\w+)'?\n(.*?)\n\s*\1\s*\)"''',
        command, re.DOTALL)
    if heredoc:
        result['message'] = heredoc.group(2)
    else:
        # Double-quoted: -m "msg" or --message="msg"
        msg = re.search(
            r'''(?:-m|--message)\s*=?\s*"((?:[^"\\]|\\.)*)"''',
            command, re.DOTALL)
        if msg:
            result['message'] = msg.group(1)
        else:
            # Single-quoted: -m 'msg'
            msg = re.search(
                r"""(?:-m|--message)\s*=?\s*'([^']*)'""",
                command, re.DOTALL)
            if msg:
                result['message'] = msg.group(1)

    # Author: --author "value" or --author="value"
    author = re.search(r'''--author\s*=?\s*"((?:[^"\\]|\\.)*)"''', command)
    if not author:
        author = re.search(r"""--author\s*=?\s*'([^']*)'""", command)
    if author:
        result['author'] = author.group(1)

    # Trailers: --trailer "value" (can appear multiple times)
    result['trailers'] = re.findall(
        r'''--trailer\s*=?\s*"((?:[^"\\]|\\.)*)"''', command)
    result['trailers'] += re.findall(
        r"""--trailer\s*=?\s*'([^']*)'""", command)

    return result


def check_git_commit_command(command):
    """Check if commit metadata (message, author, trailers) contains prohibited terms."""
    prohibited_terms = ['claude', 'anthropic']

    metadata = extract_commit_metadata(command)

    if metadata['message']:
        msg_lower = metadata['message'].lower()

        if contains_emoji(metadata['message']):
            return True, "Commit message contains emojis - removing emojis from commit"

        for term in prohibited_terms:
            if term in msg_lower:
                return True, f"Commit message contains '{term}' - removing all Claude/Anthropic references"

    if metadata['author']:
        author_lower = metadata['author'].lower()
        for term in prohibited_terms:
            if term in author_lower:
                return True, "Cannot set Claude/Anthropic as commit author"

    for trailer in metadata['trailers']:
        trailer_lower = trailer.lower()
        for term in prohibited_terms:
            if term in trailer_lower:
                return True, f"Trailer contains '{term}' - removing Claude/Anthropic references"

    return False, None

def suggest_cleaned_command(command):
    """Suggest a cleaned version of the command."""
    # Remove all emojis
    emoji_pattern = re.compile(
        "["
        "\U0001F600-\U0001F64F"  # emoticons
        "\U0001F300-\U0001F5FF"  # symbols & pictographs
        "\U0001F680-\U0001F6FF"  # transport & map symbols
        "\U0001F1E0-\U0001F1FF"  # flags (iOS)
        "\U00002702-\U000027B0"  # dingbats
        "\U000024C2-\U0001F251"  # enclosed characters
        "\U0001F900-\U0001F9FF"  # supplemental symbols
        "\U0001FA70-\U0001FAFF"  # symbols and pictographs extended-a
        "]+", flags=re.UNICODE)
    cleaned = emoji_pattern.sub('', command)

    # Remove co-author lines with Claude/Anthropic
    cleaned = re.sub(r'(?i)co-authored-by:.*(?:claude|anthropic).*\n?', '', cleaned)

    # Remove any lines mentioning generated with Claude
    cleaned = re.sub(r'(?i).*generated with.*claude.*\n?', '', cleaned)

    # Clean up author fields
    if '--author' in cleaned:
        cleaned = re.sub(r'--author[= ]["\']?[^"\']*(?:claude|anthropic)[^"\']*["\']?', '', cleaned, flags=re.IGNORECASE)

    # Remove extra whitespace and newlines
    cleaned = re.sub(r'\n{3,}', '\n\n', cleaned)
    cleaned = cleaned.strip()

    return cleaned

def main():
    try:
        input_data = json.load(sys.stdin)
        tool_name = input_data.get('tool_name', '')
        tool_input = input_data.get('tool_input', {})
        cwd = input_data.get('cwd', '')

        # Exception: Skip checks if we're in the ~/.claude/ directory
        # This is the only directory where "claude" is allowed in paths
        import os
        claude_dir = os.path.expanduser('~/.claude').replace('\\', '/')
        current_dir = cwd.replace('\\', '/')
        if current_dir.startswith(claude_dir):
            sys.exit(0)  # Allow all commands in ~/.claude/

        # Handle both Bash commands and MCP git tools
        if tool_name == 'Bash':
            command = tool_input.get('command', '')
        elif tool_name == 'git_commit':
            # For MCP git_commit tool, check the message parameter
            message = tool_input.get('message', '')
            if message:
                # Check the commit message for prohibited content
                has_issue, issue_message = check_git_commit_command(f'git commit -m "{message}"')
                if has_issue:
                    print(f"BLOCKED: {issue_message}", file=sys.stderr)
                    print("\nYour CLAUDE.md configuration specifies:", file=sys.stderr)
                    print("- Never add Claude as a commit author", file=sys.stderr)
                    print("- Always commit using the default git settings", file=sys.stderr)
                    sys.exit(2)  # Exit code 2 blocks the command
            sys.exit(0)
        else:
            sys.exit(0)

        command = tool_input.get('command', '')

        # Check if this is a git commit command
        if 'git commit' not in command and 'git config' not in command:
            sys.exit(0)

        # Block git config commands that try to set Claude as author
        if 'git config' in command:
            command_lower = command.lower()
            if 'user.name' in command and ('claude' in command_lower or 'anthropic' in command_lower):
                print("BLOCKED: Cannot set git user.name to Claude or Anthropic", file=sys.stderr)
                print("Use the default git settings for commits", file=sys.stderr)
                sys.exit(2)  # Exit code 2 blocks the command
            if 'user.email' in command and ('claude' in command_lower or 'anthropic' in command_lower):
                print("BLOCKED: Cannot set git user.email with Claude/Anthropic", file=sys.stderr)
                print("Use the default git settings for commits", file=sys.stderr)
                sys.exit(2)

        # Check git commit commands
        has_issue, message = check_git_commit_command(command)

        if has_issue:
            print(f"BLOCKED: {message}", file=sys.stderr)
            print("\nYour CLAUDE.md configuration specifies:", file=sys.stderr)
            print("- Never add Claude as a commit author", file=sys.stderr)
            print("- Always commit using the default git settings", file=sys.stderr)

            # Suggest cleaned command if it's a commit
            if 'git commit' in command:
                cleaned = suggest_cleaned_command(command)
                if cleaned and 'git commit' in cleaned:
                    print("\nSuggested cleaned command:", file=sys.stderr)
                    print(cleaned, file=sys.stderr)
                    print("\nThe commit will use your default git author settings.", file=sys.stderr)

            sys.exit(2)  # Exit code 2 blocks the command

    except Exception as e:
        # Silent fail - don't break Claude's workflow
        sys.exit(0)

if __name__ == '__main__':
    main()
