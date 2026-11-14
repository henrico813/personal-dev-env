---
name: plain-direct-english
description: Writes and reviews English prose so each sentence has one
  clear subject and one main claim, and breaks a hard sentence down part
  by part. Use when writing or reviewing prose such as code comments,
  docstrings, docs, pull request text and plans, or when the user asks
  why a sentence or comment is hard to read. Not needed for a typo or
  spelling fix alone.
metadata:
  scope: personal
---

# Plain Direct English

Write sentences a reader understands on the first pass. A sentence is
hard to read when it packs many conditions onto one main clause or
leaves the reader guessing who does what. Stricter rules from the
repository or another loaded skill win, such as the comment rules of
`code-documentation`.

## Write a clear sentence

1. Name the one fact this sentence gives the reader.
2. Put one subject first: the thing that acts or is described.
3. Give it one main claim with an active verb, such as "The parser
   rejects ...".
4. Move each extra condition into its own short sentence. If a length
   limit leaves no room, keep the main claim and cut the detail the
   code or context already shows.
5. Use "must" only for a rule the reader has to follow. Describe what
   code does with a plain present verb, such as "rejects" or "allows".
6. Check the result against the patterns in
   [references/dense-sentence-patterns.md](references/dense-sentence-patterns.md).
   They also cover a doc comment that opens with a verb.

## Write for a reader new to the work

Assume the reader did not follow the task and knows none of its
labels. Explain each project term, short name or ID the first time a
text uses it, or write plain words in its place. Write "the pull
request that skips copied files", not "P2".

## Cut AI filler

Cut filler words, such as "robust" and "ensure". Cut hedges, praise
and closing offers. Cut a restated question and a summary of what the
text above already says. Cut emojis and the words "canonical" and
"contract".
[references/ai-filler.md](references/ai-filler.md) shows a bad and a
better version of each; read it when you are not sure a phrase is
filler.

Run the checker on each draft message, doc or pull request text
before you send it, and fix each line it prints. It cannot find a
restated question or a summary, so look for those yourself. If it
cannot run, check the list above yourself. Leave out the file to read
stdin:

```bash
python3 ~/.agents/skills/plain-direct-english/scripts/check_sentences.py draft.md
```

## Guardrails

- Keep the meaning and every requirement. When the original reads
  two ways, show both readings and ask, rather than picking one.
- Add no claim about code that you have not checked. When you cannot
  read the code a sentence describes, say the sentence is unchecked.
- Never edit a comment the owner approved. Propose the rewrite through
  `comment-review`, or in your reply when that skill is not installed.
- Rewrite only what the user asked about. A typo fix is just the fix.
- When the user gives you wording for a comment, doc or PR text, use
  it as written. Change it only to fix a factual error, such as a
  wrong parameter name, or to pass a checker, and tell the user each
  change. If it breaks a rule no checker enforces, keep it and name
  the rule once. If it can be read two ways, ask.

## Final checks

- No filler is left. No sentence has a pattern from
  [dense-sentence-patterns.md](references/dense-sentence-patterns.md).
- The checker printed nothing, or your reply says it did not run.

## Reference

- [references/sentence-breakdown.md](references/sentence-breakdown.md):
  read when the user asks why a sentence is hard to read.
- [references/grammar-terms.md](references/grammar-terms.md): read
  when you need a grammar term's meaning and an example.
- [references/dense-sentence-patterns.md](references/dense-sentence-patterns.md):
  read when writing or reviewing a sentence, for each pattern that
  makes one dense, with a bad and a better version.
- [references/evaluation.md](references/evaluation.md): read only when
  testing or revising this skill.

## Remembering corrections

Follow every lesson in the Lessons section below. Each is a correction
the user made, so a lesson wins when it disagrees with the text above.
When the user corrects how you write or break down a sentence, follow
[skill-lessons](../skill-lessons/SKILL.md) with the skill name
`plain-direct-english`, and offer once to save the correction as a
lesson.

## Lessons
<!-- skill-lessons: filled from the local store; do not edit -->
