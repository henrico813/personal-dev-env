#!/usr/bin/env python3
"""
Hook to protect CLAUDE.md files:
- Blocks all modifications to user-level ~/.claude/CLAUDE.md
- Warns on large changes to other CLAUDE.md files (but allows them)
"""
import json
import sys
import os


def count_lines(content):
    """Count lines in content string."""
    if not content:
        return 0
    return len(content.strip().split('\n'))


def get_file_lines(file_path):
    """Get current line count of a file."""
    try:
        if os.path.exists(file_path):
            with open(file_path, 'r') as f:
                return count_lines(f.read())
    except:
        pass
    return 0


def estimate_new_lines(tool_name, tool_input, current_lines):
    """Estimate the resulting line count after the operation."""
    if tool_name == 'Write':
        # Write replaces entire file
        return count_lines(tool_input.get('content', ''))
    elif tool_name in ['Edit', 'MultiEdit']:
        # Edit replaces old_string with new_string
        old = tool_input.get('old_string', '')
        new = tool_input.get('new_string', '')
        old_lines = count_lines(old)
        new_lines = count_lines(new)
        return current_lines - old_lines + new_lines
    return current_lines


def main():
    try:
        input_data = json.load(sys.stdin)
        tool_name = input_data.get('tool_name', '')

        # Check only file modification tools
        if tool_name not in ['Edit', 'MultiEdit', 'Write', 'NotebookEdit']:
            sys.exit(0)

        file_path = input_data.get('tool_input', {}).get('file_path', '')
        tool_input = input_data.get('tool_input', {})

        # Check if the file being modified is named CLAUDE.md
        if os.path.basename(file_path).upper() != 'CLAUDE.MD':
            sys.exit(0)

        # Get the user's home directory
        home_dir = os.path.expanduser('~')
        user_claude_md = os.path.join(home_dir, '.claude', 'CLAUDE.md')

        # Normalize paths for comparison
        normalized_path = os.path.normpath(os.path.abspath(file_path))
        normalized_user_path = os.path.normpath(user_claude_md)

        # Block modifications to user-level CLAUDE.md completely
        if normalized_path == normalized_user_path:
            print("BLOCKED: Cannot modify user-level CLAUDE.md")
            print("\n~/.claude/CLAUDE.md contains your global instructions.")
            print("This file is protected from all automated modifications.")
            print("\nEdit this file manually if you need to update it.")
            sys.exit(2)

        # For other CLAUDE.md files, warn on large changes
        current_lines = get_file_lines(file_path)
        new_lines = estimate_new_lines(tool_name, tool_input, current_lines)
        lines_added = new_lines - current_lines

        # Warn if adding significant content
        if lines_added > 50:
            print(f"WARNING: This change adds ~{lines_added} lines to CLAUDE.md")
            print(f"  Current: {current_lines} lines")
            print(f"  After:   {new_lines} lines")
            print("\nLarge CLAUDE.md files hurt context. Consider:")
            print("  - Moving component docs to directory-local README.md")
            print("  - Keeping only project-wide rules in CLAUDE.md")
            print("\nProceeding with modification...")

        # Warn if file is getting large overall
        elif new_lines > 200:
            print(f"WARNING: CLAUDE.md is growing large ({new_lines} lines)")
            print("\nConsider moving detailed documentation to README.md files.")
            print("\nProceeding with modification...")

        sys.exit(0)  # Allow the modification

    except Exception as e:
        # Silent fail - don't break Claude's workflow
        sys.exit(0)


if __name__ == '__main__':
    main()
