require("persistence").setup()

-- Allow global vars in mksession so SavedTabNames survives
vim.opt.sessionoptions:append("globals")

-- Save tab names and close diffview before session is written
vim.api.nvim_create_autocmd("User", {
  pattern = "PersistenceSavePre",
  callback = function()
    local names = {}
    for i, t in ipairs(vim.api.nvim_list_tabpages()) do
      local n = vim.t[t].name
      if n and n ~= "" then names[i] = n end
    end
    vim.g.SavedTabNames = names

    pcall(vim.cmd, "DiffviewClose")
  end,
})

-- Reapply tab names after restore
vim.api.nvim_create_autocmd("User", {
  pattern = "PersistenceLoadPost",
  callback = function()
    local names = vim.g.SavedTabNames or {}
    for i, t in ipairs(vim.api.nvim_list_tabpages()) do
      if names[i] then vim.t[t].name = names[i] end
    end
    vim.cmd("redrawtabline")
  end,
})

local map = vim.keymap.set
map("n", "<leader>qs", function() require("persistence").load() end,               { desc = "Restore Session" })
map("n", "<leader>ql", function() require("persistence").load({ last = true }) end, { desc = "Restore Last Session" })
map("n", "<leader>qd", function() require("persistence").stop() end,               { desc = "Don't Save Current Session" })
