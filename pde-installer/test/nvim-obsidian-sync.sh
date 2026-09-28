#!/usr/bin/env bash
# Runs <leader>os in headless Neovim against a fake `ob`.
# Usage: pde-installer/test/nvim-obsidian-sync.sh
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
mkdir -p "$tmp/bin" "$tmp/home" "$tmp/vault"

cat >"$tmp/bin/ob" <<'EOF'
#!/usr/bin/env bash
case "$FAKE_OB" in
ok) exit 0 ;;
fail) echo "Sync failed: boom" >&2; exit 1 ;;
# Like `ob sync` offline: report, retry forever, exit 0 on SIGTERM.
stall) echo "Error: connect ECONNREFUSED" >&2; trap 'exit 0' TERM; while :; do sleep 0.1; done ;;
esac
EOF
chmod +x "$tmp/bin/ob"

cat >"$tmp/driver.lua" <<'EOF'
vim.opt.rtp:prepend(vim.env.NVIM_CONFIG)
package.loaded["fzf-lua"] = {}
local notes = {}
vim.notify = function(msg) table.insert(notes, msg) end

local obsidian = require("plugins.obsidian")
obsidian.sync_timeout_seconds = 1
vim.fn.maparg("<leader>os", "n", false, true).callback()

local early = vim.env.WANT_EARLY
if early and early ~= "" then
  local seen = vim.wait(500, function()
    return table.concat(notes, "\n"):find(early, 1, true) ~= nil
  end, 10)
  if not seen or not obsidian.sync_status():find("syncing") then
    io.stderr:write(("%s: early warning missing while syncing\n%s\n"):format(
      vim.env.FAKE_OB, table.concat(notes, "\n")
    ))
    os.exit(1)
  end
end

vim.wait(5000, function() return not obsidian.sync_status():find("syncing") end, 50)

local status, all = obsidian.sync_status(), table.concat(notes, "\n")
local want_status, want_note = vim.env.WANT_STATUS, vim.env.WANT_NOTE
if not status:find(want_status) or not all:find(want_note, 1, true) then
  io.stderr:write(("%s: status %q, notes:\n%s\n"):format(vim.env.FAKE_OB, status, all))
  os.exit(1)
end
os.exit(0)
EOF

run_case() {
	FAKE_OB="$1" WANT_STATUS="$2" WANT_NOTE="$3" WANT_EARLY="${4:-}" \
		HOME="$tmp/home" PATH="$tmp/bin:$PATH" \
		PDE_MAIN_VAULT="" PDE_WORK_VAULT="$tmp/vault" \
		NVIM_CONFIG="$REPO_ROOT/chezmoi/dot_config/nvim" \
		nvim --clean -l "$tmp/driver.lua"
}

run_case ok "work %d%d:%d%d" "Sync complete: work"
run_case fail "work failed" "Sync failed: work - Sync failed: boom"
run_case stall "work failed" "Sync failed: work - timed out after 1s: Error: connect ECONNREFUSED" \
	"Sync error: work - Error: connect ECONNREFUSED"
echo "nvim obsidian sync: ok"
