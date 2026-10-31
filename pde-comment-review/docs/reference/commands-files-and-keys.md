# Commands, Files, and Keys

## Commands

| Command | What it does |
| --- | --- |
| `pde-comment-review status` | Prints the state of the current branch's review as JSON, with the items or notes to act on and what to do next. |
| `pde-comment-review draft` | Saves the comments on the current branch that you must answer as the draft of a new review named after the branch, and prints it as JSON. Refuses while a round waits for your answers. |
| `pde-comment-review set ID ANSWER [TEXT]` | Changes the draft's proposal for item `ID` to `keep`, `cut` or `reword`, with `TEXT` as the new wording in plain sentences. While your notes wait, it changes a noted item for the next round. |
| `pde-comment-review submit` | Saves the draft as the next round of the branch's review, for you to answer. |
| `pde-comment-review add NAME FILE` | Checks proposals `FILE` against its repository and saves it as the next round of review `NAME`. |
| `pde-comment-review open NAME` | Shows review `NAME` one comment per screen in this terminal. |
| `pde-comment-review notes NAME` | Prints the open notes as JSON, each with the item it is about. |
| `pde-comment-review mcp` | Serves `status`, `draft`, `set` and `submit` as the MCP tools `review_status`, `review_draft`, `review_set` and `review_submit`, on stdin and stdout, for an agent harness to start. Each tool takes `dir`, the checkout's absolute path. |
| `pde-comment-review help` | Prints the commands. |

A review name may use only `a-z`, `0-9`, `.`, `_` and `-`. A command
exits 0 when it worked, 1 on a problem, with a message that starts
with `pde-comment-review:`, and 2 for a command it does not know.
`status`, `draft`, `set` and `submit` print their problems in the
JSON instead, unless the home folder is unknown.

Herdr runs four more commands itself. `popup` shows the review screen
in Herdr's popup. `tab` shows the same screen in a Herdr tab. `edit`
and `diff` are the two tabs the popup opens. Ctrl+b e runs `open` with
no name, and Ctrl+b E runs `open --tab`. `tab`, `edit`, `diff`, `open`
with no name and `open --tab` refuse to run anywhere else.

The proposals file is described in the AI's skill:
[The proposals file](../../../ai/skills/comment-review/references/proposals-file.md).

## The MCP server

No harness starts `pde-comment-review mcp` by default yet. To try it,
register it as a local server in a harness's global settings, so
sessions in every worktree see it. For OpenCode, add this to
`~/.config/opencode/opencode.json`, or run
`opencode mcp add --global pde-comment-review -- pde-comment-review mcp`:

```json
{"mcp": {"servers": {"pde-comment-review": {"type": "local", "command": ["pde-comment-review", "mcp"]}}}}
```

For Claude Code, run
`claude mcp add -s user pde-comment-review -- pde-comment-review mcp`,
and for Codex,
`codex mcp add pde-comment-review -- pde-comment-review mcp`.

The server writes only MCP messages to stdout and runs one tool call
at a time. Two servers, or a server and the commands, working on one
branch can still lose a change. `review_draft` from a second clone of
the same branch replaces the first clone's unsent draft, though never
your answers. A line on stdin that is not an MCP message ends the
server, with the error on stderr, and the harness must start it again.

## Saved files

Reviews are saved in `$XDG_STATE_HOME/pde/comment-review/`, which is
`~/.local/state/pde/comment-review/` when `XDG_STATE_HOME` is not set.
Each review has its own folder:

| File | Holds |
| --- | --- |
| `NAME/round-N.json` | Round `N`'s proposals, as `submit` or `add` saved them. |
| `NAME/draft.json` | The draft that `set` changes and `submit` saves, the round it becomes, and the commit `draft` compared the branch with, which a draft answering notes leaves empty. |
| `NAME.N/` | A finished review that `draft` moved aside to start the branch's review again. |
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
are waiting". Ctrl+b E opens the same in a tab named "comment
review". While that tab is open, Ctrl+b E moves to it instead.

## Keys

The screen opens at the first comment with no answer. When every
comment is answered, it writes the answers or shows they are written.

| Key | What it does |
| --- | --- |
| `a` | Take the AI's proposal. |
| `o` | Keep the comment as it is. |
| `c` | Cut the comment. |
| `e` | Edit the comment in nvim: in a new tab from Herdr's popup, in place elsewhere. |
| `m` | Type a note for the AI; Enter saves it, and an empty note removes it. |
| `v` | Open the file read-only in nvim at the comment. |
| `s` | Type text and see where it appears in the repository, through `rg`. |
| `j`, `k` | Scroll down or up. |
| `l`, `h` | Next or previous comment. |
| `d` | After the last answer, show the changes: in a new tab from Herdr's popup, in place elsewhere. |
| `q`, Ctrl-c | Close the screen. |

On a comment whose code was removed, `o`, `e` and `v` only say why
they do nothing.

