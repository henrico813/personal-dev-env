require("lualine").setup({
  options = {
    theme = "tokyonight",
    globalstatus = true,
    component_separators = "|",
    section_separators = "",
  },
  sections = {
    lualine_a = { "mode" },
    lualine_b = { "branch", "diff", "diagnostics" },
    lualine_c = { { "filename", path = 1 } },
    lualine_x = {
      {
        function()
          local ok, count = pcall(function() return require("pi").attention_count() end)
          return (ok and count and count > 0) and ("󱆅 " .. count) or ""
        end,
        cond = function()
          local ok, visible = pcall(function() return require("pi").is_visible() end)
          return ok and visible
        end,
      },
      "encoding", "fileformat", "filetype",
    },
    lualine_y = { "progress" },
    lualine_z = { "location" },
  },
})

-- jump to the first non-winfixbuf window in the current tab so a
-- subsequent buffer switch doesn't hit pi's locked panels
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
  },
})
