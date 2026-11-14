# Break down a hard sentence

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
[grammar-terms.md](grammar-terms.md) gives each term's meaning and an
example, such as subject, modifier, determiner or passive voice. Make
no grammar claim that it and
[dense-sentence-patterns.md](dense-sentence-patterns.md) do not
support. Call a choice about readability a readability choice.
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
