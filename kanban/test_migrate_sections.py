"""Tests for moving old cards to the current section layout."""

import migrate_sections

# An old card with both Why and Context, hints, a line that only starts
# with a comment, a two-sentence Outcome and no sessions field.
OLD_CARD = """---
stage: draft
prs: []
topics: []
---

# PDEV-KB-001 Some work
---

## Why
---

%% Who is affected and what happens today. %%
%%todo%% ask about the NAS.
- Agents cut notes short.

## Outcome
---

%% What done looks like. %%
Writes go through a rename. The rule can go.
- Tests pass.

## Context
---

%% Links, files, background and related cards. %%
- Found on 2026-11-06.

## Open questions
---

%% Questions still to answer. %%

## Decisions
---

## Log
---

- 2026-11-06 draft: created by user
"""

NEW_CARD = """---
stage: draft
prs: []
sessions: []
topics: []
---

# PDEV-KB-001 Some work
---

## Context
---

%%todo%% ask about the NAS.
- Agents cut notes short.
- Found on 2026-11-06.

## Acceptance Criteria
---

- [ ] Writes go through a rename.
- [ ] The rule can go.
- [ ] Tests pass.

## Open Questions
---

## Decisions
---

## Log
---

- 2026-11-06 draft: created by user
"""


class TestMigrateText:
    """Old cards move to the new sections with their words kept."""

    def test_migrate_text_moves_old_card_once(self) -> None:
        """An old card gets the new sections and sessions field, once.

        The migration runs on real cards that people wrote by hand, and
        it runs again on cards already moved. Lost words or a doubled
        change would damage notes the sync copies everywhere.
        """
        migrated = migrate_sections.migrate_text(OLD_CARD)

        assert migrated == NEW_CARD
        assert migrate_sections.migrate_text(migrated) == migrated
