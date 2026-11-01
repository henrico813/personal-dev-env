---
name: plain-direct-english
description: Writes and reviews English prose so each sentence has one
  clear subject and one main claim, and breaks a hard sentence down part
  by part. Use when writing or reviewing prose such as code comments,
  docstrings, docs, pull request text and plans, or when the user asks
  why a sentence or comment is hard to read. Not needed for a typo or
  spelling fix alone.
---

# Plain Direct English

Write sentences a reader understands on the first pass. A sentence is
hard to read when it packs many conditions onto one main clause or
leaves the reader guessing who does what.

Follow stricter rules from the repository or another loaded skill
first. For code comments, `code-documentation` sets the length, colon
and opening-verb rules, and those win over the steps below.

## Write a clear sentence

1. Name the one fact this sentence gives the reader.
2. Pick one subject, the thing that acts or is described, and put it
   first.
3. Give it one main claim with an active verb, such as "The parser
   rejects ...".
4. Move each extra condition into its own short sentence. If a length
   limit leaves no room, keep the main claim and cut the detail the
   code or context already shows.
5. Use "must" only for a rule the reader has to follow. Describe what
   code does with a plain present verb, such as "rejects" or "allows".
6. Check the result against the patterns in
   [reference/dense-sentence-patterns.md](reference/dense-sentence-patterns.md).

A doc comment may open with a verb and no subject, such as
"Returns ...". Name the subject in every later sentence, because the
sentence before it may have a different subject.

## Break down a hard sentence

When the user asks why a sentence is hard to read, take it apart in
this order:

1. Quote each sentence on its own.
2. Name the main clause and its subject. Say so when the subject is
   missing and what the reader would have to guess.
3. List the modifiers attached to the main clause and what each one
   limits.
4. Note the voice and any modal. Point out passive voice that hides
   who acts and "must" that turns a description into a requirement.
5. Check agreement, such as a singular subject with a plural
   complement, and each "and" or "or" that could join two different
   kinds of thing.
6. Check each claim about code against the code, and report any claim
   the code does not support.
7. Propose a rewrite that keeps every requirement of the original.

Explain each grammar term in plain words the first time you use it.
Keep the answer short, as in this example:

```text
Sentence 2: "Reports a problem for every bad item and rename together"
- Subject: missing. The sentence before is about "Each old", so the
  reader has to guess that the function reports.
- "and": "every bad item and rename" could mean bad renames or every
  rename. The code checks each rename pair, so bad renames.
Rewrite: "The parser reports every bad item and every bad rename in
one run."
```

## Guardrails

- Keep the meaning. When the original can be read two ways, show both
  readings and ask, rather than picking one.
- Add no claim about code that you have not checked.
- Never edit a comment the owner approved. Propose the rewrite through
  the `comment-review` skill.
- Rewrite only the text the user asked about. A typo fix is just the
  fix.
- Make no grammar claim the reference pages do not support. Call a
  choice that is about readability a readability choice.

## When a tool is missing

- If you cannot read the code a sentence describes, say its claims are
  unchecked.
- If `comment-review` is not installed, show the proposed rewrite in
  your reply and leave the comment as it is.

## Final checks

- Each sentence has one subject and one main claim.
- Every sentence after the first names who acts.
- Each subject matches its complement in number.
- Each "and" or "or" joins one kind of thing.
- Passive voice appears only where who acts does not matter.
- "must" marks only real requirements.
- The rewrite keeps every requirement, and its claims match the code.
- Code comments still meet the `code-documentation` limits.

## Reference

- [reference/grammar-terms.md](reference/grammar-terms.md): read when
  you need a term's meaning and an example, such as subject, modifier,
  determiner or passive voice.
- [reference/dense-sentence-patterns.md](reference/dense-sentence-patterns.md):
  read when reviewing or rewriting a sentence, for each pattern that
  makes one dense, with a bad and a better version.

## Remembering corrections

Here "the user" is the human who asked for the work, not a parent
agent. Follow every lesson in the Lessons section below. Each is a
correction the user made, so a lesson wins when it disagrees with the
text above. If the section is empty and `skill-lessons` is installed,
run `skill-lessons render` once, since an install can reset this file,
and read the section again.

When the user corrects how you write or break down a sentence, still
save it to memory as your instructions say. If `skill-lessons` is
installed, also offer once to save it here as a lesson. When the user
says "remember", save it at once and show the one line it prints:

    jq -n --rawfile text /dev/stdin \
      '{skill: "plain-direct-english", text: ($text | rtrimstr("\n"))}' \
      <<'EOF' | skill-lessons add -
    <rule>
    EOF

The rule goes on its own line, so apostrophes, quotes, backticks and
`$` are saved as written. If the user corrects a lesson again, add
`repeat_of: "L<n>"` inside the braces, with the ID from the end of that
lesson. If `skill-lessons` is not installed, say so once and save no
lesson. If the skills folder is read-only, as in a Vibe worker, follow
the lessons shown and save or render nothing. Lessons become rules in
this text only through a pull request the user asks for.

Before you offer or write a lesson, follow
[../lessons.md](../lessons.md).

## Lessons
<!-- skill-lessons: filled from the local store; do not edit -->
