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
          local ok, data = pcall(function() return require("pi.state").get("sessions.current") end)
          if ok and data and data.sessionName then
            local name = data.sessionName
            if #name > 30 then name = name:sub(1, 27) .. "..." end
            return "󰑴 " .. name
          end
          return ""
        end,
        cond = function()
          local ok, client = pcall(function() return require("pi.state").get("rpc_client") end)
          return ok and client ~= nil
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
