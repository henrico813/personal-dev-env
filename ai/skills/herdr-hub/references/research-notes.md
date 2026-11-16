# Read a research note through a summary

Read this before you bring the user the findings of a research note.

You reread your whole context on every turn, and one note can hold
tens of thousands of tokens. So never read the note in full. Have a
`local-read` job write a summary that quotes the note, check the
quotes with a script, and read only the lines that pass. Load
`model-routing` and `herdr-handoff` for these steps.

1. Write the job's brief as `model-routing` says for a reading job.
   Its input is the note. Its answer is a summary file of at most 10
   JSON lines, one per finding, so the summary stays short enough to
   read whole. Each line holds these fields:
   - `file`: the note's path.
   - `quote`: exact words from the note.
   - `point`: one sentence on what the quote means for the user.
2. Hand the job to a worker, as `herdr-handoff`'s "Hand a local job
   to a worker" says. Never run it as your own subagent, since Rules
   sends each local job to a worker.
3. Check the summary yourself with `model-routing`'s
   `check_quotes.py`. Its second argument is a table file you name,
   such as `/tmp/<note slug>-table.jsonl`. The script copies each line
   whose quote it found into that file. When the check fails, follow
   `model-routing`'s "When the check fails".
4. Read only the table file. When a quote needs more context, read
   only the lines around it, such as with
   `rg -n -C 5 "<words from the quote>" "<note>"`.
5. Write your message from the quotes, since the check covers only
   them. A `point` is the job's own reading of its quote. Repeat a
   `point` only when its quote says the same thing.
