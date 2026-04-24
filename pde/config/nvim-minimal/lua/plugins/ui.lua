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
          local ok, name = pcall(function() return require("pi.state").get("agent.session_name") end)
          if ok and name and name ~= "" then
            if #name > 30 then name = name:sub(1, 27) .. "..." end
            return "󰑴 " .. name
          end
          return ""
        end,
        cond = function()
          local ok, name = pcall(function() return require("pi.state").get("agent.session_name") end)
          return ok and name ~= nil and name ~= ""
        end,
      },
      "encoding", "fileformat", "filetype",
    },
    lualine_y = { "progress" },
    lualine_z = { "location" },
  },
})

require("bufferline").setup({
  options = {
    diagnostics = "nvim_lsp",
    separator_style = "thin",
    always_show_bufferline = false,
  },
})
