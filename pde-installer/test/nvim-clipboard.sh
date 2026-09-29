#!/usr/bin/env bash
# Verifies SSH clipboard selection and local clipboard defaults.
# Needs Neovim 0.12+ (PDE-pinned); CI's apt Neovim lacks the OSC 52 provider.
# Usage: pde-installer/test/nvim-clipboard.sh
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

cat >"$tmp/driver.lua" <<'EOF_LUA'
vim.opt.rtp:prepend(vim.env.NVIM_CONFIG)
dofile(vim.env.NVIM_CONFIG .. "/lua/core/options.lua")

if vim.env.EXPECT_SSH == "1" then
  assert(vim.g.clipboard.name == "osc52", "SSH must select OSC 52")
  assert(type(vim.g.clipboard.copy["+"]) == "function")
  assert(type(vim.g.clipboard.paste["+"]) == "function")

  local output = {}
  vim.api.nvim_ui_send = function(data)
    table.insert(output, data)
  end
  vim.api.nvim_buf_set_lines(0, 0, -1, false, { "osc52 test" })
  vim.cmd("normal! gg0yy")
  local want = "\027]52;c;" .. vim.base64.encode("osc52 test\n") .. "\027\\"
  assert(table.concat(output) == want, "yank must emit the OSC 52 sequence")
  assert(vim.fn.getreg("+") == "osc52 test\n", "yank must update + register")

  local pasted = vim.g.clipboard.paste["+"]()
  assert(pasted[1][1] == "osc52 test", "paste must read unnamed register")
  assert(pasted[2] == "V", "paste must preserve register type")
else
  assert(vim.g.clipboard == nil, "local clipboard must use Neovim default")
end
EOF_LUA
env -u SSH_TTY -u SSH_CONNECTION \
	EXPECT_SSH=0 NVIM_CONFIG="$REPO_ROOT/chezmoi/dot_config/nvim" \
	nvim --clean -l "$tmp/driver.lua"

SSH_CONNECTION="127.0.0.1 22 127.0.0.1 22" \
	EXPECT_SSH=1 NVIM_CONFIG="$REPO_ROOT/chezmoi/dot_config/nvim" \
	nvim --clean -l "$tmp/driver.lua"

echo "nvim clipboard: ok"
