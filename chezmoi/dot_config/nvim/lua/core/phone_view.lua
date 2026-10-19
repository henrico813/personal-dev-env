-- Phone view: at 72 columns or fewer, as over SSH from a phone, hides line
-- numbers, the sign column, the status line and the tab line, and wraps long
-- lines. The desktop view comes back when the screen widens.
local markdown = require("core.phone_markdown")

local M = {}

-- `tm` in ~/.zshrc picks its phone layout at the same width.
local max_columns = 72

local phone_window_opts = {
  number = false,
  relativenumber = false,
  signcolumn = "no",
  wrap = true,
  smoothscroll = true,
  breakindent = true,
}

-- The ruler and the last command fill half of a 40-column command line,
-- so short messages ask for Enter.
local phone_global_opts = {
  laststatus = 0,
  showtabline = 0,
  scrolloff = 2,
  ruler = false,
  showcmd = false,
}

local active = false
local desktop_global_opts = {}

function M.is_narrow()
  return vim.o.columns <= max_columns
end

local function is_markdown(buf)
  return vim.bo[buf].filetype == "markdown"
end

local function phone_values(buf)
  if not is_markdown(buf) then
    return phone_window_opts
  end
  return vim.tbl_extend("error", phone_window_opts, markdown.window_opts)
end

local function shows_phone_view(win)
  local buf = vim.api.nvim_win_get_buf(win)
  for name, value in pairs(phone_values(buf)) do
    -- zR and zM change foldlevel, and that is still the phone view.
    if name ~= "foldlevel" and vim.wo[win][name] ~= value then
      return false
    end
  end
  return true
end

local function is_floating(win)
  return vim.api.nvim_win_get_config(win).relative ~= ""
end

-- Saved per buffer because a buffer shown again in a window gets back the
-- options it last had, which may be phone values.
local function apply_phone_view(win)
  if is_floating(win) then
    return
  end
  local buf = vim.api.nvim_win_get_buf(win)
  local values = phone_values(buf)
  if not shows_phone_view(win) then
    local saved = {}
    for name in pairs(values) do
      saved[name] = vim.wo[win][name]
    end
    vim.b[buf].desktop_window_opts = saved
  end
  -- Local values only, so a buffer opened in a new window starts from the
  -- window's desktop values.
  for name, value in pairs(values) do
    vim.wo[win][0][name] = value
  end
end

-- BufWinEnter calls this on the desktop too, so a window not in the phone
-- view keeps its own values. A buffer with no saved values gets the global
-- ones.
local function restore_desktop_view(win)
  if is_floating(win) or not shows_phone_view(win) then
    return
  end
  local buf = vim.api.nvim_win_get_buf(win)
  local saved = vim.b[buf].desktop_window_opts or {}
  for name in pairs(phone_values(buf)) do
    local value = saved[name]
    if value == nil then
      value = vim.api.nvim_get_option_value(name, { scope = "global", win = win })
    end
    vim.wo[win][0][name] = value
  end
  -- Folds made by an expression stay after foldmethod goes back to manual.
  if is_markdown(buf) and vim.wo[win].foldmethod == "manual" then
    vim.api.nvim_win_call(win, function()
      vim.cmd("normal! zE")
    end)
  end
end

local function switch_view_for_width()
  if M.is_narrow() == active then
    return
  end
  active = M.is_narrow()
  for name, value in pairs(phone_global_opts) do
    if active then
      desktop_global_opts[name] = vim.o[name]
      vim.o[name] = value
    else
      vim.o[name] = desktop_global_opts[name]
    end
  end
  for _, win in ipairs(vim.api.nvim_list_wins()) do
    if active then
      apply_phone_view(win)
      markdown.reopen_fold(win)
    else
      if vim.wo[win].foldexpr == markdown.window_opts.foldexpr then
        markdown.remember_open_fold(win)
      end
      restore_desktop_view(win)
    end
  end
end

function M.setup()
  markdown.setup()
  local group = vim.api.nvim_create_augroup("phone_view", { clear = true })
  vim.api.nvim_create_autocmd({ "VimEnter", "VimResized" }, {
    group = group,
    callback = switch_view_for_width,
  })
  vim.api.nvim_create_autocmd("BufWinEnter", {
    group = group,
    callback = function()
      local win = vim.api.nvim_get_current_win()
      if active then
        apply_phone_view(win)
      else
        restore_desktop_view(win)
      end
    end,
  })
  -- bufferline shows its tab line again when a buffer or tab is added. Hide
  -- it, and keep bufferline's value for the desktop view.
  vim.api.nvim_create_autocmd({ "BufAdd", "TabEnter" }, {
    group = group,
    callback = function()
      if active and vim.o.showtabline ~= 0 then
        desktop_global_opts.showtabline = vim.o.showtabline
        vim.o.showtabline = 0
      end
    end,
  })
end

return M
