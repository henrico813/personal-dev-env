# Sort a card pile

Read this when the user asks you to sort many cards at once, such as
every card in `review`, into a pick list. Reading each card is a
reading job for the local model, as `model-routing` says. A script
checks what it wrote, and only then does a stronger model sort it.

1. List the card files to read, such as with `card board`, and give
   them to a worker on the local model, started with
   `herdr-handoff`. Brief it as `model-routing` says for a reading
   job. The brief names the card files and one answer file, and asks
   for one JSON line per card in this form, and nothing else:

   ```json
   {"file": "<card file>", "key": "<card key>", "stage": "<stage>", "last": "<last log date>", "pr": "<linked pull request or none>", "quote": "<exact words from the card that show its state>"}
   ```

2. Once the worker stops, check its answers yourself:

   ```bash
   python3 ~/.agents/skills/model-routing/scripts/check_quotes.py \
     <answer file> <table file>
   ```

   It writes each line whose quote it found in the card to the table
   file, and prints one line for each line it dropped. A dropped
   line fails the check, as `model-routing` says. Hand those cards to
   a new worker with the same brief.
3. Sort only the table file into the pick list. Read a card yourself
   only when its line leaves a choice open.
