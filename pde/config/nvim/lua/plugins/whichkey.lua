require("which-key").setup({
  delay = 0,
  preset = "helix",
  icons = {
    mappings = false,
  },
  filter = function(mapping)
    return mapping.desc and mapping.desc ~= ""
  end,
  spec = {
    {
      mode = { "n", "x" },
      { "<leader>b",  group = "buffer" },
      { "<leader>c",  group = "code" },
      { "<leader>f",  group = "file/find" },
      { "<leader>g",  group = "git" },
      { "<leader>gh", group = "hunks" },
      { "<leader>s",  group = "search" },
      { "<leader>x",  group = "diagnostics/quickfix" },
      { "<leader>p",  group = "pi agent" },
      { "<leader>q",  group = "session" },
      { "<leader>m",  group = "mason" },
      { "<leader>w",     group = "windows", proxy = "<c-w>" },
      { "<leader><Tab>", group = "tabs" },
      { "[",          group = "prev" },
      { "]",          group = "next" },
      { "g",          group = "goto" },
    },
  },
})

vim.keymap.set("n", "<leader>?", function()
  require("which-key").show({ global = false })
end, { desc = "Buffer keymaps" })
