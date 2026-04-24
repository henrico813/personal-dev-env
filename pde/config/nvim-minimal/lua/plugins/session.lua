require("persistence").setup()

-- Only persist the current tab, drop empty buffers, keep globals for tab names
vim.opt.sessionoptions:remove("tabpages")
vim.opt.sessionoptions:remove("blank")
vim.opt.sessionoptions:append("globals")

-- Wipe unnamed scratch buffers (alpha dashboard, [No Name], etc.) before load
vim.api.nvim_create_autocmd("User", {
  pattern = "PersistenceLoadPre",
  callback = function()
    for _, buf in ipairs(vim.api.nvim_list_bufs()) do
      if vim.api.nvim_buf_is_valid(buf) and vim.api.nvim_buf_get_name(buf) == "" then
        pcall(vim.api.nvim_buf_delete, buf, { force = true })
      end
    end
  end,
})

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

    -- If the current buffer has no filename, try to switch to a real file buffer
    -- so mksession records an actual edit instead of a stray [No Name].
    if vim.api.nvim_buf_get_name(0) == "" then
      for _, buf in ipairs(vim.api.nvim_list_bufs()) do
        if vim.api.nvim_buf_get_name(buf) ~= "" and vim.bo[buf].buflisted then
          pcall(vim.api.nvim_set_current_buf, buf)
          break
        end
      end
    end
  end,
})

-- Reapply tab names after restore, and land on a real file if possible
vim.api.nvim_create_autocmd("User", {
  pattern = "PersistenceLoadPost",
  callback = function()
    local names = vim.g.SavedTabNames or {}
    for i, t in ipairs(vim.api.nvim_list_tabpages()) do
      if names[i] then vim.t[t].name = names[i] end
    end
    vim.cmd("redrawtabline")

    -- If the active window shows an unnamed buffer, jump to the most recently
    -- accessed listed file buffer so the user lands on real content.
    if vim.api.nvim_buf_get_name(0) == "" then
      local candidates = {}
      for _, buf in ipairs(vim.api.nvim_list_bufs()) do
        if vim.api.nvim_buf_get_name(buf) ~= "" and vim.bo[buf].buflisted then
          table.insert(candidates, buf)
        end
      end
      if #candidates > 0 then
        local empty = vim.api.nvim_get_current_buf()
        vim.api.nvim_set_current_buf(candidates[#candidates])
        pcall(vim.api.nvim_buf_delete, empty, { force = true })
      end
    end
  end,
})

local map = vim.keymap.set
map("n", "<leader>qs", function() require("persistence").load() end,               { desc = "Restore Session" })
map("n", "<leader>ql", function() require("persistence").load({ last = true }) end, { desc = "Restore Last Session" })
map("n", "<leader>qd", function() require("persistence").stop() end,               { desc = "Don't Save Current Session" })
