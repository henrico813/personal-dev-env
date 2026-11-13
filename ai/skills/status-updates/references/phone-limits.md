# Phone limits

The user reads updates on a phone. `scripts/check_update.py` reports
each draft line that breaks one of these limits. Read this when the
checker reports a line, or when it cannot run and you check the draft
yourself.

- An update fills at most 20 screen lines. Blank lines do not count,
  and code lines do. A line over 72 characters fills one screen line
  for every 72 characters, plus one for any characters left over, so
  an unwrapped paragraph counts the same as a wrapped one.
- A table has at most three columns.
- A paragraph or list item fills at most three screen lines.
- Each line in a code block, such as a command, has at most 35
  characters. Put one command in each code block, and wrap a longer
  one in a short script first.

The checker reports each problem at the line where it starts, and
the update length at line 1.
