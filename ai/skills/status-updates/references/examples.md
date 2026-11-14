# Examples

Messages for made-up projects: a progress update written first from
an agent's shorthand and then for a new reader, a recap after time
away, a list of pull requests to review, and a correction. Each good
example passes `scripts/check_update.py`.

## Contents

- A progress update
- A recap after time away
- A list of pull requests to review
- A correction

## A progress update

### Bad

```text
Update:
- P2 #18 done, CI green, approval filed
- P3 needs review, see wt
- batch 2 blocked on the slug thing
- also could clean up old spike branch?
Let me know if you have questions!
```

The reader has to know what P2, P3, "wt", "batch 2" and "the slug
thing" are. The approval has no deadline, the review has no path, the
side item is raised again, and the message ends with a closing offer.

### Better

```text
Inkwell, the tool that turns a notes folder into a small website:

Changed
- Broken links now fail the build instead of showing a blank page
  (#18). 22 of 22 tests pass. Not tried on a folder over 1,000
  notes.

Waiting on you
1. Approve #18 in the approval app. It expires in about two hours.
2. Review the comments on the tag-page change (#19). In
   ~/src/inkwell/wt/tag-pages, run:
   rev 19

**3. Page addresses: what if two titles differ only by case?**
- A: add "-2" to the second page's address **(rec)**
- B: stop the build

Reply like `3A`. Pages such as "Notes" and "notes" now get the same
address. Stopping blocks a build over a harmless clash. The next
three pull requests, which add search, wait on this.
```

## A recap after time away

A recap has three parts: done, in progress, and waiting on you.

```text
Fernwood, the app that waters the garden on a timer, since Monday:

Done
- Skip watering when rain is forecast (#7): merged Tuesday.

In progress
- A separate schedule for each garden bed (#8): 18 of 19 tests
  pass. The failing test covers a bed with no plants; I am fixing
  it. Not tried on the real valve controller yet.

Waiting on you
1. Approve #9, which texts you when a valve sticks, in the approval
   app. The request expires in about three hours.

**2. Bed schedules: what does a bed with no plants get?**
- A: no watering, which saves water **(rec)**
- B: half the watering time, which keeps new seeds damp

Reply like `2A`. #8 finishes within a day either way.
```

## A list of pull requests to review

Number the pull requests in the order they should merge. Each entry
gives its repository, what it fixes for the user in one line, its
check results and anything that holds it up. Bold the entry the user
should review first. A plan or file comes with a command that opens
that file itself, never the card that links it.

````text
Ready for your review, in merge order:

1. **fernwood: skip watering when rain is forecast (#7).** 14 of 14
   tests pass. Nothing blocks it.
2. fernwood: a separate schedule for each garden bed (#8). 19 of 19
   tests pass. Merge it after #7; both change the timer file. Read
   its plan with:

   ```sh
   less ~/notes/plan.md
   ```

3. valve-kit: a text when a valve sticks (#3). Not tried on the real
   valve controller. It waits for #8 to merge.
````

The `text` fence only marks the example. Send the update itself as
plain Markdown, so the checker reads only the commands.

## A correction

Correct a wrong earlier claim in its own line, apart from the news:

```text
Correction: yesterday I said the tests cover Windows paths. They do
not; only Linux paths ran.
```
