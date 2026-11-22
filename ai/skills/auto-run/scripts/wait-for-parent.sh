# Ask GitHub every minute for the pull request of the branch PARENT.
# Once it exists, write its state and the time to parent.txt, such as
# `OPEN 2026-01-01T09:00:00Z`. After PARENT_POLL_TRIES tries with no
# pull request, 240 by default, write `NONE <time>` instead. A worker
# waits on that file with `hub wait --file`, as build.md says.
. "$(dirname "$0")/env" || exit 1
{ [ -n "$W" ] && [ "$W" != null ]; } ||
  { echo "no worktree: '$W'"; exit 1; }
cd "$W" || exit 1
tries=0
# Retry on any gh failure, since gh fails until the parent opens.
# timeout ends a hung call, so the script still stops after its last try.
until state=$(timeout 60 gh pr view "$PARENT" --json state --jq .state) &&
  [ -n "$state" ]; do
  tries=$((tries + 1))
  [ "$tries" -lt "${PARENT_POLL_TRIES:-240}" ] || { state=NONE; break; }
  sleep "${PARENT_POLL_SECONDS:-60}"
done
printf '%s %s\n' "$state" "$(date -u +%FT%TZ)" > "$D/parent.new" &&
  mv "$D/parent.new" "$D/parent.txt"
