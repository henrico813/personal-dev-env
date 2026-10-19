# Commands, Files, and Keys

## Commands

| Command | What it does |
| --- | --- |
| `pde-comment-review add NAME FILE` | Checks proposals `FILE` against its repository and saves it as the next round of review `NAME`. |
| `pde-comment-review open NAME` | Shows review `NAME` one comment per screen in this terminal. |
| `pde-comment-review notes NAME` | Prints the open notes as JSON, each with the item it is about. |
| `pde-comment-review help` | Prints the commands. |

A review name may use only `a-z`, `0-9`, `.`, `_` and `-`. A command
exits 0 when it worked, 1 on a problem, with a message that starts
with `pde-comment-review:`, and 2 for a command it does not know.

Herdr runs three more commands itself: `popup`, the review screen in
its popup, and `edit` and `diff`, its two tabs. Ctrl+b e runs `open`
with no name. `edit`, `diff` and `open` with no name refuse to run
anywhere else.

The proposals file is described in the AI's skill:
[The proposals file](../../../ai/skills/comment-review/references/proposals-file.md).

## Saved files

Reviews are saved in `$XDG_STATE_HOME/pde/comment-review/`, which is
`~/.local/state/pde/comment-review/` when `XDG_STATE_HOME` is not set.
Each review has its own folder:

| File | Holds |
| --- | --- |
| `NAME/round-N.json` | Round `N`'s proposals, as `add` saved them. |
| `NAME/origin.json` | The repository and branch, and the agent session that added the review. |
| `NAME/decisions.json` | Your answer to each item, and the round it answered. |
| `NAME/notes.json` | Your note on each item, and the round it was written on. |
| `NAME/applied.json` | The round last written into the code. |

An item shows its proposal from the newest round that has it. An
answer or note given on an older round no longer counts for it. To
start a review over, delete its folder.

## Which reviews Ctrl+b e opens

A review waits until its newest round is written. A review whose
repository is gone does not wait. With one review waiting, Ctrl+b e
opens it; with several, it lists them under the agent session that
added them, newest first; with none, Herdr shows "No comment reviews
are waiting".

## Keys

The screen opens at the first comment with no answer. When every
comment is answered, it writes the answers or shows they are written.

| Key | What it does |
| --- | --- |
| `a` | Take the AI's proposal. |
| `o` | Keep the comment as it is. |
| `c` | Cut the comment. |
| `e` | Edit the comment in nvim: in a new tab in Herdr, in place in a terminal. |
| `m` | Type a note for the AI; Enter saves it, and an empty note removes it. |
| `v` | Open the file read-only in nvim at the comment. |
| `s` | Type text and see where it appears in the repository, through `rg`. |
| `j`, `k` | Scroll down or up. |
| `l`, `h` | Next or previous comment. |
| `d` | After the last answer, show the changes: in a new tab in Herdr, in place in a terminal. |
| `q`, Ctrl-c | Close the screen. |

`a`, `o`, `c` and `m` do nothing for 600 ms after a comment appears,
so a key pressed twice on a phone answers only once. Keys typed
before the screen draws, or while nvim or less is open, are dropped.

The top row names the repository and branch. The second row shows
the comment's place, the proposal and the file, `↑N` when `N` rows
are above the screen, and on the right the answer: `?`, `kept`,
`cut`, `accepted`, `edited` or `note`.

## The edit page

`e` shows the comment as plain text: no `//`, one line per paragraph,
and code examples starting with a tab or two spaces. It starts from
your own wording if you have one, then from the AI's reword, then
from the comment. Below the `# ---- edits below are ignored ----` line
is the code after the comment, to read.

When nvim closes, the text is saved as `//` lines, indented like the
old comment and rewrapped. Text with no words cuts the comment.
Unchanged text saves nothing. Nothing is saved, and the screen says
why, when the `# ----` line was deleted or nvim quit with an error.

## What the write checks

Nothing is written, and the screen says why, when:

- a comment has no answer, such as one a new round asked about again;
- a comment's text in the file changed since the review was saved;
- `gofmt` would change a file for reasons other than the comments;
- an agent changed, removed or brought back a comment you approved or
  cut;
- the file has a comment on a declaration, such as the package
  clause, a function or a type, that is not in the review and that
  you have not approved as it is now;
- a listed rename would move no approved comment: its file has no item.

Two comments on declarations with the same exact wording in one file
cannot be written while either copy is unapproved: `add` refuses
them as items, and the write refuses an unapproved copy left out.
Change an unapproved copy's wording before `add`, or leave the whole
file out of the review and read its comments in the pull request. A
doc comment followed by a Go directive, such as `//go:noinline`, can
never be approved either; leave its file out of the review too.

Approval goes through the comment checker of the `code-documentation`
skill, `~/.agents/skills/code-documentation/scripts/check_comments.py`,
which needs `python3`. The checker is the only program that writes
`.code-comments.json`. Every comment in the review's files is
approved, not only the review's items, except comments inside
function bodies, comments with a blank line after them, and comments
that share a name in their file.

## Herdr pieces

| Piece | Comes from |
| --- | --- |
| The plugin `pde.comment-review`: the popup, the edit and diff tabs, and the action Ctrl+b e runs | `chezmoi/dot_local/share/pde/herdr-plugins/pde-comment-review/herdr-plugin.toml.tmpl` |
| Linking the plugin into Herdr after each `chezmoi apply` | `chezmoi/run_after_link_herdr_plugins.sh.tmpl` |
| Ctrl+b e, in place of Herdr's scrollback editor | `chezmoi/dot_config/herdr/config.toml` |
| The program `~/.local/bin/pde-comment-review` | built by `pde-installer` from `pde-comment-review/` |

Herdr shows one popup at a time; close another popup before pressing
Ctrl+b e. When Ctrl+b e does nothing, `herdr plugin log list` shows
why. If the popup has no edit or diff tab after an update, link the
plugin again:

```bash
herdr plugin unlink pde.comment-review
herdr plugin link ~/.local/share/pde/herdr-plugins/pde-comment-review
```

## Where the code lives

| Package | Holds |
| --- | --- |
| `internal/comment` | Everything that knows Go comment syntax. |
| `internal/review` | The proposals file, saved reviews, and building the new files. |
| `internal/popup` | The screen, its keys, and the raw terminal. |
| `internal/cmd` | The commands, the write after the last answer, and every call to Herdr, nvim, rg, less, git, delta and the checker. |
