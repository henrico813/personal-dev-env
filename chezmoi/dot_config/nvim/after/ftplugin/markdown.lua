vim.opt_local.wrap = true
vim.opt_local.linebreak = true
-- Wrapped rows continue under the text of a list item, a quote or a diff
-- line, so render-markdown's repeated quote bar does not cover letters.
vim.opt_local.breakindent = true
vim.opt_local.breakindentopt = "list:-1"
vim.opt_local.formatlistpat = [[^\%(\s*>\s*\|\s*[-*+]\s\+\|\s*\d\+[.)]\s\+\|[-+ ]\s*\)]]

vim.keymap.set("n", "zj", function()
  require("core.phone_markdown").jump_to_block(1)
end, { buffer = true, desc = "Next code block" })
vim.keymap.set("n", "zk", function()
  require("core.phone_markdown").jump_to_block(-1)
end, { buffer = true, desc = "Previous code block" })
-- Two lowercase keys next to each other, easy to type on a phone.
vim.keymap.set("n", "sd", "za", { buffer = true, desc = "Toggle code block" })
vim.keymap.set("n", "sa", function()
  require("core.phone_markdown").toggle_all_blocks()
end, { buffer = true, desc = "Toggle all code blocks" })
