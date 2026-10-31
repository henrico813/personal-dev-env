-- The z popup lists only these, to fit a phone. Other z keys work as before
-- when typed.
local z_keys_shown = { zR = true, zM = true, za = true }

require("which-key").setup({
  -- On a phone the popup covers the screen, so it waits for a pause.
  delay = function()
    return require("core.phone_view").is_narrow() and 300 or 0
  end,
  preset = "helix",
  icons = {
    mappings = false,
  },
  filter = function(mapping)
    if mapping.lhs:sub(1, 1) == "z" and not z_keys_shown[mapping.lhs] then
      return false
    end
    return mapping.desc and mapping.desc ~= ""
  end,
  -- Automatic triggers skip single letters other than g and z, so the
  -- Markdown block keys under s need their own.
  triggers = {
    { "<auto>", mode = "nxso" },
    { "s", mode = "n" },
  },
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
      { "<leader>p",   group = "ai/chat" },
      { "<leader>pa",  group = "attach context" },
      { "<leader>q",  group = "session" },
      { "<leader>m",  group = "mason" },
      { "<leader>o",  group = "obsidian" },
      { "<leader>w",  group = "windows", proxy = "<c-w>" },
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
