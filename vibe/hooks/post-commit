#!/usr/bin/env bash
set -euo pipefail

[[ "${VIBE_COMMIT_KIND:-}" == "snapshot" ]] || exit 0
[[ -n "${VIBE_SNAPSHOT_LOG:-}" ]] || exit 0

sha="$(git rev-parse HEAD)"
printf '{"sha":"%s"}\n' "$sha" >> "$VIBE_SNAPSHOT_LOG"
