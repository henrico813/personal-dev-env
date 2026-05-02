local opt = vim.opt

opt.number = true
opt.relativenumber = true
opt.expandtab = true
opt.shiftwidth = 2
opt.tabstop = 2
opt.termguicolors = true
opt.signcolumn = "yes"
opt.scrolloff = 8
opt.splitright = true
opt.splitbelow = true
opt.wrap = false
opt.ignorecase = true
opt.smartcase = true
opt.clipboard = "unnamedplus"
opt.undofile = true
opt.updatetime = 200
opt.cursorline = true
opt.linebreak = true
opt.mouse = "a"
opt.fillchars = { vert = "│", eob = " " }
opt.winbar = "%=%t %m"

vim.api.nvim_set_hl(0, "VertSplit",  { fg = "#7aa2f7", bg = "NONE" })
vim.api.nvim_set_hl(0, "NormalNC",   { bg = "#0c0e14" })
vim.api.nvim_set_hl(0, "WinBar",     { fg = "#7dcfff", bg = "#292e42", bold = true })
vim.api.nvim_set_hl(0, "WinBarNC",   { fg = "#3b4261", bg = "#0c0e14" })

local ai_filetypes = {
  "codecompanion",
  "codecompanion_input",
}

vim.api.nvim_create_autocmd("FileType", {
  pattern = ai_filetypes,
  callback = function()
    vim.wo.number = false
    vim.wo.relativenumber = false
    vim.wo.linebreak = true
    vim.wo.wrap = true
  end,
})
