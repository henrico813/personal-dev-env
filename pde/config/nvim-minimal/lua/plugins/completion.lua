require("blink.cmp").setup({
  keymap = { preset = "default" },
  sources = {
    default = { "lsp", "path", "buffer" },
  },
  completion = {
    documentation = { auto_show = true },
    trigger = { show_on_insert_on_trigger_character = false },
    menu = { auto_show = false },
  },
})
