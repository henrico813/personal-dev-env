#!/usr/bin/env bash
# Loads the Obsidian config in headless Neovim with a fake `nobs` and checks
# the vault list, the old-settings warning, <leader>os and the note pickers.
# Needs Neovim 0.10+ for vim.system; CI installs the PDE-pinned release.
# Usage: pde-installer/test/nvim-obsidian-sync.sh
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
mkdir -p "$tmp/bin"

# Called as `nobs --vault NAME sync-now`. The vault name picks the outcome.
# Every sync first prints Docker-style progress on stderr, as nobs does when
# it starts a stopped broker.
cat >"$tmp/bin/nobs" <<'EOF'
#!/usr/bin/env bash
echo "$*" >>"$NOBS_CALLS"
echo "Container nobs-$2-nobs-1  Started" >&2
case "$2" in
broken) echo "obsidian sync is not configured" >&2; exit 5 ;;
slow) exec sleep 10 ;;
killed) kill -KILL $$ ;;
esac
EOF
chmod +x "$tmp/bin/nobs"

# Presses each key in KEYS, waits for WANT_RESULTS sync reports, and writes
# every notification to $NOTES and each picker's folder and command to $PICKS.
cat >"$tmp/driver.lua" <<'EOF'
vim.opt.rtp:prepend(vim.env.NVIM_CONFIG)
local picks = {}
package.loaded["fzf-lua"] = {
  files = function(opts) table.insert(picks, opts.cwd .. "\t" .. opts.cmd) end,
}
local notes = {}
vim.notify = function(msg) table.insert(notes, msg) end

local obsidian = require("plugins.obsidian")
obsidian.sync_timeout_ms = 300
for key in (vim.env.KEYS or ""):gmatch("%S+") do
  vim.fn.maparg(key, "n", false, true).callback()
end

local want = tonumber(vim.env.WANT_RESULTS or "0")
vim.wait(5000, function()
  local done = 0
  for _, note in ipairs(notes) do
    if note:match("^Sync complete: ") or note:match("^Sync failed: ") then
      done = done + 1
    end
  end
  return done >= want
end, 20)

local out = assert(io.open(vim.env.NOTES, "w"))
out:write(table.concat(notes, "\n"), "\n")
out:close()
local picked = assert(io.open(vim.env.PICKS, "w"))
picked:write(table.concat(picks, "\n"), "\n")
picked:close()
os.exit(0)
EOF

# Starts each case with a new HOME, so no real vault list is read.
new_home() {
	home="$tmp/home-$1"
	rm -rf "$home"
	mkdir -p "$home"
	: >"$tmp/calls"
}

# Writes a nobs settings file to $1 listing the vault names that follow.
write_nobs_settings() {
	local file="$1" port=8781 sep=""
	shift
	mkdir -p "$(dirname "$file")"
	{
		printf '{"vaults": {'
		for name in "$@"; do
			printf '%s"%s": {"path": "%s/%s", "port": %d}' "$sep" "$name" "$tmp" "$name" "$port"
			sep=", "
			port=$((port + 1))
		done
		printf '}}\n'
	} >"$file"
}

write_old_settings() {
	mkdir -p "$home/.config/pde"
	printf '{"main_vault": "%s/main"}\n' "$tmp" >"$home/.config/pde/config.json"
}

# XDG is the XDG_CONFIG_HOME to pass; when empty, it is unset.
run_nvim() {
	local xdg=(-u XDG_CONFIG_HOME)
	if [ -n "${XDG:-}" ]; then
		xdg=("XDG_CONFIG_HOME=$XDG")
	fi
	env "${xdg[@]}" HOME="$home" PATH="$tmp/bin:$PATH" NOBS_CALLS="$tmp/calls" NOTES="$tmp/notes" PICKS="$tmp/picks" \
		NVIM_CONFIG="$REPO_ROOT/chezmoi/dot_config/nvim" \
		nvim --clean -l "$tmp/driver.lua"
}

fail() {
	echo "nvim obsidian sync: $1" >&2
	echo "--- notifications:" >&2
	cat "$tmp/notes" >&2
	echo "--- nobs calls:" >&2
	cat "$tmp/calls" >&2
	exit 1
}

