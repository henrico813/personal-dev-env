# card reference

`card` keeps kanban cards as notes in the Obsidian vault. The code is
`kanban/card.py`; `card --help` and `card COMMAND --help` print the
same options.

## Where cards live
---

A card is the note `500 Zettelkasten/Projects/<Project>/<KEY>
<Title>.md`. The key is the project code, `-KB-`, and a number of at
least three digits, such as `PDEV-KB-012`. The project code comes from
a line in the project folder's `AGENTS.md`:

```markdown
Project code: `PDEV`
```

`card` refuses to make a card in a project with no such line. It finds
the vault with `nobs vault path`; `card --vault NAME COMMAND` uses a
named vault.

## Commands
---

| Command | What it does |
| --- | --- |
| `card new PROJECT TITLE [--kind K] [--user] [--why TEXT]` | Makes a card in `draft` and prints its name. |
| `card new PROJECT --from FILE [--user] [--why TEXT]` | Makes a card in `draft` with the title, kind and section text from FILE, laid out as `card new --help` shows. Refuses any other heading, a repeated heading, and text before the first heading, so no line is lost. |
| `card new PROJECT TITLE --adopt STAGE --user [--why TEXT] [--session [AGENT]]` | Makes a card straight in `active` or `review`, for work already under way. `--session` also records the agent's session, as `card link --session` does. |
| `card list [--stage S] [--project P] [--kind K] [--tabs]` | Prints one line per card: key, stage, kind and title, in stage order. A long line is cut to 60 characters. With `--tabs`, each line holds the key, stage, kind, project folder and whole title, split by tabs, for scripts such as `hub board`. |
| `card board` | Prints the count per stage, active and review each against its limit, and the review queue by `stage_date`, oldest first. |
| `card show KEY [--section S]` | Prints the whole card, or one section with its items numbered from 1. |
| `card grep PATTERN [--section S] [--stage S] [--project P] [--kind K]` | Prints each line that matches a regular expression, in any case, as `KEY Section: line`. Exits 1 when nothing matches. |
| `card move KEY STAGE [--user] [--why TEXT]` | Moves a card, sets `stage_date` to today and adds a Log line. |
| `card move KEY active --session [AGENT]` | Starts a card and records the worker's session in the same write. |
| `card note KEY TEXT` | Adds a dated note to the Log. |
| `card note KEY TEXT --needs-you` | Logs a question, approval or failure that waits on the user. |
| `card note KEY TEXT --answered` | Logs the user's answer. |
| `card add KEY SECTION TEXT` | Adds a bullet to Context, Open Questions or Decisions, or a `- [ ]` item to Acceptance Criteria. |
| `card attach KEY FILE` | Copies FILE to `attachments/<KEY> <file name>` in the card's project folder and adds a `![[...]]` line to Context. Refuses a missing file and never overwrites an attachment. |
| `card edit KEY SECTION N TEXT` | Replaces the words of item N, as `card show KEY --section SECTION` numbers it. A checkbox or bullet stays, with its tick. Logs the old words. |
| `card edit KEY SECTION N --remove` | Removes item N and logs its words. |
| `card set KEY title VALUE` | Changes the title heading and renames the file. |
| `card set KEY kind VALUE` | Changes the kind. |
| `card link KEY PLAN` | Adds `[[PLAN]]` to `plans`. |
| `card link KEY URL` | Adds a pull request URL to `prs`. |
| `card link KEY --session [AGENT]` | Adds a Herdr agent's session to `sessions` and logs it. Without AGENT, it uses the agent in the current pane. |
| `card delete KEY --user` | Deletes the note. Use `dropped` instead to keep a record. |
| `card check` | Lists cards with a broken layout, an unknown stage or kind, or a key two files share. Exits 1 when it finds one. |
| `card template` | Prints the Obsidian template, `001.09 Zettel-Kanban.md`. |

`--user` records that the user decided. The tool cannot tell who
types, so an agent passes it only when the user said so.

An **item** of a section is a line that is not blank or indented,
with the indented lines right after it. `card edit` refuses an item
that changed since `card` read the card, so a hand edit is never
replaced by mistake. Replacing changes only the item's first line;
`--remove` also removes its indented lines. `card edit` takes no Log
items, though each edit adds a note line to the Log.

`new`, `move`, `edit` and plain notes need an author for the Log.
`--by NAME` gives one; otherwise `card` uses the name of the Herdr
agent in its pane. With neither, `card` stops. The user passes
`--by user`.

`--session AGENT` takes a Herdr agent name or pane. `card` reads the
agent's kind and session id from `herdr agent get AGENT`, such as
`opencode ses_e4dc27c20ffeJkm3JZ8dwLh7rW`. A session already in
`sessions` is not added or logged again.

## Stages
---

| Stage | Meaning |
| --- | --- |
| `draft` | Context and Acceptance Criteria are still being written. |
| `ready` | The user released it; it waits for a free work slot. |
| `active` | A worker plans and builds it, or does the research. |
| `review` | The worker finished; it waits for the user to accept. |
| `accepted` | The user accepted it. Final; follow-up work gets a new card. |
| `dropped` | The user decided not to do it. |

