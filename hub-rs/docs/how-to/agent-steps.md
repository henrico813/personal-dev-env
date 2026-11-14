# Steps for an agent

This page tells an AI agent what to run, what it will see and what to
do next. Follow the part for your role. Each part stands alone.

Every `hub` command ends with an exit code. Act on it the same way
every time:

- `0`: it worked. Go on.
- `1`: a tool such as Herdr or `card` failed. Read the error that
  starts with `hub:`, wait a minute, try once more, then tell the
  user.
- `2`: the command was wrong. Read the error that starts with `hub:`
  or `error:`, fix the command and run it again.
- `3`: it did not do the work. The error that starts with `hub:` says
  why. Do what the step says for it.

## You are a worker

A worker is a session that a hub started. You need two things:

- Your name, such as `demo-kb-001-impl`. Your prompt or handoff file
  gives it.
- The run folder, a folder the hub reads. It is in `HUB_RUN`.

1. Check the run folder:

   ```sh
   echo "$HUB_RUN"
   ```

   If it prints a path, go on. If it prints an empty line, find the
   run folder in your handoff file and add `--run <folder>` after the
   command name in every step, such as
   `hub inbox --run /tmp/run demo-kb-001-impl`. If you cannot find
   it, tell the user and stop.

2. Before you write a handoff, or when the hub says a message waits,
   read your held messages:

   ```sh
   hub inbox demo-kb-001-impl
   ```

   - `no held messages for demo-kb-001-impl`: nothing waits. Go on.
   - `2 held messages for demo-kb-001-impl`: below it, each message
     starts with a line such as `--- 14:05`. Follow each message. If
     you write a handoff, copy each message into it.

3. Before you start a long job, such as a test run, tell the hub's
   watch to wake you when it ends. Start the job so it outlives your
   tool call and writes a summary file at its end, register the wait,
   then end your turn:

   ```sh
   setsid nohup bash /tmp/tests.sh > /tmp/tests.log 2>&1 &
   hub wait demo-kb-001-impl --file /tmp/tests.done --deadline 30
   ```

   - `hub watch prods demo-kb-001-impl once /tmp/tests.done changes
     or at 14:35; end your turn now`: end your turn.
   - A line that starts with `error:`, with exit 2: give exactly one
     of `--file <path>` and `--pid <process ID>`.

   A message that starts with `hub watch: ` is a prod from the watch,
   not from a person. After a wait, it says your wait ended. When you
   sat idle 10 minutes without a report, it asks you to report. Do
   what it says.

4. When your work is done, or you are stuck, write your report to a
   file, such as `/tmp/report-demo-kb-001-impl.md`. Then send it:

   ```sh
   hub report demo-kb-001-impl /tmp/report-demo-kb-001-impl.md
   ```

   For a short report, use standard input instead:

   ```sh
   printf '%s\n' "Done: fixed the typo." "PR: #12" |
     hub report demo-kb-001-impl
   ```

   - `report demo-kb-001-impl written, 2 lines`: the hub has it. End
     your turn.
   - `hub: empty report; nothing written` with exit 3: the text was
     empty. Write the report and run the command again.
   - `hub: Bad/Name is not an agent name` with exit 2: use your name
     exactly as your prompt gives it.

   `hub report` and `hub wait` may print one more line,
   `hub watch restarted`, when the watch had stopped. It needs
   nothing from you.

   A new report replaces your last one whole. Put everything the hub
   needs in it.

## You are a hub

You watch workers and act on their news. Run each command from your
own Herdr pane, with `HUB_RUN` set to your run folder.

1. Check the run folder:

   ```sh
   echo "$HUB_RUN"
   ```

   If it prints a path, go on. If it prints an empty line, find the
   run folder in your handoff file and add `--run <folder>` after the
   command name in every step, such as `hub status --run /tmp/run`. If
   you cannot find it, tell the user and stop.

2. Start the watch once, at the start:

   ```sh
   hub watch start
   ```

   It prints `hub watch started; it watches the workers of <you>`, or
   `hub watch runs; ...` when it already runs. The watch prods idle
   workers and never types into your pane. If it exits 3, tell the
   user its reason once and go on.

