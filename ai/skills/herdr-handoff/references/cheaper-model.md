# Prepare Work for a Cheaper Model

Long reviews and audits repeat one loop many times, such as "read the
ticket, check it against the rules, write a verdict". The cheap
model, as `model-routing` picks, can run that loop when the judgment,
the facts and the tool steps live in files. This page moves them
there, out of the conversation.

When the user wants the session reviewed first, follow the
`session-review` skill.

## 1. Prepare the work

Make one change at a time. Before each one, show the user what it
adds, where, and why, then wait for a yes.

1. Write the judgment as a short decision table, since a small model
   follows a table better than prose:

   | Finding | Verdict |
   | --- | --- |
   | Accuracy fix | Out of scope |
   | Broken link | Fix now |

2. Gather the facts for each remaining item into a small packet: its
   source text, linked records and open questions. Start the packets
   with one line that says what the items are and in which order to
   do them.
3. Script each tool sequence you ran more than twice. Give the script
   a preview run that writes nothing, and a run that writes only
   after the user approves. Test the preview run before you save the
   script. First check where each tool can run, since some writes
   work only through an MCP server and not from a script.
4. Link each source at a fixed version, such as a Git tag or commit.
   Never point to `/tmp`, which gets wiped.
5. Save tool behavior you learned as a lesson in the skill that owns
   the tool, so the next model knows it without this session.

Put all of it in a Handoff section of the work's own log, such as its
vault note. Keep that section readable on a phone. The handoff file
points to that section instead of copying it.

## 2. Hand it over

- Give the user the one-line prompt that starts the next session,
  such as "Continue the audit from the Handoff section of <note>."
- To start that session in Herdr, go back to
  SKILL.md and follow Write the handoff file and Start the session.
- Write every message to the user by the `status-updates` skill.
