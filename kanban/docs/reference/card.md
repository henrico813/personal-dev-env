# card reference

`card` keeps kanban cards as notes in the Obsidian vault. The code is
`kanban/card.py`; `card --help` and `card COMMAND --help` print the
same options.

## Where cards live
---

A card is the note `500 Zettelkasten/Projects/<Project>/<KEY>
<Title>.md`. The key is the project code, a hyphen and a number of at
least three digits, such as `PDEV-516`. Done and dropped cards keep
older keys such as `PDEV-KB-012`, which `card` still reads; `card
rekey` gives open cards new keys. A plan written for a card
takes the card's key, so the two notes share a key. `card` counts a
note as a card only when its `type` field is `card`. The hub and
sweep read a new key only when its project code has exactly four
characters, as `PDEV` has. The project code comes from a line in the
project folder's `AGENTS.md`:

```markdown
Project code: `PDEV`
```

`card` refuses to make a card in a project with no such line. It finds
the vault with `nobs vault path`; `card --vault NAME COMMAND` uses a
named vault. The shape of the key and the file name comes from the
[naming rules](#naming-rules).

## Commands
---

| Command | What it does |
| --- | --- |
| `card new PROJECT TITLE [--kind K] [--user] [--why TEXT]` | Makes a card in `draft` and prints its name. Refuses a title that breaks a [title rule](#title-rules). |
| `card new PROJECT --from FILE [--user] [--why TEXT]` | Makes a card in `draft` with the title, kind and section text from FILE, laid out as `card new --help` shows. Refuses any other heading, a repeated heading, and text before the first heading, so no line is lost. |
| `card new PROJECT TITLE --adopt STAGE --user [--why TEXT] [--session [AGENT]]` | Makes a card straight in `active`, `in-review` or `blocked`, for work already under way. `blocked` needs `--why`. `--session` also records the agent's session, as `card link --session` does, but skips its check for a session on another card. |
| `card list [--stage S] [--project P] [--kind K] [--tabs]` | Prints one line per card: key, stage, kind and title, in stage order. A long line is cut to 60 characters. With `--tabs`, each line holds the key, stage, kind, project folder and whole title, split by tabs, for scripts such as `hub board`. |
| `card board` | Prints the count per stage, active and in-review each against its limit, and the in-review cards by `stage_date`, oldest first. When a card is blocked, it also prints their count and the oldest wait, and lists them, longest wait first, with days waited and the reason. See [Board](#board). |
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
| `card set KEY title VALUE` | Changes the title heading and renames the file. Refuses a title that breaks a [title rule](#title-rules). |
| `card set KEY kind VALUE` | Changes the kind. |
| `card link KEY PLAN` | Adds `[[PLAN]]` to `plans`. |
| `card link KEY URL` | Adds a pull request URL to `prs`. |
| `card link KEY --session [AGENT]` | Adds a Herdr agent's session to `sessions` and logs it. Without AGENT, it uses the agent in the current pane. Refuses a session already on another card, unless this card holds it too. A session that serves a second card, such as a merge worker's, logs its work there with `card note KEY2 TEXT` instead. |
| `card merged URL` | Moves each open card whose `prs` holds URL to `done`, or logs why it stays. See [Moves](#moves). |
| `card delete KEY --user` | Deletes the note. Use `dropped` instead to keep a record. |
| `card rekey [--apply --user]` | Gives each open card with an old key, such as `PDEV-KB-012`, the next new key of its project, such as `PDEV-523`. It prints one `OLD NEW` line per card and one `relink PATH` line per note whose links change. Done and dropped cards keep their keys. Without `--apply`, it writes nothing. `--apply` needs `--user`. It prints its backup folder under `~/.local/state/card`, backs up each note before writing it, renames the cards and adds `old_key`, then points links in every note under Projects to the new names, and prints an `undo` command. Undo puts the notes the rekey wrote back as they were before it, so it drops any edit made to them since. After two runs, run the newer undo command first. It stops at a note that changed since it read it, still printing the undo command; run rekey again to finish, or run the undo command. Refuses while a Herdr agent on this computer works on one of the cards, by its name or a session in `sessions`, when Herdr cannot list its agents, when two cards hold one key as key or `old_key`, when a new name is taken, or when a card was written but not renamed; then run the last undo command and rekey again. A second run renames the cards still on old keys and fixes links still on their old names. |
| `card check` | Lists cards with a broken layout, an unknown stage or kind, a key two cards share, a note named like an old `-KB-` card without `type: card`, a note with a new card name and a `stage` field but no `type`, a session two cards share, or an open card whose title breaks a [title rule](#title-rules). Exits 1 when it finds one. |
| `card name RULE VALUE...` | Prints the name a naming rule makes. The VALUEs fill the rule's fields in order, and the last field takes the rest: `card name branch feat retry uploads` prints `feat/retry-uploads`. Refuses a name the rule would not read back, such as one with an unknown branch type. Needs no vault. |
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
`card merged` needs neither; its Log lines name `card merged`.

`--session AGENT` takes a Herdr agent name or pane. `card` reads the
agent's kind and session id from `herdr agent get AGENT`, such as
`opencode ses_e4dc27c20ffeJkm3JZ8dwLh7rW`. A session already in
`sessions` is not added or logged again.

## Stages
---

| Stage | Meaning |
| --- | --- |
| `draft` | Context and Acceptance Criteria are still being written. |
| `todo` | The user or an agent released it; it waits for a free work slot. |
| `active` | A worker plans and builds it, or does the research. |
| `in-review` | The worker finished; it waits for the user to accept. |
| `blocked` | It waits only on people outside the team, such as another team's reviewers. It has no limit. |
| `done` | The user accepted it. Final; follow-up work gets a new card. |
| `dropped` | The user decided not to do it. |

## Moves
---

| Move | Who | Needs |
| --- | --- | --- |
| new card -> `draft` | user or staff | |
| `draft` -> `todo` | user or staff | an Acceptance Criteria checkbox and a title that passes the [title rules](#title-rules) |
| `todo` -> `draft` | user or staff | |
| `todo` -> `active` | staff; the user past the active limit | a free slot, unless `--user` |
| `active` -> `todo` | staff | work paused; `--why` names its handoff; a title that passes the [title rules](#title-rules) |
| `active` -> `draft` | user | |
| `active` -> `in-review` | staff; the user past the in-review limit | a free slot, unless `--user` |
| `in-review` -> `active` | user | |
| `in-review` -> `done` | user | |
| `active` or `in-review` -> `blocked` | staff | `--why` naming who it waits on |
| `blocked` -> `in-review` | staff | |
| `blocked` -> `active` | staff; the user past the active limit | a free slot, unless `--user` |
| `blocked` -> `done` | user | |
| any but `done` -> `dropped` | user | |
| `dropped` -> `draft` | user | |
| new card -> `active` or `in-review` | user, by adoption | |
| new card -> `blocked` | user, by adoption | `--why` naming who it waits on |

"user" moves need `--user`. "staff" moves are the staff agent's,
and `card` makes them without `--user`. The user can make a staff
move too, with `--user`, so the Log says the user decided. When no
single move reaches the stage, `card move` makes the shortest chain
of moves the caller may make, through `draft`, `todo`, `active`
and `in-review`, with one Log line each. So `active` -> `done`
with `--user` goes through `in-review`. Each move in the chain is
checked on its own, and a refused one stops the chain before the
card changes. When only the user can finish the chain, an agent's
move is refused with the move it can make, such as
`card move KEY in-review`. A stage no chain reaches, such as any
stage after `done`, is refused. Only `draft` -> `todo` needs a
checkbox, so paused work with no criteria can still go back to
`todo`. The **limits** are 8 cards in `active` and 6 in `in-review`,
each counted across every project. `blocked` cards count against
neither. Adoption skips the limits and the Acceptance Criteria
check.

`card merged URL` runs after a merge the user approved, so it moves
cards to `done` as the user's move. It looks at each open card whose
`prs` holds URL. Two spellings of one pull request match, such as
`http` against `https`, or a URL ending in `/files`. A card moves
only when every Acceptance Criteria box is ticked and gh, the GitHub
CLI, says each other pull request in `prs` merged. It moves along
the chain `card move --user` takes, so `active` goes through
`in-review`. Otherwise the card keeps its stage, and a Log note says
what holds it; a rerun for the same merge adds no second note. When
gh fails or a card cannot be written, it leaves that card unchanged,
goes on to the next and exits 1. A pull request counts only when its
URL is in `prs`, which `card link KEY URL` adds; nothing adds it when
the pull request opens.

## Title rules
---

A title reads like a commit title, with no label or key. `card`
checks the title alone, without the card's own key.

| Rule | Fails when |
| --- | --- |
| T1 length | The title is over 50 characters. |
| T2 label | One of the first three words ends in `:`, such as `Board:`. |
| T3 key | A word is a card or plan key in any case, such as `DEMO-KB-004`, `DEMO-41` or `ISO-8601`. |
| T4 form | The title starts with a lower-case letter, ends with a period, or holds a character Obsidian refuses in file names: `\ / : * ? " < > \| # ^ [ ]`. A label's colon counts for T2 only. |

`card new` and `card set KEY title` refuse a title that breaks a
rule, and list every broken rule at once:

```text
card: title refused:
  T1 length: 53 characters, at most 50
  T2 label: "Board:"
Write it as a commit title of at most 50 characters, such as "Retry failed uploads".
```

When a title passes, `card new` and `card set` print only the card's
name. `card check` lists every broken rule of each card not in `done`
or `dropped`. Each move into `todo` refuses a title that breaks a
rule, also when it is one step of a chain. `--user` skips no rule.

## Board
---

`card board` fits each line in 40 columns, for a phone. The hub
reads the limits line, the `blocked` line and the `BLOCKED` list,
so their words stay as shown:

```text
draft 4  todo 2  active 3
in-review 2  blocked 2
done 9  dropped 1
active 3 of 8, in-review 2 of 6
blocked 2, oldest 8 days

IN-REVIEW, oldest first (2)
- DEMO-KB-001 2026-11-02
  Fix the docs
- DEMO-KB-002 2026-11-10
  Speed up search

BLOCKED, oldest first (2)
- DEMO-KB-003 8 days
  Retry failed uploads
  platform team's review
- DEMO-KB-009 1 day
  Add a cache
  security sign-off
```

Days are whole days since `stage_date`. The third line of a
blocked card is the `--why` of its newest move into the
stage, left out when there is none. The `blocked` line and
list appear only when a card is in the stage.

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
| `old_key` | The key before `card rekey`, such as `PDEV-KB-012`. `card` still finds the card by it. |

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

A card moves to `todo` only when Acceptance Criteria holds at least
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
| `card merged` | `STAGE: by card merged for the user[. URL merged]` |
| `card merged`, card stays | `note (card merged): URL merged, but the card stays STAGE: REASONS` |
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
change once, then stops. `card new` picks the next number while it
holds the lock `~/.local/state/card/new.lock`. The number is one more
than the highest number of any card or plan with the project code, old
keys included. `card` looks for them in every folder under Projects.
The Obsidian template takes no lock, so `card check` finds
a key two cards share.

## Naming rules
---

`card` makes and reads names with the rules in `names.toml`. It reads
`$PDE_NAMES_FILE` when set, else the copy in its own checkout,
`chezmoi/dot_config/pde/names.toml`, so its code and rules match right
after a `git pull`. Without that copy it reads
`~/.config/pde/names.toml`, which `pde-installer install` puts there.
With no file, every command stops with
`no naming rules at ~/.config/pde/names.toml: run pde-installer install`.

Each rule has a `make` template, such as `{key|lower}-{role}`, and a
`parse` pattern that reads a name back:

```toml
[agent]
make = "{key|lower}-{role}"
parse = '^(?P<key>[a-z][a-z0-9]*-(?:kb-)?[0-9]{3,})-(?P<role>[a-z0-9-]+)$'
```

`card name agent PDEV-516 build` fills that template and prints
`pdev-516-build`.

`card name RULE` takes the rules `card_key`, `card_file`, `plan_file`,
`branch`, `commit_title`, `agent`, `hub` and `review_agent`. The
comments at the top of `names.toml` give the filters, the limits on
patterns, and the step after changing `card_key` or `card_file`.
`card` reads only `version = 1`.
