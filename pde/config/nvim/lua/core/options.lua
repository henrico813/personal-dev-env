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

local pi_filetypes = {
  "pi-chat-history",
  "pi-chat-prompt",
  "pi-chat-attachments",
}

local function is_pi_ft(ft)
  for _, v in ipairs(pi_filetypes) do
    if v == ft then return true end
  end
  return false
end

local function clamp_pi_windows()
  local max_width = math.floor(vim.o.columns * 0.40)
  local min_width = math.floor(vim.o.columns * 0.25)
  for _, win in ipairs(vim.api.nvim_list_wins()) do
    local buf = vim.api.nvim_win_get_buf(win)
    if is_pi_ft(vim.bo[buf].filetype) then
      local width = vim.api.nvim_win_get_width(win)
      if width > max_width then
        vim.api.nvim_win_set_width(win, max_width)
      elseif width < min_width then
        vim.api.nvim_win_set_width(win, min_width)
      end
    end
  end
end

vim.api.nvim_create_autocmd({ "WinResized", "VimResized" }, {
  callback = clamp_pi_windows,
})

vim.api.nvim_create_autocmd("FileType", {
  pattern = pi_filetypes,
  callback = function(ev)
    vim.wo.number = false
    vim.wo.relativenumber = false
    vim.wo.linebreak = true

    -- keep blink completion only in the prompt buffer
    if ev.match ~= "pi-chat-prompt" then
      vim.b[ev.buf].completion = false
    end

    -- render history as markdown via built-in treesitter
    if ev.match == "pi-chat-history" then
      vim.treesitter.language.register("markdown", "pi-chat-history")
      pcall(vim.treesitter.start, ev.buf, "markdown")
    end

    -- file picker in the prompt — inserts selected path at cursor
    if ev.match == "pi-chat-prompt" then
      vim.keymap.set({ "n", "i" }, "<C-f>", function()
        require("fzf-lua").files({
          actions = {
            ["default"] = function(selected)
              if selected and selected[1] then
                local path = selected[1]:gsub("^%s*[^%s]+%s+", "")
                vim.api.nvim_buf_call(ev.buf, function()
                  vim.api.nvim_put({ path }, "c", true, true)
                end)
              end
            end,
          },
        })
      end, { buffer = ev.buf, desc = "Insert file path" })
    end
  end,
})
