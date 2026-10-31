# The Proposals File

Agents prepare a branch's review with `draft`, `set` and `submit`.
A proposals file is for a change those cannot make. Save it as the
next round with `pde-comment-review add NAME FILE`, where `NAME` is
the `review` the JSON shows.

## Format

```json
{
  "version": 1,
  "repo": "/home/me/src/shop",
  "items": [
    {
      "id": 1,
      "file": "cart.go",
      "old": "// Sums the prices.\n",
      "proposal": "reword",
      "new": "// Returns the sum of prices, in cents.\n",
      "why": "Says what it returns and the unit."
    },
    {
      "id": 2,
      "file": "cart.go",
      "old": "\t// Prices are in cents, so sums never round.\n",
      "proposal": "keep"
    }
  ],
  "renames": ["Sum=Total"]
}
```

## Fields

| Field | Meaning |
| --- | --- |
| `version` | Must be `1`. |
| `repo` | The repository's absolute path. Every round of a review uses the same one. |
| `items[].id` | A number unique in the file. A later round uses an `id` again only to replace that item; `add` refuses an `id` an earlier round used for another comment. |
| `items[].file` | A `.go` or `.py` file, as a plain path inside `repo`, such as `internal/cart/cart.go`. |
| `items[].old` | The whole comment exactly as it is in the file now: every `//` or `#` line, or the whole docstring with its quotes, with its indentation and its final newline. On an item with a `name`, it may be the approved text, which the file no longer has. |
| `items[].proposal` | `keep`, `cut` or `reword`. |
| `items[].new` | The new comment lines, for `reword` only. |
| `items[].why` | Optional: one or two sentences the user sees as `why:`. |
| `items[].also_at` | Optional: other places that say the same thing, as `file:line`. |
| `items[].name` | Set by `draft`, or by you on a `code_removed` item: the comment checker's name for the comment, which the write finds it by. Keep it when you copy an item. |
| `items[].code_removed` | Marks a comment whose code was removed, so the user can only drop it. `draft` sets it; set it yourself when `add` says an approved comment is gone because you deleted its code. Needs `name` and `cut`; leave it out once the code is back. |
| `renames` | Optional: code you renamed, as `OLD=NEW` pairs, the way the comment checker's `--rename` takes them. Each moves its approval when the answers are written; `add` refuses one that would move no approved comment, because its file has no item. |

## When `add` refuses a file

`add` checks every item against the repository and saves nothing if
one is wrong. It names each problem, for example:

- the name has a capital letter or a space;
- `version` is not 1, or a field name is unknown;
- `file` is not a `.go` or `.py` file inside `repo`;
- `old` or `new` is not whole `//` or `#` lines or one whole
  docstring, or is a directive such as `//go:generate`;
- `new` adds a backslash escape, such as `\d`, to a docstring that
  is not raw. Double the backslash;
- `old` of an item without a `name` is not found exactly once in its
  file. For a repeated comment the error gives
  the line of each copy; the skill's step 4 says what to do;
- an item with a `name` has no comment by that name in its file, or
  its removed code is back;
- `new` is missing for `reword`, set for `keep` or `cut`, or the same
  as `old`;
- `gofmt` would change a Go file whose comment an item cuts or rewords.
  Run `gofmt -w` on it first;
- an `id` is used twice, or a rename is not `OLD=NEW`;
- two items are the same comment;
- a rename would move no approved comment, because its file has no
  item. Add an item for that comment in its file;
- an item is for a comment the user cut, or its `old` is not the
  text the user approved. The error says what to do.

Fix what it names and run `add` again.
