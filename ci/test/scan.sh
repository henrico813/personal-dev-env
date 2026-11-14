#!/usr/bin/env bash
# Checks that the scan blocks private names and personal data in each
# place they go public, without printing the names it finds.
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
fail() {
    printf 'scan test: %s\n' "$1" >&2
    exit 1
}

# Makes a repository with only the scanner and its rules, so test commits
# never touch this checkout.
new_repo() {
    repo=$tmp/$1
    git init -q -b main "$repo"
    mkdir "$repo/ci"
    cp "$root/.gitleaks.toml" "$repo/"
    cp "$root/ci/scan" "$repo/ci/"
    git -C "$repo" add -A
    git -C "$repo" -c user.name=Test -c user.email=test@example.com commit -qm start
}

export PRIVATE_TERMS=$'zorblax\nquill pen'
export PDE_PRIVATE_TERMS_FILE=$tmp/missing
# The scan reads settings from the home folder, so the tests use their own.
export HOME=$tmp/home
mkdir -p "$HOME/.config/pde"

# Names often appear inside longer words or wrapped across lines, and the
# output lands in CI logs, so it must not repeat them.
new_repo terms
printf 'see Zorblaxkit # gitleaks:allow\nuse a quill\n  pen\n' >"$repo/notes.md"
git -C "$repo" add notes.md
if (cd "$repo" && bash ci/scan staged) >"$tmp/out" 2>&1; then
    fail "a staged private name must fail"
fi
grep -qxF 'private-term notes.md:1' "$tmp/out" || fail "missed the name inside a word"
grep -qxF 'private-term notes.md:2' "$tmp/out" || fail "missed the wrapped name"
if grep -qiE 'zorblax|quill' "$tmp/out"; then
    fail "the output repeats a private name"
fi

# CI scans the files on main and each commit a pull request adds.
git -C "$repo" -c user.name=Test -c user.email=test@example.com commit -qm "notes on zorblax" -m "more on zorblax"
for scan in tree 'commits HEAD~1 HEAD'; do
    # $scan splits into the mode and its arguments.
    # shellcheck disable=SC2086
    if (cd "$repo" && bash ci/scan $scan) >"$tmp/out" 2>&1; then
        fail "a committed private name must fail $scan"
    fi
    grep -qE '^private-term notes\.md:1( in [0-9a-f]{12})?$' "$tmp/out" || fail "$scan missed the name"
    ! grep -qiE 'zorblax|quill' "$tmp/out" || fail "$scan repeats a private name"
done
# Commit messages go public too. commits ran last, line 3 is the subject and
# line 5 the body.
grep -qxF 'private-term messages:3' "$tmp/out" || fail "missed the name in a commit subject"
grep -qxF 'private-term messages:5' "$tmp/out" || fail "missed the name in a commit body"

# A folder name goes public too, and its path must not reach the logs.
mkdir "$repo/Zorblax-kit"
printf 'a quill pen\n' >"$repo/Zorblax-kit/a.md"
git -C "$repo" add -A
if (cd "$repo" && bash ci/scan staged) >"$tmp/out" 2>&1; then
    fail "a private folder name must fail"
fi
grep -qxF 'private-term-path (path withheld)' "$tmp/out" || fail "missed the private folder name"
! grep -qiE 'zorblax|quill' "$tmp/out" || fail "the output shows a private path"

# Commits once used a personal address as author. GitHub's own committer
# address, which squash merges use, and noreply and example addresses in
# files are fine.
new_repo email
printf 'mail 1+me@users.noreply.github.com or dev@example.com\n' >"$repo/ok.md"
# The home folder is built at run time too, so this file holds none.
printf 'cd /home/%s/src\n' rowan >"$repo/home.md"
git -C "$repo" add ok.md home.md
# Built here, so this file does not trip the scan of the whole tree.
personal=me@mail.$(printf test)
GIT_COMMITTER_EMAIL=noreply@github.com git -C "$repo" -c user.name=Me -c user.email="$personal" commit -qm "add ok"
if (cd "$repo" && bash ci/scan commits HEAD~1 HEAD) >"$tmp/out" 2>&1; then
    fail "a personal author email must fail"
fi
grep -qxF 'personal-email messages:1' "$tmp/out" || fail "missed the author email"
grep -qE '^home-path home\.md:1 in [0-9a-f]{12}$' "$tmp/out" || fail "missed the home folder"
if grep -qE 'ok\.md|messages:2' "$tmp/out"; then
    fail "flagged a noreply or example address"
fi

# A pull request's title and description are public as soon as it opens.
printf 'Title\n\nAbout ZORBLAX\n' >"$tmp/pr.md"
if (cd "$repo" && bash ci/scan text "$tmp/pr.md") >"$tmp/out" 2>&1; then
    fail "a private name in pull request text must fail"
fi
grep -qxF 'private-term text:3' "$tmp/out" || fail "missed the name in pull request text"
! grep -qiE 'zorblax|quill' "$tmp/out" || fail "the output repeats a private name"

# A bad commit range fails as a scan error, not as a clean scan.
status=0
(cd "$repo" && bash ci/scan commits no-such-commit HEAD) >"$tmp/out" 2>&1 || status=$?
[[ $status == 2 ]] || fail "a gitleaks error must exit 2, got $status"

# A missing secret must fail CI rather than skip the names.
status=0
(cd "$repo" && CI=true PRIVATE_TERMS='' bash ci/scan tree) >"$tmp/out" 2>&1 || status=$?
[[ $status == 2 ]] || fail "CI without PRIVATE_TERMS must exit 2, got $status"

# A PC can name a second list of terms in its settings. The scan finds
# those terms like the others and never prints them.
printf '# made-up product names\nfennick\n' >"$tmp/work-terms.txt"
printf '{"work_terms_file": "%s"}\n' "$tmp/work-terms.txt" >"$HOME/.config/pde/config.json"
printf 'Title\n\nClose FENNICK-12 first\n' >"$tmp/pr.md"
if (cd "$repo" && bash ci/scan text "$tmp/pr.md") >"$tmp/out" 2>&1; then
    fail "a work term in pull request text must fail"
fi
grep -qxF 'private-term text:3' "$tmp/out" || fail "missed the work term"
! grep -qi 'fennick' "$tmp/out" || fail "the output repeats a work term"
rm "$HOME/.config/pde/config.json"

# A wrong path to the list must stop the scan, not skip the work terms.
status=0
(cd "$repo" && PDE_WORK_TERMS_FILE=$tmp/no-such-list bash ci/scan text "$tmp/pr.md") >"$tmp/out" 2>&1 || status=$?
[[ $status == 2 ]] || fail "a missing work-term list must exit 2, got $status"

echo "scan: ok"
