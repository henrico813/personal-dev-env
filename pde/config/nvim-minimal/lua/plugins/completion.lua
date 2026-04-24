require("blink.cmp").setup({
  keymap = { preset = "default" },
  sources = {
    default = { "lsp", "path", "buffer" },
  },
  completion = {
    documentation = { auto_show = true },
  },
})
