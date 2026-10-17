# The Proposals File

An agent writes one proposals file for each round of a review and
saves it with `pde-comment-review add NAME FILE`.

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
| `items[].id` | A number unique in the file. A later round uses the same `id` to replace an item. |
| `items[].file` | A `.go` file, as a plain path inside `repo`, such as `internal/cart/cart.go`. |
| `items[].old` | The whole comment exactly as it is in the file now: every `//` line, its indentation and its final newline. |
| `items[].proposal` | `keep`, `cut` or `reword`. |
| `items[].new` | The new comment lines, for `reword` only. |
| `items[].why` | Optional: one or two sentences the user sees as `why:`. |
| `items[].also_at` | Optional: other places that say the same thing, as `file:line`. |
| `renames` | Optional: code you renamed, as `OLD=NEW` pairs, the way the comment checker's `--rename` takes them. The user decides whether approvals move to the new names. |

## When `add` refuses a file

`add` checks every item against the repository and saves nothing if
one is wrong. It names each problem, for example:

- the name has a capital letter or a space;
- `version` is not 1, or a field name is unknown;
- `file` is not a `.go` file inside `repo`;
- `old` or `new` is not whole `//` lines, or is a directive such as
  `//go:generate`;
- `old` is not found exactly once in its file. For a repeated
  comment the error gives the line of each copy; the skill's step 1
  says what to do;
- `new` is missing for `reword`, set for `keep` or `cut`, or the same
  as `old`;
- an `id` is used twice, or a rename is not `OLD=NEW`;
- two items are the same comment;
- applying would approve a comment in a listed file that the user
  has not seen: a comment the user approved or cut was changed,
  removed or brought back, a comment on a declaration is not an
  item, or a comment has a line with no space after `//`, such as a
  Go directive. The error says what to do for each.

Fix what it names and run `add` again.
