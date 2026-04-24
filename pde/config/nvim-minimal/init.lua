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
require("plugins.pi")
require("plugins.alpha")
require("plugins.lazygit")
require("plugins.gitlinker")
