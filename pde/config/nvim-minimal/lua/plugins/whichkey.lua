require("which-key").setup({
  delay = 300,
  preset = "helix",
  spec = {
    {
      mode = { "n", "x" },
      { "<leader>b",  group = "buffer" },
      { "<leader>c",  group = "code" },
      { "<leader>f",  group = "file/find" },
      { "<leader>g",  group = "git" },
      { "<leader>gh", group = "hunks" },
      { "<leader>s",  group = "search" },
      { "<leader>u",  group = "ui" },
      { "<leader>x",  group = "diagnostics/quickfix" },
      { "<leader>w",  group = "windows", proxy = "<c-w>" },
      { "[",          group = "prev" },
      { "]",          group = "next" },
      { "g",          group = "goto" },
    },
  },
})

vim.keymap.set("n", "<leader>?", function()
  require("which-key").show({ global = false })
end, { desc = "Buffer keymaps" })
