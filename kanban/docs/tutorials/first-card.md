# Your first card

In this tutorial you make a card, write down why the work matters and
what done looks like, release it, and see it on the board. It takes
about ten minutes. You need Obsidian open on the vault and a shell
where `card` runs.

A **card** is one vault note that tracks one piece of work, from idea
to the moment you accept it. Each card has a **key**, such as
`PDEV-KB-012`: the project code, `KB` for kanban, and a number. A card
sits in one **stage** at a time, such as `draft` or `ready`, and
changes stage only through a **move**.

## 1. Make the card
---

In the shell, make a card in the DevEnv project:

```sh
card new DevEnv "Try the card tool" --by user --user
```

`card` prints the new card's name, such as `PDEV-KB-005 Try the card
tool`. `--by user` names you as the one who acted, for the card's
Log, its dated history at the bottom. `--user` records that you, not
an agent, decided.

You can make a card in Obsidian instead. Run the Templater command
*Create new note from template*, pick `001.09 Zettel-Kanban`, then
answer the three prompts: project, title and kind. The **kind** is
`build` for work that ends in a change to code, config or the running
setup, or `research` for work that ends in a document or a decision.

## 2. Shape it
---

Open the card in Obsidian. It is in
`500 Zettelkasten/Projects/DevEnv/`. Under **Context**, write why the
work matters: the problem, and links to anything related. Under
**Acceptance Criteria**, write what must be true when it is done, one
checkbox line each, such as:

```markdown
- [ ] `card board` lists the new card under ready.
- [ ] Relevant tests and validation pass.
```

The [reference](../reference/card.md#sections) says what each section
holds. From the shell, you can read a section back:

```sh
card show PDEV-KB-005 --section "Acceptance Criteria"
```

Use the key `card new` printed for you.

## 3. Release it
---

Releasing a card moves it from `draft` to `ready`, which tells the
staff agent the card may start. Only you can release a card, and
`card` refuses until Acceptance Criteria holds at least one `- [ ]`
line.

```sh
card move PDEV-KB-005 ready --by user --user
```

Make moves with `card move`, not by editing `stage` in Obsidian's
Properties panel. A move also sets `stage_date` and adds a Log line,
and an edit in the panel does neither.

## 4. See the board
---

In the shell:

```sh
card board
```

It prints how many cards sit in each stage, and how many are `active`
or in `review` out of the **work limit** of 8. That limit keeps the
work waiting on you small enough to clear. Last comes the review
queue, the cards waiting for you to accept them, oldest first.

In Obsidian, open the note `500 Zettelkasten/Kanban.md`. Its Board
view groups every card by stage, and its Review queue view shows the
same queue as `card board`.

## 5. Clean up
---

This card was practice, so drop it:

```sh
card move PDEV-KB-005 dropped --by user --user
```

A dropped card stays in the vault as a record. To work on it after
all, move it back to `draft`.

Next, look up the other stages and moves in the
[card reference](../reference/card.md).