## Moves
---

| Move | Who | Needs |
| --- | --- | --- |
| new card -> `draft` | user or staff | |
| `draft` -> `ready` | user | an Acceptance Criteria checkbox |
| `ready` -> `draft` | user or staff | |
| `ready` -> `active` | staff; the user past the active limit | a free slot, unless `--user` |
| `active` -> `ready` | staff | work paused; `--why` names its handoff |
| `active` -> `draft` | user | |
| `active` -> `review` | staff; the user past the review limit | a free slot, unless `--user` |
| `review` -> `active` | user | |
| `review` -> `accepted` | user | |
| any but `accepted` -> `dropped` | user | |
| `dropped` -> `draft` | user | |
| new card -> `active` or `review` | user, by adoption | |

"user" moves need `--user`. "staff" moves are the staff agent's,
and `card` makes them without `--user`. The user can make a staff
move too, with `--user`, so the Log says the user decided. Any move
not in the table is refused. Only `draft` -> `ready` needs a
checkbox, so paused work with no criteria can still go back to
`ready`. The **limits** are 8 cards in `active` and 6 in `review`,
each counted across every project. Adoption skips the limits and the
Acceptance Criteria check.

## Frontmatter
---

```yaml
tags:
  - "#Kanban"
type: card
stage: draft
stage_date: 2026-11-06
template_version: 1
project: DevEnv
kind: build
date_created: 2026-11-06
plans: []
prs: []
sessions: []
topics:
  - "[[DevEnv]]"
```

| Field | Meaning |
| --- | --- |
| `tags` | `#Kanban`; the board finds cards by it. |
| `type` | Always `card`. |
| `stage` | One of the stages above. |
| `stage_date` | The day of the last move, or of creation. The board sorts by it. |
| `template_version` | The layout version, now 1. |
| `project` | The project folder name. |
| `kind` | `build` ends in a change to code, config or the running setup; `research` ends in a document or a decision. |
| `date_created` | The day the card was made. |
| `plans` | Links to plan notes, added by `card link`. |
| `prs` | Pull request URLs, added by `card link`. |
| `sessions` | The agent sessions that worked on the card, as `KIND SESSION_ID`, added by `--session`. |
| `topics` | A link to the project, and any other topics. |

The key has no field. `card` reads it from the file name, and the
title heading repeats it. `card` changes only the fields a command
names and keeps every other line, including fields added by hand.

## Sections
---

A card has these sections, in this order. A new card has the headings
only, with no hint or placeholder text.

| Section | What it holds |
| --- | --- |
| Context | Why does this work matter? Describe the problem, motivation, relevant background, and constraints. Include links to relevant code, documentation, issues, or prior work. |
| Acceptance Criteria | What must be true for this issue to be considered complete? Each item is an observable, verifiable condition; relevant tests and validation pass. |
| Open Questions | What remains unresolved that could affect the implementation or outcome? |
| Decisions | Record significant decisions made while working on this issue, including rationale when useful. |
| Log | Record meaningful progress, findings, and milestones that future contributors need to understand. |

Each acceptance criterion is a checkbox line, and the last one is
usually the tests:

```markdown
- [ ] <observable, verifiable condition>
- [ ] Relevant tests and validation pass.
```

A card moves to `ready` only when Acceptance Criteria holds at least
one checkbox line. `card add KEY "Acceptance Criteria" TEXT` adds one
as `- [ ] TEXT`.

Cards made before this layout have Why, Outcome and "Open questions"
sections, hints between `%%` marks, and no `sessions` field.
`kanban/migrate_sections.py` moves them to this layout; see its
`--help`. A second run changes nothing.

## Log lines
---

Each line starts with `- YYYY-MM-DD`. Text in brackets is optional.

| Written by | Form |
| --- | --- |
| `card new` | `draft: by NAME [for the user][. WHY]` |
| `card new --adopt` | `STAGE: adopted by NAME for the user[. WHY]` |
| `card move` | `STAGE: by NAME [for the user][. WHY]` |
| `card note --needs-you` | `needs-you: TEXT` |
| `card note --answered` | `answered: TEXT` |
| `card note` | `note (NAME): TEXT` |
| `card edit` | `note (NAME): replaced SECTION item N, was "OLD"` or `note (NAME): removed SECTION item N, "OLD"` |
| `--session` | `session: NAME (KIND SESSION_ID)` |
| the Obsidian template | `draft: by the user in Obsidian` |

Cards made before these forms existed start with
`draft: created by NAME`.

## Writes
---

The vault is on a network drive with sync. `card` writes a note only
when its text changes. It writes `.<KEY>.tmp` in the same folder and
renames it over the note. If the note changed since `card` read it,
such as a save in Obsidian, `card` reads it again and redoes its
change once, then stops. `card new` picks the next number under the
lock `~/.local/state/card/new.lock`. The Obsidian template takes no
lock, so `card check` finds a key two cards share.
