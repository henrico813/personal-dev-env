local lint = require("lint")

-- Use the planner module's pinned golangci-lint so the editor matches CI.
lint.linters.golangcilint.cmd = "go"
lint.linters.golangcilint.args = {
  "tool",
  "golangci-lint",
  "run",
  "--output.json.path=stdout",
  "--issues-exit-code=0",
  "--show-stats=false",
}

lint.linters_by_ft = {
  go = { "golangcilint" },
}

vim.api.nvim_create_autocmd("BufWritePre", {
  pattern = { "*.go" },
  callback = function() vim.lsp.buf.format({ async = false }) end,
})

vim.api.nvim_create_autocmd("BufWritePost", {
  callback = function() lint.try_lint() end,
})

vim.keymap.set("n", "<leader>cl", function()
  vim.lsp.buf.format({ async = false })
  lint.try_lint()
end, { desc = "Format & lint" })
