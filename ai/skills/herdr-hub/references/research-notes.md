# Read a research note through a summary

Read this before you bring the user the findings of a research note.

You reread your whole context on every turn, and one note can hold
tens of thousands of tokens. So never read the note in full. Have a
`local-read` job write a summary that quotes the note, check the
quotes with a script, and read only the lines that pass.

1. Brief the job as `model-routing` says for a reading job. Its input
   is the note. Its answer is a summary file of at most 10 JSON
   lines, one per finding. Each line holds these fields:
   - `file`: the note's path.
   - `quote`: exact words from the note.
   - `point`: one sentence on what the quote means for the user.
2. Check the summary with `model-routing`'s `check_quotes.py`, as that
   skill says. The script writes each line whose quote it found to a
   table file. When the check fails, follow `model-routing`'s "When
   the check fails".
3. Read only the table file. When a quote needs more context, read
   only the lines around it, such as with
   `rg -n -C 5 "<words from the quote>" "<note>"`.
4. Write your message from the quotes, since the check covers only
   them. A `point` is the job's own reading, so check it against its
   quote before you repeat it.
