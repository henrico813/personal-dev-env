#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
checker="$script_dir/check-message"
temporary="$(mktemp -d)"
trap 'rm -rf "$temporary"' EXIT

subject50="fix: $(printf '%*s' 45 '' | tr ' ' x)"
subject51="fix: $(printf '%*s' 46 '' | tr ' ' x)"
body72="$(printf '%*s' 72 '' | tr ' ' x)"
body73="$(printf '%*s' 73 '' | tr ' ' x)"

printf '%s\n' "$subject50" >"$temporary/subject50"
bash "$checker" "$temporary/subject50"

printf '%s\n' "$subject51" >"$temporary/subject51"
if bash "$checker" "$temporary/subject51"; then
  printf '%s\n' 'expected 51-character subject rejection' >&2
  exit 1
fi

printf 'fix: wrap body\n\n%s\n' "$body72" >"$temporary/body72"
bash "$checker" "$temporary/body72"

printf 'fix: wrap body\n\n%s\n' "$body73" >"$temporary/body73"
if bash "$checker" "$temporary/body73"; then
  printf '%s\n' 'expected 73-character body rejection' >&2
  exit 1
fi