expect_note() {
	grep -qxF "$1" "$tmp/notes" || fail "missing notification: $1"
}

# Prints the files that picker number $1 would list.
picker_files() {
	local cwd cmd
	IFS=$'\t' read -r cwd cmd < <(sed -n "$1p" "$tmp/picks")
	(cd "$cwd" && eval "$cmd")
}

# A computer with no vaults must open Neovim silently; this is how every new
# computer starts.
new_home zero
run_nvim
[ -z "$(tr -d '\n' <"$tmp/notes")" ] || fail "zero vaults must not notify"

# After `nobs vault set`, the old pde settings stay until they are deleted
# later, so the warning must stop once the nobs file exists. A sync whose
# broker had to start prints Docker's output on stderr and still succeeds.
# XDG_CONFIG_HOME is relative, so, like nobs, Neovim must use ~/.config.
new_home one
write_old_settings
write_nobs_settings "$home/.config/nobs/config.json" notes
XDG=relative/xdg KEYS="<leader>os" WANT_RESULTS=1 run_nvim
if grep -qF "pde/config.json" "$tmp/notes"; then
	fail "warned after nobs vault set"
fi
expect_note "Sync complete: notes"
grep -qxF -- "--vault notes sync-now" "$tmp/calls" || fail "notes was not synced"

# Vault names are not fixed, and every vault is synced. A failed sync shows
# nobs's own error, not Docker's progress. The file is found through an
# absolute XDG_CONFIG_HOME, as nobs finds it.
new_home three
write_nobs_settings "$tmp/xdg-three/nobs/config.json" alpha broken gamma
XDG="$tmp/xdg-three" KEYS="<leader>os" WANT_RESULTS=3 run_nvim
expect_note "Sync complete: alpha"
expect_note "Sync failed: broken - obsidian sync is not configured"
expect_note "Sync complete: gamma"
[ "$(wc -l <"$tmp/calls")" -eq 3 ] || fail "want one sync per vault"

# A computer that has not moved its vaults to nobs yet would otherwise lose
# its vault list without a word. Pressing keys must not repeat the warning.
new_home old
write_old_settings
KEYS="<leader>os <leader>oo <leader>os" run_nvim
[ "$(grep -c "nobs vault set NAME PATH for each one, then restart Neovim" "$tmp/notes")" -eq 1 ] ||
	fail "want the old-settings warning exactly once"

# A broker that never answers must not leave the sync running forever. A
# killed nobs is reported as code 0 with a signal, and must not look complete.
new_home slow
write_nobs_settings "$home/.config/nobs/config.json" killed slow
KEYS="<leader>os" WANT_RESULTS=2 run_nvim
expect_note "Sync failed: slow - timed out after 0.3s"
grep -q "^Sync failed: killed" "$tmp/notes" || fail "a killed nobs must fail"

# The main vault's .gitignore skips every note, and a phone has no Alt-i to
# show ignored files, so the pickers must list them anyway. .obsidian stays
# out. The space in "work notes" checks that <leader>oo quotes each path.
new_home pickers
write_nobs_settings "$home/.config/nobs/config.json" main "work notes"
git init -q "$tmp/main"
printf '*\n!*/\n!.gitignore\n' >"$tmp/main/.gitignore"
mkdir -p "$tmp/main/.obsidian" "$tmp/work notes"
touch "$tmp/main/plan.md" "$tmp/main/.obsidian/app.json" "$tmp/work notes/todo.md"
KEYS="<leader>om <leader>oo" run_nvim
picker_files 1 >"$tmp/main-files"
grep -qxF "plan.md" "$tmp/main-files" || fail "<leader>om must list ignored notes"
if grep -qF "app.json" "$tmp/main-files"; then
	fail "<leader>om must leave out .obsidian"
fi
picker_files 2 >"$tmp/all-files"
grep -qxF "$tmp/main/plan.md" "$tmp/all-files" || fail "<leader>oo must list the main vault"
grep -qxF "$tmp/work notes/todo.md" "$tmp/all-files" || fail "<leader>oo must list the work vault"

echo "nvim obsidian sync: ok"
