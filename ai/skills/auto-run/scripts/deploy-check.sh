# Run each try line of deploy.checks in the PR folder, in order, in
# one scratch folder, and write one pass or FAIL line per line to
# deploy.txt. The script, not the model that runs it, writes the last
# line, done, only after every line passed. So a failed try leaves no
# done line. Each command's output goes to deploy.log.
. "$(dirname "$0")/env" || exit 1
grep '^try ' "$D/deploy.checks" > "$D/deploy.lines" ||
  { echo "no try lines in $D/deploy.checks"; exit 1; }
work=$(mktemp -d) || exit 1
echo "folder: $work" > "$D/deploy.txt" || exit 1

# Run COMMAND with sh in the scratch folder. It passes when it exits 0
# and, when TEXT is given, its output holds TEXT.
try() {
  local name=$1 command=$2 text=${3-} out rc
  out=$(cd "$work" && sh -c "$command" < /dev/null 2>&1)
  rc=$?
  printf '## %s\n%s\n' "$name" "$out" >> "$D/deploy.log"
  if [ "$rc" -ne 0 ]; then
    echo "FAIL $name: exit $rc"
  elif [ -n "$text" ] && ! printf '%s\n' "$out" | grep -qF -e "$text"; then
    echo "FAIL $name: output lacks $text"
  else
    echo "pass $name"
    return 0
  fi
  return 1
}

status=0
while IFS= read -r line <&3; do
  eval "$line" >> "$D/deploy.txt" 2>> "$D/deploy.log" ||
    { status=1; break; }
done 3< "$D/deploy.lines"
[ "$status" -ne 0 ] || echo done >> "$D/deploy.txt"
cat "$D/deploy.txt"
exit "$status"
