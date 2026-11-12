#!/usr/bin/env bash
# Merge a pull request the user reviewed, for a merge worker on a local
# model. Run it from the repository:
#
#   bash merge-worker.sh NUMBER FOLDER COMMAND...
#
# merge_worker.py does the work in its own session, so it keeps going
# when the model's shell tool stops this script. FOLDER/summary.txt
# gets its output, then "exit: <code>" and "done", so the hub reads
# what the script wrote, not the model's words. A second run with the
# same FOLDER starts nothing and waits for the first run's summary.
set -u

if [ $# -lt 3 ]; then
  echo "usage: merge-worker.sh NUMBER FOLDER COMMAND..." >&2
  exit 2
fi
folder=$2
summary=$folder/summary.txt
worker=$(dirname "$0")/merge_worker.py
mkdir -p "$folder" || exit 2
# mkdir fails when the folder exists, so only the first run starts.
if mkdir "$folder/started" 2> /dev/null; then
  setsid bash -c \
    'python3 "$@" > "$0" 2>&1; printf "exit: %s\ndone\n" "$?" >> "$0"' \
    "$summary" "$worker" "$@" < /dev/null > /dev/null 2>&1 &
fi
until [ "$(tail -n 1 "$summary" 2> /dev/null)" = done ]; do
  sleep 1
done
code=$(tail -n 2 "$summary" | sed -n 's/^exit: //p')
exit "${code:-2}"
