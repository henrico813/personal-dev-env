vim.g.mapleader = " "
vim.g.maplocalleader = "\\"

vim.cmd("packloadall")

require("core.options")
require("core.keymaps")
require("plugins.colorscheme")
require("plugins.ui")
require("plugins.fzf")
require("plugins.completion")
require("plugins.whichkey")
require("plugins.lsp")
require("plugins.gitsigns")
require("plugins.trouble")
require("plugins.grugfar")
require("plugins.session")
require("plugins.render-markdown")
do
  local ok, err = pcall(require, "plugins.obsidian")
  if not ok and not tostring(err):match("module 'plugins%.obsidian' not found") then
    vim.schedule(function()
      vim.notify(err, vim.log.levels.ERROR)
    end)
  end
end
require("plugins.codecompanion")
require("plugins.alpha")
require("plugins.lazygit")
require("plugins.gitlinker")
require("plugins.diffview")
require("plugins.lint")
