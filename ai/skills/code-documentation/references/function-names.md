# Function names without a verb

Read this before you keep a function or method name that does not
start with a verb, in any language, or when `check-comments` reports
a name under `verb-first`. In Rust, also read
[rust-names.md](rust-names.md), which gives Rust's exact getter
bodies.

## Names that need no verb

On top of tests and the names "Function names" in SKILL.md leaves
out, these names need no verb:

- a test fixture or a Python property
- `main`, and `init` in Go
- a constructor named as its language does, such as `NewServer` in
  Go or `new` in Rust
- a getter, in any language: a method with no arguments whose body
  only returns a field, such as `report.title()`
- a shell function a person types as a command, which is a tool and
  follows "Tool names" in SKILL.md

A method that builds, computes or looks up its result is no getter
and needs a verb, even when it takes no arguments. A name with no
verb tells the caller that the call only reads a value.

```go
// A getter, since it only returns a field.
func (r *Report) Title() string { return r.title }

// Needs a verb, such as BuildPath, since it builds a new value.
func (r *Report) Path() string { return filepath.Join(r.dir, r.name) }
```

`check-comments` passes every Go method with no arguments as a
getter without reading its body, so check each one yourself.

## A reported word

`verb-first` fails a first word that is no English verb. A word that
is a verb but not a common one, such as `screen`, is only reported,
since it is often a noun in the name. A reported name passes the
check, so your reading is all that stops a noun name from merging.

To tell which a reported word is, read the name as a phrase. When
the phrase names what the function returns, such as `screen_lines`,
the word is a noun: rename it, such as `count_screen_lines`. Keep the
name only when the word acts as a verb in it, such as `stage_files`,
and list it in your final message.

## Languages the check cannot read

`check-comments` reads only Go, Python and Rust. In other languages,
such as JavaScript, TypeScript, Lua or shell, check each name you add
or rename yourself. Read it as a phrase, as above. Rename it when its
first word is no verb, or is a noun in the name.
