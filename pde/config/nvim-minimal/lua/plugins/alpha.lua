local alpha = require("alpha")
local theta = require("alpha.themes.theta")

-- Parse a chafa --fg-only ANSI file into alpha header lines with per-run colors.
-- Returns a list of { text, hl = {{ "HL", start, stop }, ... } } per line.
local function load_ansi_header(path)
  local f = io.open(path, "r")
  if not f then return nil end
  local raw = f:read("*a")
  f:close()

  -- strip chafa's cursor show/hide sequences that can appear anywhere
  raw = raw:gsub("\27%[%?25[lh]", "")

  local out = {}
  for line in raw:gmatch("([^\r\n]+)") do
    if line ~= "" then
      local text_buf = {}
      local col = 0
      local hl = {}
      local run_hex, run_start = nil, 0
      local cur_hex = nil
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
      table.insert(out, { text = table.concat(text_buf), hl = hl })
    end
  end
  return out
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

local function build_header(parsed)
  local elements = {}
  for _, line in ipairs(parsed) do
    table.insert(elements, {
      type = "text",
      val = line.text,
      opts = { position = "center", hl = line.hl },
    })
  end
  return { type = "group", val = elements, opts = { position = "center" } }
end

local parsed = load_ansi_header(vim.fn.stdpath("config") .. "/header.ansi")
if parsed then
  register_highlights(parsed)
  theta.config.layout[2] = build_header(parsed)
end

-- push the header down toward the middle of the screen
theta.config.layout[1].val = 6

local buttons = require("alpha.themes.dashboard").button
theta.config.layout[6].val = {
  buttons("s", "  restore session",  function() require("persistence").load() end),
  buttons("f", "  find file",        "<cmd>FzfLua files<cr>"),
  buttons("r", "  recent files",     "<cmd>FzfLua oldfiles<cr>"),
  buttons("g", "  live grep",        "<cmd>FzfLua live_grep<cr>"),
  buttons("q", "  quit",             "<cmd>qa<cr>"),
}

alpha.setup(theta.config)
