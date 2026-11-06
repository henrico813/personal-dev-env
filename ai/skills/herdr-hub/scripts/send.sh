#!/usr/bin/env bash
# Send one message to a Herdr agent once it can take it, then wait for
# the turn the message starts to end. Run it in the background:
#
#   bash send.sh [--machine LABEL] NAME RUN_FOLDER MESSAGE
#
# It writes Herdr's replies to RUN_FOLDER/send-NAME.txt, which ends
# with "exit <code>" when the send is over. While that file has no exit
# line, the send is pending and another send to NAME exits 2.
set -u

machine=()
if [ "${1:-}" = --machine ]; then
  machine=(--machine "${2:-}")
  shift 2
fi
if [ $# -ne 3 ]; then
  echo "usage: send.sh [--machine LABEL] NAME RUN_FOLDER MESSAGE" >&2
  exit 2
fi
name=$1
out=$2/send-$name.txt
message=$3

if [ -f "$out" ] && ! grep -q '^exit ' "$out"; then
  echo "a send to $name is pending; see $out" >&2
  exit 2
fi
: > "$out" || exit 1

{
  # A blocked agent rejects every prompt, and a prompt sent mid-turn
  # would make --wait match the end of that turn, so wait for idle.
  herdr "${machine[@]}" agent wait "$name" --until idle --until done \
    --timeout 1800000 &&
    # Wait while the user is in the agent's pane, so the message does
    # not land in the middle of their typing.
    while herdr "${machine[@]}" agent get "$name" |
      jq -e '.result.agent.focused' > /dev/null; do
      sleep 30
    done &&
    herdr "${machine[@]}" agent prompt "$name" "$message" --wait
  echo "exit $?"
} >> "$out" 2>&1
