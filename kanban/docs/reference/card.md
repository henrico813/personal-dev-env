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
| `card new PROJECT TITLE --adopt STAGE --user [--why TEXT]` | Makes a card straight in `active` or `review`, for work already under way. |
| `card list [--stage S] [--project P] [--kind K]` | Prints one line per card: key, stage, kind and title, in stage order. |
| `card board` | Prints the count per stage, active plus review against the work limit, and the review queue by `stage_date`, oldest first. |
| `card show KEY [--section S]` | Prints the whole card, or one section. |
| `card grep PATTERN [--section S] [--stage S] [--project P] [--kind K]` | Prints each line that matches a regular expression, in any case, as `KEY Section: line`. Exits 1 when nothing matches. |
| `card move KEY STAGE [--user] [--why TEXT]` | Moves a card, sets `stage_date` to today and adds a Log line. |
| `card note KEY TEXT` | Adds a dated note to the Log. |
| `card note KEY TEXT --needs-you` | Logs a question, approval or failure that waits on the user. |
| `card note KEY TEXT --answered` | Logs the user's answer. |
| `card add KEY SECTION TEXT` | Adds a bullet to Why, Outcome, Open questions or Decisions. |
| `card set KEY title VALUE` | Changes the title heading and renames the file. |
| `card set KEY kind VALUE` | Changes the kind. |
| `card link KEY PLAN` | Adds `[[PLAN]]` to `plans`. |
| `card link KEY URL` | Adds a pull request URL to `prs`. |
| `card delete KEY --user` | Deletes the note. Use `dropped` instead to keep a record. |
| `card check` | Lists cards with a broken layout, an unknown stage or kind, or a key two files share. Exits 1 when it finds one. |
| `card template` | Prints the Obsidian template, `001.09 Zettel-Kanban.md`. |

`--user` records that the user decided. The tool cannot tell who
types, so an agent passes it only when the user said so.

`new`, `move` and plain notes need an author for the Log. `--by NAME`
gives one; otherwise `card` uses the name of the Herdr agent in its
pane. With neither, `card` stops. The user passes `--by user`.

## Stages
---

| Stage | Meaning |
| --- | --- |
| `draft` | Why and Outcome are still being written. |
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
| `draft` -> `ready` | user | an Outcome |
| `ready` -> `draft` | user or staff | |
| `ready` -> `active` | staff; the user past the work limit | a free slot, unless `--user` |
| `active` -> `draft` | user | |
| `active` -> `review` | staff | |
| `review` -> `active` | user | |
| `review` -> `accepted` | user | |
| any but `accepted` -> `dropped` | user | |
| `dropped` -> `draft` | user | |
| new card -> `active` or `review` | user, by adoption | |

"user" moves need `--user`. "staff" moves are the staff agent's,
and `card` makes them without `--user`. Any move not in the table is
refused. The **work limit** is 8 cards in `active` plus `review`,
counted across every project. Adoption skips the limit and the
Outcome check.

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
| `topics` | A link to the project, and any other topics. |

The key has no field. `card` reads it from the file name, and the
title heading repeats it. `card` changes only the fields a command
names and keeps every other line, including fields added by hand.

## Sections
---

Why, Outcome, Open questions, Decisions and Log, in that order. Hints
between `%%` marks do not count as text, so a card whose Outcome holds
only its hint cannot move to `ready`.

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
