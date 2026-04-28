local alpha = require("alpha")
local dashboard = require("alpha.themes.dashboard")

-- Parse a chafa --fg-only ANSI file into { { text, hl = {{"HL", s, e}, ...} }, ... }
local function load_ansi(path)
  local f = io.open(path, "r")
  if not f then return nil end
  local raw = f:read("*a")
  f:close()
  raw = raw:gsub("\27%[%?25[lh]", "")

  local lines = {}
  for line in raw:gmatch("([^\r\n]+)") do
    if line ~= "" then
      local text_buf, col, hl = {}, 0, {}
      local cur_hex, run_hex, run_start = nil, nil, 0
      local i = 1
      while i <= #line do
        local r, g, b, e = line:match("^\27%[38;2;(%d+);(%d+);(%d+)m()", i)
        if r then
          cur_hex = string.format("%02x%02x%02x", r, g, b)
          i = e
        elseif line:sub(i, i + 4) == "\27[39m" then
          cur_hex = nil; i = i + 5
        elseif line:sub(i, i + 2) == "\27[m" then
          cur_hex = nil; i = i + 3
        else
          local byte = line:byte(i)
          local cnt = 1
          if byte and byte >= 0xF0 then cnt = 4
          elseif byte and byte >= 0xE0 then cnt = 3
          elseif byte and byte >= 0xC0 then cnt = 2 end
          if cur_hex ~= run_hex then
            if run_hex then
              table.insert(hl, { "NeuroHL_" .. run_hex, run_start, col })
            end
            run_hex = cur_hex
            run_start = col
          end
          table.insert(text_buf, line:sub(i, i + cnt - 1))
          col = col + cnt
          i = i + cnt
        end
      end
      if run_hex then
        table.insert(hl, { "NeuroHL_" .. run_hex, run_start, col })
      end
      table.insert(lines, { text = table.concat(text_buf), width = col, hl = hl })
    end
  end
  return lines
end

local function register_highlights(parsed)
  local seen = {}
  for _, line in ipairs(parsed) do
    for _, spec in ipairs(line.hl) do
      local name = spec[1]
      if not seen[name] then
        seen[name] = true
        local hex = name:match("^NeuroHL_(%x+)$")
        if hex then vim.api.nvim_set_hl(0, name, { fg = "#" .. hex }) end
      end
    end
  end
end

