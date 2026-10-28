# Review Comments

Use this page to decide on each comment an AI wrote or changed in Go
or Python code before it stays. You answer one comment per screen;
after the
last answer, the tool writes your answers into the code, with no AI
involved, and approves the comments, so agents keep your wording.

The full install puts the tool in `~/.local/bin`. Check that it runs:

```bash
pde-comment-review help
```

## 1. Save the proposals

An AI usually does this step with `draft`, then `submit`; its
instructions are in the `comment-review` skill. To try a review by
hand, make a small repository with one Go file:

```bash
mkdir shop && cd shop && git init -b main
cat > cart.go <<'EOF'
package cart

// Sums the prices.
func Total(prices []int) int {
	sum := 0
	for _, p := range prices {
		sum += p
	}
	return sum
}

// Reports whether a price is too high.
func TooHigh(price int) bool { return price > 1000 }
EOF
git add cart.go && git commit -m start
```

Write a proposals file with one item per comment. List every comment
on the package clause, a function, a type or another declaration in
the file that you have not approved as it is now, not only the ones to
change, since the write approves only the comments you answer.
Comments inside function bodies are optional. Run this from the
`shop` folder; it fills in the folder's path:

```bash
cat > ../round-1.json <<EOF
{
  "version": 1,
  "repo": "$PWD",
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
      "old": "// Reports whether a price is too high.\n",
      "proposal": "keep"
    }
  ]
}
EOF
```

`old` is the comment exactly as it is in the file, with its final
`\n`. `proposal` is `keep`, `cut` or `reword`; only `reword` has a
`new`. The AI's skill lists every field: [The proposals
file](../../../ai/skills/comment-review/references/proposals-file.md).

Save it as round 1 of a review called `shop`:

```bash
pde-comment-review add shop ../round-1.json
```

```text
saved round 1 of review shop: shop · main
```

The line ends with the repository and branch the review is for. If an
item is wrong, `add` names it and saves nothing.

## 2. Answer each comment

In Herdr, press Ctrl+b e; it replaces Herdr's scrollback editor. When
several reviews wait, a list opens first: move with `j` and `k`, and
press Enter. In a terminal, run:

```bash
pde-comment-review open shop
```

The screen shows one comment, with the code around it:

```text
shop · main
1/2 reword? cart.go ↑2              ?
>why: Says what it returns and the
   unit.
-// Sums the prices.
+// Returns the sum of prices, in
+// cents.
 func Total(prices []int) int {
   sum := 0
   for _, p := range prices {
     sum += p
  ... 8 more (j)
a accept o keep c cut e edit
m note v view s search q quit
```

Press `a` to take the proposal. The screen moves to `TooHigh`. To ask
the AI for a different proposal, press `m`, type
`say the limit is 1000 cents`, and press Enter. Then press `q`. Each
answer and note is saved at once, so opening the review again starts
at the next unanswered comment.

When the AI removed the code under a comment you approved, the screen
says `Code removed. Drop this comment?`: press `a` or `c` to drop it,
or `m` to ask the AI for the code back.

The other keys are `o` to keep a comment as it is, `c` to cut it, `e`
to write your own wording in nvim, `v` to read the file in nvim (quit
with `:q`), and `s` to search the repository (quit the results with
`q`). [Keys](../reference/commands-files-and-keys.md#keys) says what
each key does and why some wait a moment before they work.

## 3. Send notes back to the AI

You can skip this step: answer `TooHigh` with `o` instead of a note,
and go on to step 4.

Tell the AI the review has notes; it answers them with `status`,
`set` and `submit`. For this review made by hand, read them with
`pde-comment-review notes shop`, write a new proposals file with only
the noted comments, keeping their `id`, and save it as round 2:

```bash
cat > ../round-2.json <<EOF
{
  "version": 1,
  "repo": "$PWD",
  "items": [
    {
      "id": 2,
      "file": "cart.go",
      "old": "// Reports whether a price is too high.\n",
      "proposal": "reword",
      "new": "// Reports whether a price is over the 1000 cent limit.\n",
      "why": "The owner asked for the limit."
    }
  ]
}
EOF
pde-comment-review add shop ../round-2.json
```

Open the review again: only `TooHigh` needs a new answer, and every
other answer stays. Press `a`, the last answer: the tool writes them.

## 4. See what was written

After the last answer, the tool writes your answers into the files,
runs `gofmt` on Go files, and approves each comment you answered in
`.code-comments.json`, with no question first. Code the AI listed as
renamed keeps your approvals under its new name:

```text
shop · main
all 2 answered                   done
 Done. Your answers are in the
   code. d shows the changes.
```

Press `d` to see the changes, in a new tab when you are in Herdr. If
something is wrong, nothing is written and the screen says why. For
example, when an AI edited `TooHigh`'s comment after `add`:

```text
shop · main
all 2 answered            not written
 nothing was written: cart.go: item
   2 changed in the file after it was
   added; ask the AI to send it again
```

Ask the AI to send that comment again in a new round. Your other
answers stay; when you answer the new one, every answer is written.
Opening the review again also tries the write again.

## 5. Commit

The tool does not commit. Review the changes with `git diff` and commit
the changed files. Do not commit `.code-comments.json`: it stays on your
computer. To keep it out of `git status`, run:

```bash
echo .code-comments.json >> .git/info/exclude
```
