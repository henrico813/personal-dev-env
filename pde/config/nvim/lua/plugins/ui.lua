require("lualine").setup({
  options = {
    theme = "tokyonight",
    globalstatus = true,
    component_separators = "|",
    section_separators = "",
    refresh = {
      statusline = 1000,
      tabline = 1000,
      winbar = 1000,
      refresh_time = 100,
      events = {
        "WinEnter",
        "BufEnter",
        "BufWritePost",
        "SessionLoadPost",
        "FileChangedShellPost",
        "VimResized",
        "Filetype",
        "ModeChanged",
      },
    },
  },
  sections = {
    lualine_a = { "mode" },
    lualine_b = { "branch", "diff", "diagnostics" },
    lualine_c = { { "filename", path = 1 } },
    lualine_x = {
      {
        function()
          local ok, status = pcall(function() return require("plugins.obsidian").sync_status() end)
          return ok and status or ""
        end,
      },
      {
        function()
          local ok, cc = pcall(require, "codecompanion")
          if not ok then
            return ""
          end

          local chat = cc.buf_get_chat(0) or cc.last_chat()
          if not chat or not chat.adapter then
            return ""
          end

          local adapter = chat.adapter.name or chat.adapter.formatted_name or "ai"
          local model = chat.adapter.schema and chat.adapter.schema.model and chat.adapter.schema.model.default or nil
          if model and model ~= "" then
            return string.format("󱙺 %s/%s", adapter, model)
          end
          return string.format("󱙺 %s", adapter)
        end,
      },

      "encoding", "fileformat", "filetype",
    },
    lualine_y = { "progress" },
    lualine_z = { "location" },
  },
})

-- jump to the first non-winfixbuf window in the current tab so a
-- subsequent buffer switch doesn't hit locked utility panels
local function ensure_editable_win()
  if not vim.wo.winfixbuf then return end
  for _, win in ipairs(vim.api.nvim_tabpage_list_wins(0)) do
    if not vim.wo[win].winfixbuf then
      vim.api.nvim_set_current_win(win)
      return
    end
  end
end

require("bufferline").setup({
  options = {
    diagnostics = "nvim_lsp",
    separator_style = "thin",
    always_show_bufferline = false,
    left_mouse_command = function(bufnum)
      ensure_editable_win()
      vim.api.nvim_set_current_buf(bufnum)
    end,
    name_formatter = function(buf)
      -- tab-level entries (right indicators)
      if buf.tabnr then
        local named = vim.t[buf.tabnr].name
        if named and named ~= "" then return named end
        for _, win in ipairs(vim.api.nvim_tabpage_list_wins(buf.tabnr)) do
          local ft = vim.bo[vim.api.nvim_win_get_buf(win)].filetype or ""
          if ft:match("^Diffview") then return "Diff" end
          if ft == "codecompanion" or ft == "codecompanion_input" then return "AI" end
        end
      end
      -- buffer-level entries (main strip)
      if buf.bufnr then
        local ft = vim.bo[buf.bufnr].filetype or ""
        if ft:match("^Diffview") then return "Diff" end
        if ft == "codecompanion" or ft == "codecompanion_input" then return "AI" end
      end
      return buf.name
    end,
  },
})

-- name the current tab
vim.keymap.set("n", "<leader><Tab>r", function()
  vim.ui.input({ prompt = "Tab name: ", default = vim.t.name or "" }, function(input)
    if input == nil then return end
    vim.t.name = input
    vim.cmd("redrawtabline")
  end)
end, { desc = "Rename tab" })