-- Build side-by-side layout: image on the left, menu entries on the right.
-- Returns an alpha "group" element containing one composed text element per row.
local function build_side_by_side(image, menu, gap)
  gap = gap or 4
  local image_w, menu_w = 0, 0
  for _, l in ipairs(image) do if l.width > image_w then image_w = l.width end end
  for _, m in ipairs(menu) do if #m.text > menu_w then menu_w = #m.text end end
  local total_w = image_w + gap + menu_w

  local rows = math.max(#image, #menu)
  -- vertically center the menu against the image
  local menu_offset = math.floor((#image - #menu) / 2)
  if menu_offset < 0 then menu_offset = 0 end

  local elements = {}
  for i = 1, rows do
    local img = image[i]
    local menu_idx = i - menu_offset
    local m = menu[menu_idx]

    local text, hl
    if img then
      text = img.text .. string.rep(" ", image_w - img.width + gap)
      hl = {}
      for _, spec in ipairs(img.hl) do
        table.insert(hl, { spec[1], spec[2], spec[3] })
      end
    else
      text = string.rep(" ", image_w + gap)
      hl = {}
    end
    if m then
      local menu_col = #text
      text = text .. m.text
      if m.hl then
        table.insert(hl, { m.hl, menu_col, #text })
      end
      if m.shortcut_hl then
        local sc_start = text:find(m.shortcut, menu_col + 1, true)
        if sc_start then
          table.insert(hl, { m.shortcut_hl, sc_start - 1, sc_start - 1 + #m.shortcut })
        end
      end
    end
    -- pad every row to the same width so alpha centers them identically
    if #text < total_w then
      text = text .. string.rep(" ", total_w - #text)
    end

    table.insert(elements, {
      type = "text",
      val = text,
      opts = { position = "center", hl = hl, shortcut = m and m.shortcut or nil },
    })
  end

  -- Attach keymaps for each menu row so the shortcut still works
  for i, entry in ipairs(menu) do
    if entry.shortcut and entry.command then
      local row_idx = i + menu_offset
      local element = elements[row_idx]
      if element then
        element.on_press = function()
          if type(entry.command) == "function" then entry.command()
          else vim.cmd(entry.command) end
        end
      end
    end
  end

  return { type = "group", val = elements }
end

local image = load_ansi(vim.fn.stdpath("config") .. "/header.ansi") or {}
register_highlights(image)

-- List the top N persistence.nvim sessions, most recent first, each mapped to a
-- chdir + load function.
local function session_entries(n)
  n = n or 5
  local dir = vim.fn.stdpath("state") .. "/sessions"
  local files = vim.fn.globpath(dir, "*.vim", false, true)
  local entries = {}
  for _, f in ipairs(files) do
    local stat = vim.loop.fs_stat(f)
    if stat then table.insert(entries, { path = f, mtime = stat.mtime.sec }) end
  end
  table.sort(entries, function(a, b) return a.mtime > b.mtime end)

  local out = {}
  for i, e in ipairs(entries) do
    if i > n then break end
    -- persistence encodes cwd as %-separated path with %% for the branch suffix
    local name = vim.fn.fnamemodify(e.path, ":t:r")
    local cwd_part, branch = name:match("(.-)%%%%(.*)$")
    if not cwd_part then cwd_part = name end
    local cwd = cwd_part:gsub("%%", "/")
    local label = vim.fn.fnamemodify(cwd, ":t") .. (branch and (" (" .. branch .. ")") or "")
    local shortcut = tostring(i)
    table.insert(out, {
      text = shortcut .. "  " .. label,
      shortcut = shortcut,
      shortcut_hl = "Keyword",
      hl = "Type",
      command = function()
        vim.cmd("cd " .. vim.fn.fnameescape(cwd))
        require("persistence").load()
      end,
    })
  end
  return out
end

local menu = {
  { text = "f  find file",       shortcut = "f", shortcut_hl = "Keyword",
    hl = "Type", command = "FzfLua files" },
  { text = "r  recent files",    shortcut = "r", shortcut_hl = "Keyword",
    hl = "Type", command = "FzfLua oldfiles" },
  { text = "g  live grep",       shortcut = "g", shortcut_hl = "Keyword",
    hl = "Type", command = "FzfLua live_grep" },
  { text = "q  quit",            shortcut = "q", shortcut_hl = "Keyword",
    hl = "Type", command = "qa" },
}

-- Append session entries with a separator
local sessions = session_entries(5)
if #sessions > 0 then
  table.insert(menu, { text = "── sessions ──", hl = "Comment" })
  for _, s in ipairs(sessions) do table.insert(menu, s) end
end

-- Bind hotkeys globally so they trigger inside alpha regardless of row
local alpha_bufnr
vim.api.nvim_create_autocmd("FileType", {
  pattern = "alpha",
  callback = function(ev)
    alpha_bufnr = ev.buf
    for _, entry in ipairs(menu) do
      if entry.shortcut and entry.command then
        vim.keymap.set("n", entry.shortcut, function()
          if type(entry.command) == "function" then entry.command()
          else vim.cmd(entry.command) end
        end, { buffer = ev.buf, nowait = true, silent = true })
      end
    end
  end,
})

dashboard.config.layout = {
  { type = "padding", val = 4 },
  build_side_by_side(image, menu, 6),
  { type = "padding", val = 2 },
}

alpha.setup(dashboard.config)

-- Dynamic vertical centering — recompute top padding on enter/resize.
local content_rows = math.max(#image, #menu) + 2
local function recenter()
  if vim.bo.filetype ~= "alpha" then return end
  local pad = math.max(1, math.floor((vim.o.lines - content_rows) / 2))
  dashboard.config.layout[1].val = pad
  pcall(require("alpha").redraw)
end
vim.api.nvim_create_autocmd({ "VimEnter", "VimResized" }, { callback = recenter })
