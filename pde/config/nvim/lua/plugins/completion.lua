require("blink.cmp").setup({
  keymap = { preset = "default" },
  sources = {
    default = { "lsp", "path", "buffer" },
    per_filetype = {
      ["pi-chat-prompt"] = { "pi" },
    },
    providers = {
      pi = { name = "Pi", module = "pi.completion.blink" },
    },
  },
  completion = {
    documentation = { auto_show = true },
    trigger = { show_on_insert_on_trigger_character = false },
    menu = { auto_show = false },
  },
})
