require("blink.cmp").setup({
  keymap = { preset = "enter" },
  sources = {
    default = { "lsp", "path", "buffer" },
    per_filetype = {
      codecompanion = { "codecompanion" },
      codecompanion_input = { "codecompanion" },
    },
  },
  -- Client machines have no Rust build of the matcher, so they use the Lua
  -- one without a warning.
  fuzzy = { implementation = "prefer_rust" },
  completion = {
    documentation = { auto_show = true },
    trigger = { show_on_insert_on_trigger_character = false },
    menu = { auto_show = false },
    list = { selection = { preselect = false } },
  },
})
