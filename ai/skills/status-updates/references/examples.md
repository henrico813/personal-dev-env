# Examples

One progress update, written first from an agent's shorthand and then
for a new reader. The project is made up.

## Bad

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

## Better

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
3. Pages whose titles differ only by case, such as "Notes" and
   "notes", get the same web address. Should the second get "-2"
   added, or should the build stop? I recommend "-2"; stopping
   blocks a build over a harmless clash. The next three pull
   requests, which add search, wait on this answer.
```
