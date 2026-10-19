require("render-markdown").setup({
  file_types = { "markdown", "codecompanion" },
  -- Keeps rendering in insert mode, so lines do not reflow when typing starts.
  render_modes = true,
  -- Shows raw Markdown on the cursor line only while typing, so a long
  -- wrapped paragraph does not reflow each time j or k reaches it.
  anti_conceal = { disabled_modes = { "n" } },
  win_options = { concealcursor = { rendered = "nc" } },
  -- Repeats the quote bar on wrapped rows. Only Markdown buffers get the
  -- indent that leaves room for it, in after/ftplugin/markdown.lua.
  overrides = { filetype = { markdown = { quote = { repeat_linebreak = true } } } },
})
