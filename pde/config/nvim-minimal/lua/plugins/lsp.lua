require("mason").setup()
require("mason-lspconfig").setup({
  ensure_installed = { "lua_ls" },
  automatic_enable = true,
})

vim.keymap.set("n", "<leader>mm", "<cmd>Mason<cr>",        { desc = "Open Mason" })
vim.keymap.set("n", "<leader>mu", "<cmd>MasonUpdate<cr>",  { desc = "Update registry" })
vim.keymap.set("n", "<leader>ml", "<cmd>MasonLog<cr>",     { desc = "Mason log" })

-- LSP buffer keymaps on attach
vim.api.nvim_create_autocmd("LspAttach", {
  callback = function(ev)
    local map = function(mode, lhs, rhs, desc)
      vim.keymap.set(mode, lhs, rhs, { buffer = ev.buf, desc = desc })
    end
    map("n", "gd", vim.lsp.buf.definition, "Go to definition")
    map("n", "gD", vim.lsp.buf.declaration, "Go to declaration")
    map("n", "gr", vim.lsp.buf.references, "References")
    map("n", "gi", vim.lsp.buf.implementation, "Implementation")
    map("n", "K",  vim.lsp.buf.hover, "Hover")
    map("n", "<leader>ca", vim.lsp.buf.code_action, "Code action")
    map("n", "<leader>cr", vim.lsp.buf.rename, "Rename")
    map("n", "<leader>cf", function() vim.lsp.buf.format({ async = true }) end, "Format")
    map("n", "<leader>cd", vim.diagnostic.open_float,                              "Line diagnostics")
    map("n", "<leader>ci", function()
      local buf = ev.buf
      vim.lsp.inlay_hint.enable(not vim.lsp.inlay_hint.is_enabled({ bufnr = buf }), { bufnr = buf })
    end,                                                                           "Toggle inlay hints")
    map("n", "<leader>cw", function() require("fzf-lua").lsp_live_workspace_symbols() end, "Workspace symbols")
    map("n", "<leader>cD", function() require("fzf-lua").lsp_document_symbols() end,       "Document symbols")
    map("n", "[d", vim.diagnostic.goto_prev, "Prev diagnostic")
    map("n", "]d", vim.diagnostic.goto_next, "Next diagnostic")
  end,
})