`a`, `o`, `c` and `m` do nothing for 600 ms after a comment appears,
so a key pressed twice on a phone answers only once. Keys typed
before the screen draws, or while nvim or less is open, are dropped.

The top row names the repository and branch. The second row shows
the comment's place, the proposal and the file, `↑N` when `N` rows
are above the screen, and on the right the answer: `?`, `kept`,
`cut`, `accepted`, `edited` or `note`.

## The edit page

`e` shows the comment as plain text: no `//`, `#` or docstring quotes,
one line per paragraph, and code examples starting with a tab or two
spaces. It starts from your own wording if you have one, then from the
AI's reword, then from the comment. Below the
`# ---- edits below are ignored ----` line is the code after the
comment, to read. For a Python docstring, its `def` or `class` lines
come first.

When nvim closes, the text is saved as `//` or `#` lines or a docstring,
laid out like the wording you started from and rewrapped. Text with no
words cuts the comment. Unchanged text saves nothing. Nothing is saved,
and the screen says why, when the `# ----` line was deleted or nvim quit
with an error. When the checker refuses your text, such as a docstring
holding `"""` or a new backslash escape, the screen says why and keeps
your text. Go back to that item and press `e` to fix it, or quit nvim
with `:cq` to drop it. You cannot cut a docstring that is the only
statement in its function, class or block, because the code would break;
`c` and an empty page cannot cut it.

## What the write checks

Nothing is written, and the screen says why, when:

- a comment has no answer, such as one a new round asked about again;
- a comment's text in the file changed since the review was saved,
  except for an item from `draft` whose `old` is the text you
  approved, where your answer replaces the change;
- an item from `draft` has no comment by its name in its file, or
  its removed code is back;
- `gofmt` would change a Go file for reasons other than the comments;
- a Python answer would change what the code does, such as a new
  backslash escape in a docstring;
- an item is for a comment you cut, or its `old` is not the text you
  approved;
- a listed rename would move no approved comment: its file has no item.

`submit` and `add` refuse an item without a `name` whose comment has
the same wording as another in its file. Change an unapproved copy's
wording first.
After changing code to fix a refusal, run `draft` again and repeat
the `set` changes.
`draft` leaves out a doc comment followed by a Go directive, such as
`//go:noinline`, and it stays unapproved even when you answer it.

Writing your decisions into the files and approving them both go
through the comment checker,
`pde-comment-checker` on `PATH`, which `pde-installer` installs
and which needs `python3`. If the write fails, or the checker gets
Ctrl-C, SIGTERM or SIGHUP before it finishes, the checker tries to
put the files and approvals back. A signal that arrives after the
write finishes does not undo it. The checker cannot put files back
after SIGKILL, or when the file system fails. After a failed run,
check the files with `git -C <repo> status`.
`draft` asks the checker which comments you must answer. Each command
that runs the checker first checks that it speaks this tool's
interface, and refuses one from another install with a hint to run
`pde-installer`.
The checker is the only program that writes
`.code-comments.json`. Only the comments you answered are approved,
and one left out keeps its approval as it was. The checker never
approves a comment inside a function body, one with a blank line
after it, or one that shares a name in its file.

## Herdr pieces

| Piece | Comes from |
| --- | --- |
| The plugin `pde.comment-review`: the popup, the review, edit and diff tabs, and the actions Ctrl+b e and Ctrl+b E run | `chezmoi/dot_local/share/pde/herdr-plugins/pde-comment-review/herdr-plugin.toml.tmpl` |
| Linking the plugin into Herdr and reloading Herdr's config after each `chezmoi apply` | `chezmoi/run_after_link_herdr_plugins.sh.tmpl` |
| Ctrl+b e, in place of Herdr's scrollback editor, and Ctrl+b E | `chezmoi/dot_config/herdr/config.toml` |
| The program `~/.local/bin/pde-comment-review` | built by `pde-installer` from `pde-comment-review/` |

Herdr shows one popup at a time; close another popup before pressing
Ctrl+b e. When Ctrl+b e does nothing, `herdr plugin log list` shows
why. When it opens the scrollback editor instead, run
`herdr server reload-config`. If the popup has no edit or diff tab
after an update, link the plugin again:

```bash
herdr plugin unlink pde.comment-review
herdr plugin link ~/.local/share/pde/herdr-plugins/pde-comment-review
```

## Where the code lives

| Package | Holds |
| --- | --- |
| `internal/review` | The proposals file, saved reviews, and building the new files. |
| `internal/popup` | The screen, its keys, and the raw terminal. |
| `internal/cmd` | The commands, the MCP server, the write after the last answer, and every call to Herdr, nvim, rg, less, git, delta and the checker. |

The code names things differently from the files and messages. An
item is a `FeedbackRequest`, its proposal is its `Suggestion`, and a
round is a `Batch`. Your answer and note on an item are its
`UserFeedback`.
