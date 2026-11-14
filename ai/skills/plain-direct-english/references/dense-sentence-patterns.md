# Dense sentence patterns

The patterns that make a sentence hard to read, each with a bad and a
better version. The bad versions come from a Go doc comment on a
function that reads a proposals file and checks it. The better
versions of a code comment still meet the `code-documentation` limits
of two sentences and no colon. Each better version fixes only its own
pattern.

## Contents

- Many modifiers on one main clause
- A singular subject with a plural complement
- An "and" that joins two kinds of thing
- A missing subject in a later sentence
- Passive voice that hides who acts
- "must" where the text describes behavior

## Many modifiers on one main clause

One main clause carries every condition, so the reader must hold them
all before the sentence ends.

Bad:

```text
Each old must be whole "//" lines found exactly once in its file,
starting a line, with no comment line just above or below; a Go
directive just above or below is allowed.
```

Better, in prose with no length limit:

```text
Each old text is a block of whole "//" lines. It appears exactly once
in its file and starts a line. The parser rejects it when a comment
line sits just above or below it, but allows a Go directive there.
```

Better, as a Go comment, keeping the main claim and cutting detail:

```go
// Rejects an old text that is not whole "//" lines. The parser also
// rejects an unnamed item's old text unless it appears exactly once in
// its file with no comment line just above or below.
```

## A singular subject with a plural complement

The subject and its complement differ in number, so the reader cannot
tell how many things one subject covers.

Bad:

```text
Each old must be whole "//" lines.
```

Better:

```text
Each old text must be a block of whole "//" lines.
```

## An "and" that joins two kinds of thing

The second item after "and" can be read as one more of the first kind
or as a separate thing.

Bad:

```text
Reports a problem for every bad item and rename together.
```

Better:

```text
The parser reports every bad item and every bad rename in one run.
```

## A missing subject in a later sentence

An opening verb with no subject, such as "Returns ...", is fine in a
doc comment. When a later sentence drops its subject, the reader has
to guess who acts, because the sentence before may have a different
subject.

Bad:

```text
Each old must be whole "//" lines found exactly once in its file.
Reports a problem for every bad item and rename together.
```

Better:

```text
Each old text must be whole "//" lines found exactly once in its file.
The parser reports every bad item and every bad rename in one run.
```

## Passive voice that hides who acts

The thing acted on becomes the subject, and the actor drops out. Keep
passive voice only when the reader does not need to know who acts.

Bad:

```text
A Go directive just above or below is allowed.
```

Better:

```text
The parser allows a Go directive just above or below.
```

## "must" where the text describes behavior

"must" turns a description into a requirement. In a comment on the
code that checks a rule, it reads as an order to the reader rather
than as what the code does.

Bad:

```text
Each old must be a whole line.
```

Better:

```text
Rejects an old text that is not a whole line.
```

Keep "must" for a rule the reader has to follow, such as in docs for
the person who writes the proposals file.
