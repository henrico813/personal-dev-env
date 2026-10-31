-- Markdown on a phone: folds each code block, the frontmatter and each diff
-- header to one labeled line, and marks every screen row of an added or
-- removed diff line. phone_view sets these window values on Markdown buffers.
local M = {}

local ns = vim.api.nvim_create_namespace("phone_markdown")

M.window_opts = {
  foldmethod = "expr",
  foldexpr = "v:lua.require'core.phone_markdown'.fold_level(v:lnum)",
  foldtext = "v:lua.require'core.phone_markdown'.fold_text()",
  foldlevel = 0,
  foldenable = true,
}

local scans = {}

-- nvim asks for the fold level of every line, so the buffer is read once
-- per change and kept.
local function scan(buf)
  local tick = vim.api.nvim_buf_get_changedtick(buf)
  local found = scans[buf]
  if found and found.tick == tick then
    return found
  end
  local levels, labels, signs = {}, {}, {}
  local lines = vim.api.nvim_buf_get_lines(buf, 0, -1, false)
  local frontmatter = lines[1] == "---"
  local fence, fence_line, language, file, in_header
  for i, line in ipairs(lines) do
    if frontmatter then
      levels[i] = 1
      if i == 1 then
        labels[i] = "frontmatter"
      elseif line == "---" then
        frontmatter = false
        labels[1] = ("frontmatter · %d lines"):format(i)
      end
    elseif fence then
      local ticks = line:match("^%s*(```+)%s*$")
      -- A fence ends only at a line of at least as many backticks, so a
      -- ```` block can show a ``` block inside it.
      if ticks and #ticks >= fence then
        levels[i] = 1
        local name = file and vim.fn.fnamemodify(file, ":t") or language
        labels[fence_line] = ("%s · %d lines"):format(name, i - fence_line + 1)
        fence, in_header = nil, false
      elseif language == "diff" and line:match("^diff %-%-git ") then
        file = file or line:match(" b/(%S+)$")
        levels[i], labels[i], in_header = 2, "diff header", true
      elseif in_header and not line:match("^@@") then
        levels[i] = 2
      else
        levels[i], in_header = 1, false
        if language == "diff" then
          signs[i] = line:sub(1, 1)
        end
      end
    else
      local ticks, info = line:match("^%s*(```+)%s*(%S*)")
      if ticks then
        fence, fence_line, file = #ticks, i, nil
        language = info ~= "" and info or "code"
        levels[i] = 1
      else
        levels[i] = 0
      end
    end
  end
  found = { tick = tick, levels = levels, labels = labels, signs = signs }
  scans[buf] = found
  return found
end

local function label(buf, lnum)
  return "▸ " .. (scan(buf).labels[lnum] or "code")
end

function M.fold_level(lnum)
  return scan(vim.api.nvim_get_current_buf()).levels[lnum] or 0
end

function M.fold_text()
  return label(vim.api.nvim_get_current_buf(), vim.v.foldstart)
end

-- Moves to the first line of the next (1) or previous (-1) code block or
-- frontmatter, open or closed. nvim's own zj and zk stop at the diff header
-- fold inside an opened block.
function M.jump_to_block(direction)
  local levels = scan(vim.api.nvim_get_current_buf()).levels
  local lnum = vim.fn.line(".")
  for _ = 1, vim.v.count1 do
    local i = lnum + direction
    while levels[i] and not (levels[i] > 0 and (levels[i - 1] or 0) == 0) do
      i = i + direction
    end
    if not levels[i] then
      break
    end
    lnum = i
  end
  vim.api.nvim_win_set_cursor(0, { lnum, 0 })
end

-- Opens every block when any is closed, and closes them all otherwise.
function M.toggle_all_blocks()
  for lnum = 1, vim.fn.line("$") do
    if vim.fn.foldclosed(lnum) ~= -1 then
      vim.cmd("normal! zR")
      return
    end
  end
  vim.cmd("normal! zM")
end

-- Remembers whether the cursor is in an open fold before the desktop view
-- removes the folds.
function M.remember_open_fold(win)
  vim.w[win].phone_fold_open = vim.api.nvim_win_call(win, function()
    local lnum = vim.fn.line(".")
    return vim.fn.foldlevel(lnum) > 0 and vim.fn.foldclosed(lnum) == -1
  end)
end

-- Reopens that fold after the folds come back, so turning the phone does
-- not close the block being read.
function M.reopen_fold(win)
  if vim.w[win].phone_fold_open then
    vim.api.nvim_win_call(win, function()
      vim.cmd("normal! zv")
    end)
    vim.w[win].phone_fold_open = nil
  end
end

local sign_highlights = { ["+"] = "Added", ["-"] = "Removed" }

local function draw_line(win, buf, row)
  local lnum = row + 1
  local closed = vim.api.nvim_win_call(win, function()
    return vim.fn.foldclosed(lnum) == lnum
  end)
  if closed then
    -- render-markdown draws its code bar over a closed fold, so the label is
    -- drawn on top of it.
    local text = label(buf, lnum)
    local fill = vim.api.nvim_win_get_width(win) - vim.fn.strdisplaywidth(text)
    vim.api.nvim_buf_set_extmark(buf, ns, row, 0, {
      virt_text = { { text .. string.rep(" ", math.max(fill, 0)), "Folded" } },
      virt_text_pos = "overlay",
      hl_mode = "replace",
      priority = 10000,
      ephemeral = true,
    })
    return
  end
  local highlight = sign_highlights[scan(buf).signs[lnum]]
  if highlight then
    -- Every screen row of a wrapped line gets the bar, so its later rows do
    -- not look like unchanged lines.
    vim.api.nvim_buf_set_extmark(buf, ns, row, 0, {
      virt_text = { { "▌", highlight } },
      virt_text_pos = "overlay",
      virt_text_repeat_linebreak = true,
      priority = 10000,
      ephemeral = true,
    })
  end
end

function M.setup()
  vim.api.nvim_set_decoration_provider(ns, {
    on_win = function(_, win)
      return vim.wo[win].foldexpr == M.window_opts.foldexpr
    end,
    on_line = function(_, win, buf, row)
      draw_line(win, buf, row)
    end,
  })
end

return M
