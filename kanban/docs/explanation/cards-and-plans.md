# Cards, plans and the staff agent

The user runs about 30 coding-agent sessions in Herdr, a terminal
workspace manager. Herdr shows whether an agent is working or has
stopped, but a stopped agent may have finished, died mid-task, or be
waiting on a question. Nothing recorded which of these it was, or
whether the user had accepted the work. Cards are that record.

A card and a plan answer different questions, so they are separate
notes. A **card** is like a GitHub issue: it says why some work
matters and what done looks like, and its stage tracks the work from
idea to the user's acceptance. A **plan**, written with the `planner`
tool, is like a pull request: exact, checked code changes for one
piece of the work. A card may need no plan, such as a research card,
or several. Plans keep their strict checks, and cards stay loose
enough for the user to edit by hand in Obsidian.

The **staff agent** will be one Herdr session, named `general-staff`,
that runs the board. It starts a worker for the oldest `ready` card
when a slot is free and checks a worker's pull request and tests
before it moves the card to `review`. It brings the user one item at
a time: a card to accept, a question a worker asked, or a pull request
write waiting for approval. It keeps no memory of its own. Each time
it wakes, it reads the cards, Herdr's session list and the pending
approvals again, so a restart loses nothing. The user keeps every
decision about what to build and whether finished work is accepted.

The full design, with the problem it solves and the alternatives it
rejected, is the vault note
`500 Zettelkasten/Projects/DevEnv/DESIGN-Kanban Cards and General Staff.md`.