3. See every session:

   ```sh
   hub status
   ```

   Read the line that starts with `open` and the line that starts with
   `unread`. In the table, STATE says who acts: `working` needs
   nothing, `hub` waits on you, `you` waits on the user, `kept`,
   `to-close`, `to-park` and `parked` need nothing from you.

4. Wait for news:

   ```sh
   hub next --timeout 100
   ```

   Keep the timeout under your shell tool's limit, often 120 seconds.
   It prints one line and exits 0. Most lines end with your own
   context size, such as `(hub 131k)`. Do what the line says, then
   run step 4 again:

   - `report demo-kb-001-impl (hub 131k)`: run
     `hub read demo-kb-001-impl --note "<what the report says>"`.
     Write the note in a few words, such as `PR #12 ready`. It prints
     the report and ends with `unread:` and the names of other unread
     reports, or `-`. Act on the report. Read each name after
     `unread:` the same way.
   - `missed report demo-kb-001-impl (hub 131k)`: the report waited
     10 minutes. Read it as for a `report` line.
   - `blocked demo-kb-001-impl (hub 131k)`: the worker waits for the
     user at a question or approval. Tell the user. Never answer it
     yourself.
   - `not sent demo-kb-001-impl: <reason> (hub 131k)`: a message did
     not arrive. Run `hub status`. If the worker is still there, send
     the message again as step 5 says. Then delete the `.notsent`
     file for it in `$HUB_RUN/sends/`.
   - `unsure demo-kb-001-impl: <reason> (hub 131k)`: a message may
     have arrived. Read the worker's screen:

     ```sh
     herdr agent read demo-kb-001-impl \
       --source recent-unwrapped --lines 40
     ```

     If the message is not there, delete the
     `demo-kb-001-impl.*.sending` file in `$HUB_RUN/sends/` and send
     the message again.
   - `board over its limit: active 10 of 8, review 18 of 6 (hub 131k)`:
     too many cards are in progress. Tell the user, with the board
     line from `hub status`.
     Start no new work until the user says to go on.
   - `to-close docs-fix: report read; run hub clean`, or the same
     with `to-park`: those sessions finished. Run step 6.
   - `no hub handoff for kanban-hub-3 -> hub adopt kanban-hub-3
     (hub 131k)`: an older hub of this run is still open. Run
     `hub adopt kanban-hub-3`.
   - `hand off now: hub at 152k`: your context is full. Stop the
     loop. Hand off as the herdr-hub skill says, or tell the user.
   - `replaced by a newer hub next`: another `hub next` runs for this
     folder. Do not run step 4 again.
   - `timeout (hub 131k)`: nothing happened. Run step 4 again.

   `hub read` exits 3 with `no unread report from demo-kb-001-impl`
   when you already read it. Go on.

5. To send a worker a message, write it to a file, then send the
   file:

   ```sh
   hub send demo-kb-001-impl --file /tmp/msg.md
   ```

   - `sent demo-kb-001-impl`: it arrived.
   - `held demo-kb-001-impl: <why>`: it waits. `hub next` sends it
     later. Do nothing.
   - `reopening demo-kb-001-impl; held until it starts`: the worker
     was parked. A new tab opens and gets the message.
   - `sent demo-kb-002-impl (demo-kb-001-impl handed off to it)`: the
     worker handed off, and the newer one got it.
   - `hub: not sent demo-kb-001-impl: <reason>` with exit 1: it did not
     arrive. Run `hub status` and check the worker is there.
   - `hub: unsure demo-kb-001-impl: <reason>; see sends/<file>` with
     exit 1: check as for the `unsure` line in step 4, and delete that
     file before you send again.

6. After a `to-close` or `to-park` line, see what would change, then
   close and park:

   ```sh
   hub clean --dry-run
   hub clean
   ```

   It prints `closed NAME: WHY`, `parked NAME: KEY review` or
   `forgot NAME` for each change, `kept NAME: WHY` for each session it
   keeps, and `nothing to close` when nothing changed. Never close a
   `kept` session yourself; its WHY says what closing would lose.

7. To keep a record, add a line to `notes.md`:

   ```sh
   hub note "merged #12"
   ```

   It prints `noted HHMM`. Never write `notes.md` with file tools.
