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

-- Client installs no Go, so lint Go files only where go exists.
if vim.fn.executable("go") == 1 then
  lint.linters_by_ft = {
    go = { "golangcilint" },
  }
end

vim.api.nvim_create_autocmd("BufWritePre", {
  pattern = { "*.go" },
  callback = function(args)
    -- Without gopls, as on client machines, skip formatting instead of
    -- warning on every save.
    if #vim.lsp.get_clients({ bufnr = args.buf, method = "textDocument/formatting" }) > 0 then
      vim.lsp.buf.format({ async = false })
    end
  end,
})

vim.api.nvim_create_autocmd("BufWritePost", {
  callback = function() lint.try_lint() end,
})

vim.keymap.set("n", "<leader>cl", function()
  vim.lsp.buf.format({ async = false })
  lint.try_lint()
end, { desc = "Format & lint" })
