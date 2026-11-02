# How the Cache and the Refresh Work

Reading pull requests on a phone means moving the cursor, opening a screen
and coming back many times. If each of those asked GitHub, each would wait.
`prs` asks GitHub ahead of time and keeps the answers on disk, so moving and
opening screens reads files. The cost is that the list you first see is as
old as the last refresh that worked, until the new one lands.

## Naming Each Entry by Its Update Time

The other screens need more than the list has: the description, the files,
the comments and the diff. `prs` saves these per pull request, in files
named by the pull request's last-update time.

GitHub changes that time on every push, comment and review. So a changed
pull request looks for a file name that does not exist yet and fetches
again, and an unchanged one never does. There is no expiry time to tune, and
an entry can never show a pull request as it was before a change the list
already knows about.

## Fetching Ahead

The PR list screen's `prs` process keeps running while fzf shows the list.
Inside it, the refresher in `internal/app/refresh.go` does the fetching:

1. With a saved list, fzf starts on it at once, and the refresher fetches
   the list again straight away.
1. When the new list lands, the refresher saves it and sends fzf a `reload`
   action through fzf's `--listen` socket. fzf replaces the rows in place,
   and `--id-nth` keeps the cursor on the same pull request.
1. The refresher deletes the entries of pull requests that are no longer
   listed, and older entries of those that are.
1. It fetches the description, files, comments and diff of each listed pull
   request whose entries are missing, 4 at a time, newest first.
1. It waits for `r`, which sends it SIGUSR1, and then does all of this
   again. An `r` during step 4 refreshes the list at once, and the fetch
   then goes on with what is still missing.

With no saved list, `prs` fetches the list before fzf starts, so a GitHub
failure shows as an error rather than as an empty list.

A screen that opens before its entry has landed fetches the entry itself and
saves it, with a loading line such as `Getting the diff of #372`. A very
large pull request's diff can fail in the background, and then the diff
screen fetches it on open and shows gh's reason if that fails too.

## Showing How Old the List Is

While a refresh runs, the prompt shows the list's age, such as
`prs updated 5s ago`, and the refresher updates it every second. When the
refresh works, the age goes away. When it fails, the age stays, the saved
list stays, and the longer details show the reason, from gh or from saving
the list, under `Could not refresh the list.` The status strip has no room
for it, so on a short screen the prompt's age is the sign.

## Writing Safely

You may run `prs` on the desktop and on the phone at the same time, and both
write the same cache. Each file is written under a temporary name and
renamed into place, so a reader never sees half a file. Pruning keeps a
temporary file younger than a minute, since the other `prs` may still be
writing it.
