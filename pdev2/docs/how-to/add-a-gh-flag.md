# Add a New gh Flag

Use this when a new `gh` release adds a flag to `gh pr edit`. If the flag can
come before the PR number and is not listed, the approver loses the diff of
the current and requested title and body.

## Why It Matters

To show that diff, the program must find the PR number or branch in the
command. It skips flags it knows, along with their values. When it meets an
unknown flag before the PR number, it cannot tell whether the next argument
is that flag's value or the PR, so it does not guess. The review then shows
`Cannot show current values: the PR number or branch is unclear.` The write
still works; only the diff is lost.

## 1. Check the Flag

```bash
gh pr edit --help
```

Note whether the new flag takes a value, such as `--add-label name`, or not,
such as `--remove-milestone`.

## 2. Add It to the List

In `parse.go`, add the flag to the `"edit"` entry of `targetFlags`: `true` if
it takes a value, `false` if not. Add its short form too, if it has one.

If the flag sets the title, body, body file, or repository, add it to
`valueFlags` with that kind instead. `parse` reads those values for every
operation, and the target search already skips them.

## 3. Add a Test Case

Add a case to `TestEditTargetSkipsFlagValues` in `parse_test.go` that puts the
new flag and a value before the PR number, for example:

```go
{name: "add watcher", args: []string{"--add-watcher", "sam", "12"}, target: "12"},
```

Run the test from `pdev2/`:

```bash
go test -run TestEditTargetSkipsFlagValues ./...
```

It fails with `unknown = true` until the flag is listed, then passes.
