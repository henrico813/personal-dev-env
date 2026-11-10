#!/usr/bin/env bash
# Run the pull-request skill's merge_prep.py once for a merge worker on
# a local model. Run it from the repository:
#
#   bash merge-worker.sh NUMBER FOLDER COMMAND...
#
# FOLDER/summary.txt gets merge_prep.py's output, then "exit: <code>"
# and "done", so the hub reads what the script wrote, not the model's
# words. merge_prep.py's own files go to FOLDER/prep.
set -u

if [ $# -lt 3 ]; then
  echo "usage: merge-worker.sh NUMBER FOLDER COMMAND..." >&2
  exit 2
fi
number=$1
folder=$2
shift 2
prep=$(dirname "$0")/../../pull-request/scripts/merge_prep.py
mkdir -p "$folder" || exit 2
python3 "$prep" "$number" --out "$folder/prep" "$@" \
  > "$folder/summary.txt" 2>&1
code=$?
printf 'exit: %s\ndone\n' "$code" >> "$folder/summary.txt"
exit "$code"
